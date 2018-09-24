use bimap::BiMap;
use conn::{Conn, Event, IString, Message};
use failure::Error;
use regex::Regex;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread;

lazy_static! {
    pub static ref MENTION_REGEX: Regex = Regex::new(r"<@[A-Z0-9]{9}>").unwrap();
    pub static ref CHANNEL_REGEX: Regex = Regex::new(r"<#[A-Z0-9]{9}\|(?P<n>.*?)>").unwrap();
    pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
}

#[derive(Clone)]
struct Handler {
    channels: BiMap<::slack_api::ConversationId, IString>,
    users: BiMap<::slack_api::UserId, IString>,
    server_name: IString,
    my_mention: String,
    my_name: IString,
}

impl Handler {
    pub fn to_omni(
        &self,
        message: ::slack_api::Message,
        outer_channel: Option<::slack_api::ConversationId>,
    ) -> Option<Message> {
        use slack_api::Message::*;
        use slack_api::{MessageBotMessage, MessageSlackbotResponse, MessageStandard};
        // TODO: Add more success cases to this
        let (channel, user, mut text, ts, reactions) = match message {
            Standard(MessageStandard {
                channel,
                user,
                text,
                ts: Some(ts),
                reactions,
                ..
            }) => {
                let user = user.unwrap_or("UNKNOWNUS".into());
                (
                    outer_channel.or_else(|| channel.map(|c| c.into())),
                    self.users
                        .get_right(&user)
                        .unwrap_or(&user.as_str().into())
                        .clone(),
                    text,
                    ts,
                    reactions
                        .iter()
                        .map(|r| (r.name.as_str().into(), r.count.unwrap_or_default() as usize))
                        .collect(),
                )
            }
            BotMessage(MessageBotMessage {
                channel,
                username: Some(name),
                text: Some(text),
                ts: Some(ts),
                reactions,
                ..
            }) => (
                outer_channel.or_else(|| channel.map(|c| c.into())),
                name.into(),
                text,
                ts,
                reactions
                    .iter()
                    .map(|r| (r.name.as_str().into(), r.count.unwrap_or_default() as usize))
                    .collect(),
            ),
            SlackbotResponse(MessageSlackbotResponse {
                channel,
                user: Some(user),
                text,
                ts: Some(ts),
                reactions,
                ..
            }) => (
                outer_channel.or_else(|| channel.map(|c| c.into())),
                self.users
                    .get_right(&user)
                    .unwrap_or(&user.as_str().into())
                    .clone(),
                text,
                ts,
                reactions
                    .iter()
                    .map(|r| (r.name.as_str().into(), r.count.unwrap_or_default() as usize))
                    .collect(),
            ),
            _ => return None,
        };

        text = text.replace("&amp;", "&");
        text = text.replace("&lt;", "<");
        text = text.replace("&gt;", ">");

        text = MENTION_REGEX
            .replace_all(&text, |caps: &::regex::Captures| {
                if let Some(name) = self.users.get_right(&caps[0][2..11].into()) {
                    format!("@{}", name)
                } else {
                    format!("@{}", &caps[0][2..11])
                }
            }).into_owned();

        text = CHANNEL_REGEX.replace_all(&text, "#$n").into_owned();

        if let Some(channel) = channel.and_then(|c| self.channels.get_right(&c)) {
            return Some(::conn::Message {
                server: self.server_name.as_ref().into(),
                channel: channel.clone(),
                sender: user.into(),
                is_mention: text.contains(self.my_name.as_ref()),
                contents: text,
                timestamp: ts.into(),
                reactions,
            });
        } else {
            return None;
        }
    }

    pub fn to_slack(&self, mut text: String) -> String {
        for (id, name) in self.users.iter() {
            let name_mention = format!("@{}", name);
            let slack_mention = format!("<@{}>", id);
            text = text.replace(&name_mention, &slack_mention);
        }

        for (id, name) in self.channels.iter() {
            let name_mention = format!("#{}", name);
            let slack_mention = format!("<#{}|{}>", id, name);
            text = text.replace(&name_mention, &slack_mention);
        }

        text
    }
}

pub struct SlackConn {
    token: String,
    team_name: IString,
    users: BiMap<::slack_api::UserId, IString>,
    channels: BiMap<::slack_api::ConversationId, IString>,
    channel_names: Vec<IString>,
    handler: Arc<Handler>,
    _sender: SyncSender<Event>,
    emoji: Vec<IString>,
}

impl SlackConn {
    pub fn create_on(token: String, sender: SyncSender<Event>) -> Result<(), Error> {
        let emoji_handle = {
            let token = token.clone();
            thread::spawn(move || ::slack_api::emoji::list(&*CLIENT, &token))
        };

        let connect_handle = {
            let token = token.clone();
            thread::spawn(
                move || -> Result<::slack_api::rtm::ConnectResponse, Error> {
                    Ok(::slack_api::rtm::connect(
                        &*CLIENT,
                        &token,
                        &::slack_api::rtm::ConnectRequest::new(),
                    )?)
                },
            )
        };

        let users_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let users_response = ::slack_api::users::list(
                    &*CLIENT,
                    &token,
                    &::slack_api::users::ListRequest::new(),
                )?;

                let mut users = BiMap::new();
                for user in users_response.members {
                    users.insert(user.id, IString::from(user.name));
                }

                Ok(users)
            })
        };

        let channels_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let channels = ::slack_api::channels::list(
                    &*CLIENT,
                    &token,
                    &::slack_api::channels::ListRequest::new(),
                )?;

                let mut channels_map = BiMap::new();
                let mut channel_names = Vec::new();
                for channel in channels
                    .channels
                    .into_iter()
                    .filter(|c| c.is_member.unwrap_or(false) && !c.is_archived.unwrap_or(true))
                {
                    let ::slack_api::Channel { id, name, .. } = channel;
                    channel_names.push(name.as_str().into());
                    channels_map.insert(
                        ::slack_api::ConversationId::Channel(id),
                        IString::from(name),
                    );
                }

                Ok((channel_names, channels_map))
            })
        };

        let groups_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                Ok(::slack_api::groups::list(
                    &*CLIENT,
                    &token,
                    &::slack_api::groups::ListRequest::new(),
                )?)
            })
        };

        let dms_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                Ok(::slack_api::im::list(
                    &*CLIENT,
                    &token,
                    &::slack_api::im::ListRequest::new(),
                )?)
            })
        };

        let (mut channel_names, mut channels) = channels_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??;

        for group in groups_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??
            .groups
            .into_iter()
            .filter(|g| !g.is_archived.unwrap_or(true))
        {
            let ::slack_api::Group { id, name, .. } = group;
            channel_names.push(name.as_str().into());
            channels.insert(::slack_api::ConversationId::Group(id), IString::from(name));
        }

        // Must have users before we can figure out who each DM is for
        let users: BiMap<::slack_api::UserId, IString> =
            users_handle.join().map_err(|e| format_err!("{:?}", e))??;

        for dm in dms_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??
            .ims
            .into_iter()
            .filter(|d| !d.is_user_deleted.unwrap_or(false))
        {
            let ::slack_api::Im { id, user, .. } = dm;
            let username = users
                .get_right(&user)
                .unwrap_or(&IString::from(user.as_str()))
                .clone();
            channel_names.push(username.clone());
            channels.insert(::slack_api::ConversationId::DirectMessage(id), username);
        }

        channel_names.sort();

        let response = connect_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??;

        let mut websocket = ::websocket::ClientBuilder::new(&response.url)?.connect_secure(None)?;

        let slf = response.slf;
        let team_name = response.team.name;

        let handler = Arc::new(Handler {
            channels: channels.clone(),
            users: users.clone(),
            server_name: team_name.as_str().into(),
            my_name: slf.name.into(),
            my_mention: format!("<@{}>", slf.id.clone()),
        });

        // Give the emoji handle as long as possible to complete
        let mut emoji = emoji_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??
            .emoji
            .unwrap_or_default()
            .keys()
            .map(|e| IString::from(e.as_str()))
            .collect::<Vec<_>>();
        emoji.sort();

        let _ = sender.send(Event::Connected(Box::new(SlackConn {
            token: token.clone(),
            users,
            channels: channels.clone(),
            channel_names,
            team_name: team_name.as_str().into(),
            _sender: sender.clone(),
            handler: handler.clone(),
            emoji,
        })));

        let thread_sender = sender.clone();
        let thread_handler = Arc::clone(&handler);

        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
            loop {
                match websocket.recv_message() {
                    Ok(Text(message)) => {
                        // parse the message and add it to events
                        match ::serde_json::from_str::<::slack_api::Event>(&message) {
                            Ok(::slack_api::Event::Message(
                                ::slack_api::Message::MessageChanged(
                                    ::slack_api::MessageMessageChanged {
                                        channel,
                                        message: Some(message),
                                        previous_message: Some(previous_message),
                                        ..
                                    },
                                ),
                            )) => {
                                let _ = thread_sender.send(Event::MessageEdited {
                                    server: thread_handler.server_name.clone(),
                                    channel: thread_handler
                                        .channels
                                        .get_right(&channel.into())
                                        .unwrap_or(&IString::from(channel.as_str()))
                                        .clone(),
                                    timestamp: previous_message.ts.into(),
                                    contents: message.text,
                                });
                            }
                            Ok(::slack_api::Event::ReactionAdded(
                                ::slack_api::EventReactionAdded { item, reaction, .. },
                            )) => {
                                if let ::slack_api::Event::Message(message) = *item {
                                    if let Some(omnimessage) = thread_handler.to_omni(message, None)
                                    {
                                        let _ = thread_sender.send(Event::ReactionAdded {
                                            server: omnimessage.server,
                                            channel: omnimessage.channel,
                                            timestamp: omnimessage.timestamp,
                                            reaction: reaction.into(),
                                        });
                                    }
                                }
                            }
                            Ok(::slack_api::Event::ReactionRemoved(
                                ::slack_api::EventReactionRemoved { item, reaction, .. },
                            )) => {
                                if let ::slack_api::Event::Message(message) = *item {
                                    if let Some(omnimessage) = thread_handler.to_omni(message, None)
                                    {
                                        let _ = thread_sender.send(Event::ReactionRemoved {
                                            server: omnimessage.server,
                                            channel: omnimessage.channel,
                                            timestamp: omnimessage.timestamp,
                                            reaction: reaction.into(),
                                        });
                                    }
                                }
                            }
                            // Miscellaneous slack messages that should appear as normal messages
                            Ok(::slack_api::Event::Message(slack_message)) => {
                                if let Some(omnimessage) =
                                    thread_handler.to_omni(slack_message, None)
                                {
                                    let _ = thread_sender.send(Event::Message(omnimessage));
                                } else {
                                    error!("Failed to convert message:\n{}", message);
                                }
                            }

                            // Got some other kind of event we haven't handled yet
                            Ok(::slack_api::Event::ChannelMarked(markevent)) => {
                                let _ = thread_sender.send(Event::MarkChannelRead {
                                    server: thread_handler.server_name.clone(),
                                    channel: thread_handler
                                        .channels
                                        .get_right(&markevent.channel.into())
                                        .unwrap_or(&markevent.channel.as_str().into())
                                        .clone(),
                                    read_at: markevent.ts.into(),
                                });
                            }

                            Ok(::slack_api::Event::GroupMarked(markevent)) => {
                                let _ = thread_sender.send(Event::MarkChannelRead {
                                    server: thread_handler.server_name.clone(),
                                    channel: thread_handler
                                        .channels
                                        .get_right(&markevent.channel.into())
                                        .unwrap_or(&IString::from(markevent.channel.as_str()))
                                        .clone(),
                                    read_at: markevent.ts.into(),
                                });
                            }

                            Ok(::slack_api::Event::FileShared(fileshare)) => {
                                let _ = thread_sender.send(Event::Message(Message {
                                    server: thread_handler.server_name.clone(),
                                    channel: thread_handler
                                        .channels
                                        .get_right(&fileshare.channel_id.into())
                                        .unwrap_or(&fileshare.channel_id.as_str().into())
                                        .clone(),
                                    sender: thread_handler
                                        .users
                                        .get_right(&fileshare.user_id)
                                        .unwrap_or(&fileshare.channel_id.as_str().into())
                                        .clone(),
                                    contents: fileshare.file_id.to_string(),
                                    is_mention: false,
                                    timestamp: fileshare
                                        .ts
                                        .map(|t| t.into())
                                        .unwrap_or(::chrono::Utc::now()),
                                    reactions: Vec::new(),
                                }));
                            }

                            Ok(_) => {}

                            // Don't yet support this thing
                            Err(e) => {
                                error!("Failed to parse:\n{}\n{}", message, e);
                            }
                        }
                    }
                    Ok(Ping(data)) => {
                        websocket.send_message(&Pong(data)).unwrap_or_else(|_| {
                            error!("Failed to Pong");
                        });
                    }
                    Ok(Close(_)) => {
                        error!("Slack websocket closed");
                    }
                    _ => {}
                }
            }
        });

        // Launch threads to populate the message history
        for (conversation_id, conversation_name) in channels.clone() {
            let sender = sender.clone();
            let handler = handler.clone();
            let token = token.clone();
            let server_name = team_name.as_str().into();

            thread::spawn(move || {
                use slack_api::{channels, groups, im};
                use std::error::Error;
                let (messages, read_at) = match conversation_id {
                    ::slack_api::ConversationId::Channel(channel_id) => {
                        let req = channels::InfoRequest::new(channel_id);
                        let read_at = channels::info(&*CLIENT, &token, &req)
                            .and_then(|info| {
                                info.channel.last_read.ok_or(::slack_api::Error::Slack(
                                    "timestamp missing".to_owned(),
                                ))
                            }).unwrap_or_else(|e| {
                                error!("{:?}", e);
                                ::chrono::Utc::now().into()
                            });
                        let mut req = channels::HistoryRequest::new(channel_id);
                        req.count = Some(1000);
                        let messages = match channels::history(&*CLIENT, &token, &req) {
                            Ok(response) => response.messages,
                            Err(e) => {
                                error!("{:?}", e.cause());
                                Vec::new()
                            }
                        };
                        for m in messages.iter() {
                            if let ::slack_api::Message::Tombstone(ref msg) = m {
                                error!("{:#?}", msg);
                            }
                        }
                        (messages, read_at)
                    }
                    ::slack_api::ConversationId::Group(group_id) => {
                        let mut req = groups::InfoRequest::new(group_id);
                        let read_at = groups::info(&*CLIENT, &token, &req)
                            .and_then(|info| {
                                info.group.last_read.ok_or(::slack_api::Error::Slack(
                                    "timestamp missing".to_owned(),
                                ))
                            }).unwrap_or_else(|e| {
                                error!("{:?}", e);
                                ::chrono::Utc::now().into()
                            });

                        let mut req = groups::HistoryRequest::new(group_id);
                        req.count = Some(1000);
                        let messages = match groups::history(&*CLIENT, &token, &req) {
                            Ok(response) => response.messages,
                            Err(e) => {
                                error!("{:?}", e.cause());
                                Vec::new()
                            }
                        };
                        (messages, read_at)
                    }
                    ::slack_api::ConversationId::DirectMessage(dm_id) => {
                        let messages =
                            match im::history(&*CLIENT, &token, &im::HistoryRequest::new(dm_id)) {
                                Ok(response) => response.messages,
                                Err(e) => {
                                    error!("{:?}", e.cause());
                                    Vec::new()
                                }
                            };
                        (
                            messages,
                            ::slack_api::Timestamp::from(::chrono::offset::Utc::now()),
                        )
                    }
                };

                for message in messages.into_iter().rev() {
                    if let Some(message) = handler.to_omni(message, Some(conversation_id)) {
                        let _ = sender.send(Event::Message(message));
                    }
                }
                let _ = sender.send(Event::HistoryLoaded {
                    server: server_name,
                    channel: conversation_name,
                    read_at: read_at.into(),
                });
            });
        }

        Ok(())
    }
}

impl Conn for SlackConn {
    fn name(&self) -> &str {
        &self.team_name
    }

    fn channels(&self) -> &[IString] {
        &self.channel_names
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let token = self.token.clone();
        let contents = self.handler.to_slack(contents.to_string());
        let channel = match self.handler.channels.get_left(channel.into()) {
            Some(id) => id.clone(),
            None => {
                error!("Unknown channel: {}", channel);
                return;
            }
        };

        ::std::thread::spawn(move || {
            use slack_api::chat::post_message;
            let mut request = ::slack_api::chat::PostMessageRequest::new(channel, &contents);
            request.as_user = Some(true);
            // This is a hack because the reqwest client randomly disconnects; re-sending the
            // request makes it reconnect
            if post_message(&*CLIENT, &token, &request).is_err() {
                if let Err(e) = post_message(&*CLIENT, &token, &request) {
                    error!("{}", e);
                }
            }
        });
    }

    fn mark_read(&self, channel: &str) {
        use slack_api::{channels, groups, im};

        let channel_or_group_id = match self.channels.get_left(channel) {
            Some(s) => *s,
            None => {
                error!(
                    "Tried to mark unread for channel {} in server {} but channel does not exist",
                    channel,
                    self.name()
                );
                return;
            }
        };

        let token = self.token.clone();

        let timestamp = ::chrono::offset::Utc::now().into();

        thread::spawn(move || match channel_or_group_id {
            ::slack_api::ConversationId::Channel(channel_id) => {
                let request = channels::MarkRequest::new(channel_id, timestamp);
                if let Err(e) = channels::mark(&*CLIENT, &token, &request) {
                    error!("{}", e);
                }
            }
            ::slack_api::ConversationId::Group(group_id) => {
                let request = groups::MarkRequest::new(group_id, timestamp);
                if let Err(e) = groups::mark(&*CLIENT, &token, &request) {
                    error!("{}", e);
                }
            }
            ::slack_api::ConversationId::DirectMessage(dm_id) => {
                let request = im::MarkRequest::new(dm_id, timestamp);
                if let Err(e) = im::mark(&*CLIENT, &token, &request) {
                    error!("{}", e);
                }
            }
        });
    }

    fn autocomplete(&self, word: &str) -> Vec<String> {
        match word.chars().next() {
            Some('@') => self
                .users
                .iter()
                .map(|(_id, name)| name)
                .filter(|name| name.starts_with(&word[1..]))
                .map(|s| String::from("@") + s)
                .collect(),
            Some('#') => self
                .channels
                .iter()
                .map(|(_id, name)| name)
                .filter(|name| name.starts_with(&word[1..]))
                .map(|s| String::from("#") + s)
                .collect(),
            Some(':') => self
                .emoji
                .iter()
                .filter(|name| name.starts_with(&word[1..]))
                .map(|s| format!(":{}:", s))
                .collect(),
            Some('+') => if word.chars().count() > 2 {
                self.emoji
                    .iter()
                    .filter(|name| name.starts_with(&word[2..]))
                    .map(|s| format!("+:{}:", s))
                    .collect()
            } else {
                Vec::new()
            },
            _ => Vec::new(),
        }
    }

    fn add_reaction(&self, reaction: &str, channel: &str, timestamp: ::conn::DateTime) {
        let token = self.token.clone();
        let name = IString::from(reaction);

        let channel = match self.channels.get_left(channel) {
            Some(c) => *c,
            None => {
                error!("internal error, invalid channel name {}", channel);
                return;
            }
        };

        thread::spawn(move || {
            use slack_api::reactions::Reactable;
            let request = ::slack_api::reactions::AddRequest::new(
                &name,
                Reactable::Message {
                    channel,
                    timestamp: timestamp.into(),
                },
            );

            if let Err(e) = ::slack_api::reactions::add(&*CLIENT, &token, &request) {
                error!("{:?}", e);
            }
        });
    }
}
