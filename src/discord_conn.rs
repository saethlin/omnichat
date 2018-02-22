use std::sync::mpsc::Sender;
use std::thread;
use std::sync::{Arc, RwLock};
use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message};
use conn::ConnError::DiscordError;
use failure::Error;
use discord;
use discord::model::ChannelId;

pub struct DiscordConn {
    discord: Arc<RwLock<discord::Discord>>,
    sender: Sender<Event>,
    name: String,
    channels: BiMap<ChannelId, String>,
    channel_names: Vec<String>,
}

impl DiscordConn {
    pub fn new(
        token: String,
        server_name: String,
        sender: Sender<Event>,
    ) -> Result<Box<Conn>, Error> {
        use discord::model::PossibleServer::Online;

        let dis = discord::Discord::from_user_token(&token)?;
        let (mut connection, info) = dis.connect()?;

        let server = info.servers
            .iter()
            .filter_map(|s| {
                if let &Online(ref server) = s {
                    Some(server)
                } else {
                    None
                }
            })
            .find(|s| s.name == server_name)
            .ok_or(DiscordError)?
            .clone();

        let my_id = discord::State::new(info).user().id;

        use discord::model::ChannelType;
        use discord::model::permissions::Permissions;
        let mut channel_names = Vec::new();
        let mut channel_ids = Vec::new();
        // Build a HashMap of all the channels we're permitted access to
        for channel in &server.channels {
            // Check permissions
            let channel_perms = server.permissions_for(channel.id, my_id);

            if channel.kind == ChannelType::Text
                && channel_perms.contains(Permissions::READ_MESSAGES | Permissions::SEND_MESSAGES)
            {
                channel_names.push(channel.name.clone());
                channel_ids.push(channel.id);
            }
        }

        let channels = BiMap::new(BiMapBuilder {
            human: channel_names.clone(),
            id: channel_ids,
        });

        // Collect a vector of the channels we have muted

        let handle = Arc::new(RwLock::new(dis));
        // Load message history
        let t_channels = channels.clone();
        for (id, name) in t_channels.into_iter() {
            let t_dis = handle.clone();
            let t_sender = sender.clone();
            let serv_name = server_name.clone();
            thread::spawn(move || {
                let mut messages = t_dis
                    .read()
                    .unwrap()
                    .get_messages(id, discord::GetMessages::MostRecent, None)
                    .unwrap_or_else(|e| {
                        t_sender
                            .send(Event::Error(format!("{}", e)))
                            .expect("Sender died");
                        Vec::new()
                    });

                // TODO: handle ordering of messages in the frontend
                messages.sort_by_key(|m| m.timestamp.timestamp());

                for m in messages.into_iter() {
                    t_sender
                        .send(Event::HistoryMessage(Message {
                            server: serv_name.clone(),
                            channel: name.clone(),
                            sender: m.author.name,
                            contents: m.content,
                        }))
                        .expect("Sender died");
                }
                t_sender
                    .send(Event::HistoryLoaded {
                        server: serv_name.clone(),
                        channel: name.clone(),
                    })
                    .expect("sender died");;
            });
        }

        let h_sender = sender.clone();
        let serv_name = server_name.clone();
        let h_channels = channels.clone();
        let h_handle = handle.clone();
        // Launch a thread to handle incoming messages
        thread::spawn(move || {
            // Grab data to identify mentions of the logged in user
            // TODO: Why can't I ? on the return of .read()
            let current_user = h_handle.read().unwrap().get_current_user().unwrap();
            let mut my_mention = format!("{}", current_user.id.mention());
            my_mention.insert(2, '!');

            while let Ok(ev) = connection.recv_event() {
                match ev {
                    discord::model::Event::MessageCreate(message) => {
                        if h_channels.contains_id(&message.channel_id) {
                            let content = message.content.clone();
                            let event = Message {
                                server: serv_name.clone(),
                                channel: h_channels.get_human(&message.channel_id).unwrap().clone(),
                                contents: message.content,
                                sender: message.author.name,
                            };

                            if content.contains(&my_mention) {
                                h_sender.send(Event::Mention(event)).expect("Sender died");
                            } else {
                                h_sender.send(Event::Message(event)).expect("Sender died");
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        return Ok(Box::new(DiscordConn {
            discord: handle.clone(),
            sender: sender,
            name: server_name.clone(),
            channels: channels,
            channel_names: channel_names,
        }));
    }
}

impl Conn for DiscordConn {
    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let dis = self.discord.write().unwrap();
        if let Err(_) = dis.send_message(
            self.channels
                .get_id(&String::from(channel))
                .unwrap()
                .clone(),
            contents,
            "",
            false,
        ) {
            self.sender
                .send(Event::Error("Message failed to send".to_owned()))
                .expect("Sender died");
        }
    }

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn channels(&self) -> Vec<&String> {
        self.channel_names.iter().collect()
    }

    fn autocomplete(&self, _word: &str) -> Option<String> {
        None
    }

    fn name(&self) -> &String {
        &self.name
    }
}
