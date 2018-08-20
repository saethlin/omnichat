use bimap::BiMap;
use conn::ConnError::ConnectError;
use conn::{Conn, Event, Message};
use discord;
use discord::model::ChannelId;
use failure::Error;
use inlinable_string::InlinableString as IString;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, RwLock};
use std::thread;

pub struct DiscordConn {
    discord: Arc<RwLock<discord::Discord>>,
    _sender: SyncSender<Event>,
    name: IString,
    channels: BiMap<ChannelId, IString>,
    channel_names: Vec<IString>,
    handler: Arc<Handler>,
}

struct Handler {
    server_name: IString,
    channels: BiMap<ChannelId, IString>,
    mention_patterns: Vec<(IString, IString)>,
    channel_patterns: Vec<(IString, IString)>,
    discord: Arc<RwLock<discord::Discord>>,
}

impl Handler {
    pub fn to_omni(&self, message: &::discord::model::Message) -> String {
        let mut text = message.content.clone();
        for &(ref id, ref human) in &self.channel_patterns {
            text = text.replace(id.as_ref(), human);
        }

        for user in &message.mentions {
            let raw_mention = format!("{}", user.mention());
            text = if text.as_str().contains(&raw_mention) {
                text.replace(&raw_mention, &format!("@{}", user.name))
            } else {
                text.replace(&format!("<@!{}>", user.id), &format!("@{}", user.name))
            }
        }

        text
    }

    // TODO: This is incomplete, as I don't have a full listing of users
    // It's also laughably slow for a big server like progdisc
    pub fn to_discord(&self, mut text: String) -> String {
        for &(ref id, ref human) in self
            .mention_patterns
            .iter()
            .chain(self.channel_patterns.iter())
        {
            text = text.replace(human.as_ref(), id);
        }
        text
    }
}

impl DiscordConn {
    pub fn new(
        discord: Arc<RwLock<::discord::Discord>>,
        info: &::discord::model::ReadyEvent,
        event_stream: ::spmc::Receiver<::discord::model::Event>,
        server_name: &str,
        sender: SyncSender<Event>,
    ) -> Result<Box<Conn>, Error> {
        use discord::model::PossibleServer::Online;

        let server = info
            .servers
            .iter()
            .filter_map(|s| {
                if let Online(ref server) = s {
                    Some(server)
                } else {
                    None
                }
            }).find(|s| s.name == server_name)
            .ok_or(ConnectError)?
            .clone();

        let mut mention_patterns = Vec::new();
        for member in &server.members {
            let human = member.display_name();
            mention_patterns.push((
                format!("{}", member.user.mention()).into(),
                format!("@{}", human).into(),
            ));
            if member.nick.is_some() {
                let id = &member.user.id;
                mention_patterns.push((format!("<@!{}>", id).into(), format!("@{}", human).into()));
            }
        }

        let my_id = discord::State::new(info.clone()).user().id;
        let me_as_member = discord
            .read()
            .unwrap()
            .get_member(server.id, my_id)
            .unwrap();
        let my_roles = me_as_member.roles.clone();

        use discord::model::permissions::Permissions;
        use discord::model::ChannelType;
        let mut channel_names = Vec::new();
        let mut channel_ids = Vec::new();
        let mut channel_patterns = Vec::new();
        // Build a HashMap of all the channels we're permitted access to
        for channel in &server.channels {
            channel_patterns.push((
                format!("<#{}>", channel.id).into(),
                format!("#{}", channel.name).into(),
            ));

            // Check permissions
            let channel_perms = server.permissions_for(channel.id, my_id);

            let mut can_see = channel_perms.contains(Permissions::READ_MESSAGES);

            for perm_override in &channel.permission_overwrites {
                let is_for_me = match perm_override.kind {
                    ::discord::model::PermissionOverwriteType::Member(user_id) => user_id == my_id,
                    ::discord::model::PermissionOverwriteType::Role(role_id) => {
                        my_roles.iter().any(|r| r == &role_id)
                    }
                };
                if is_for_me && perm_override.allow.contains(Permissions::READ_MESSAGES) {
                    can_see = true;
                } else if is_for_me && perm_override.deny.contains(Permissions::READ_MESSAGES) {
                    can_see = false;
                }
            }

            // Also filter for channels that are not voice or category markers
            if can_see
                && channel.kind != ChannelType::Category
                && channel.kind != ChannelType::Voice
            {
                channel_names.push(channel.name.as_str().into());
                channel_ids.push(channel.id);
            }
        }

        let channels = BiMap::from(&channel_ids, &channel_names);

        let handler = Arc::new(Handler {
            server_name: server_name.into(),
            channels,
            mention_patterns,
            channel_patterns,
            discord: Arc::clone(&discord),
        });

        // Load message history
        for (id, name) in handler.channels.clone() {
            let handle = discord.clone();
            let sender = sender.clone();
            let handler = Arc::clone(&handler);
            // TODO: Figure out how to deal with not having a read state and get rid of the clone
            let last_read_timestamp = info
                .read_state
                .clone()
                .unwrap()
                .iter()
                .find(|i| i.id == id)
                .and_then(|i| i.last_message_id)
                .map(|i| i.creation_date())
                .unwrap();

            let read_at = last_read_timestamp.with_timezone(&::chrono::Utc);

            thread::spawn(move || {
                let current_user = handle.read().unwrap().get_current_user().unwrap();
                let my_mention = format!("{}", current_user.id.mention());
                let messages = handle
                    .read()
                    .unwrap()
                    .get_messages(id, discord::GetMessages::MostRecent, Some(100))
                    .unwrap_or_else(|e| {
                        error!("{}", e);
                        Vec::new()
                    });

                for m in messages {
                    sender
                        .send(Event::Message(Message {
                            server: handler.server_name.clone(),
                            channel: name.clone(),
                            sender: m.author.name.as_str().into(),
                            contents: handler.to_omni(&m),
                            is_mention: m
                                .mentions
                                .iter()
                                .map(|u| format!("{}", u.id.mention()))
                                .any(|m| m == my_mention),
                            timestamp: m.timestamp.with_timezone(&::chrono::Utc),
                            reactions: Vec::new(), // TODO: read reactions
                        })).expect("Sender died");
                }
                sender
                    .send(Event::HistoryLoaded {
                        server: handler.server_name.clone(),
                        channel: name.clone(),
                        read_at,
                    }).expect("sender died");
            });
        }

        {
            let sender = sender.clone();
            let handle = discord.clone();
            let handler = Arc::clone(&handler);
            // Launch a thread to handle incoming messages
            thread::spawn(move || {
                // Grab data to identify mentions of the logged in user
                let current_user = handle.read().unwrap().get_current_user().unwrap();
                let my_mention = format!("{}", current_user.id.mention());

                while let Ok(ev) = event_stream.recv() {
                    if let discord::model::Event::MessageCreate(message) = ev {
                        if let Some(channel_name) =
                            handler.channels.get_right(&message.channel_id).cloned()
                        {
                            sender
                                .send(Event::Message(Message {
                                    server: handler.server_name.clone(),
                                    channel: channel_name,
                                    is_mention: message
                                        .mentions
                                        .iter()
                                        .map(|u| format!("{}", u.id.mention()))
                                        .any(|m| m == my_mention),
                                    contents: handler.to_omni(&message),
                                    sender: message.author.name.into(),
                                    timestamp: message.timestamp.with_timezone(&::chrono::Utc),
                                    reactions: Vec::new(), // TODO: Add reactions
                                })).expect("Sender died");
                            // Ack the message
                            handler
                                .discord
                                .read()
                                .unwrap()
                                .ack_message(message.channel_id, message.id)
                                .unwrap();
                        } else {
                            // TODO: Messages from other servers end up here
                            // And they really shouldn't even be sent to us in the first place
                        }
                    }
                }
            });
        }

        Ok(Box::new(DiscordConn {
            discord,
            _sender: sender,
            name: handler.server_name.clone(),
            channels: handler.channels.clone(),
            channel_names,
            handler,
        }))
    }
}

impl Conn for DiscordConn {
    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let dis = self.discord.write().unwrap();
        if let Err(err) = dis.send_message(
            self.channels.get_left(channel).unwrap().clone(),
            &self.handler.to_discord(contents.to_string()),
            "",
            false,
        ) {
            error!("{}", err);
        }
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_ref()))
    }

    fn name(&self) -> &str {
        &self.name
    }
}
