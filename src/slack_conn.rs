use bimap::BiMap;
use conn::{Conn, Event, IString, Message};
use failure::Error;
use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use regex::Regex;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, RwLock};
use std::thread;

lazy_static! {
    pub static ref MENTION_REGEX: Regex = Regex::new(r"<@[A-Z0-9]{9}>").unwrap();
    pub static ref CHANNEL_REGEX: Regex = Regex::new(r"<#[A-Z0-9]{9}\|(?P<n>.*?)>").unwrap();
    pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
}

struct Handler {
    channels: BiMap<::slack_api::ConversationId, IString>,
    users: BiMap<::slack_api::UserId, IString>,
    server_name: IString,
    my_name: IString,
    input_sender: ::futures::sync::mpsc::Sender<::websocket::OwnedMessage>,
    tui_sender: SyncSender<Event>,
    pending_messages: Vec<PendingMessage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageAck {
    #[allow(unused)]
    ok: bool,
    reply_to: u32,
    text: String,
    ts: ::slack_api::Timestamp,
}

struct PendingMessage {
    id: u32,
    channel: IString,
}

impl Handler {
    pub fn to_omni(
        &self,
        message: ::slack_api::rtm::Message,
        outer_channel: Option<::slack_api::ConversationId>,
    ) -> Option<Message> {
        use slack_api::rtm::Message::*;
        use slack_api::rtm::{MessageBotMessage, MessageSlackbotResponse, MessageStandard};
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
                    outer_channel.or(channel),
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
                outer_channel.or(channel),
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
                outer_channel.or(channel),
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
                sender: user,
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

    pub fn process_slack_message(&mut self, message: &str) {
        // TODO: keep track of message indices
        if let Ok(ack) = ::serde_json::from_str::<MessageAck>(&message) {
            // Remove the message from pending messages
            if let Some(index) = self
                .pending_messages
                .iter()
                .position(|m| m.id == ack.reply_to)
            {
                let _ = self.tui_sender.send(Event::Message(Message {
                    channel: self.pending_messages[index].channel.clone(),
                    contents: ack.text,
                    is_mention: false,
                    reactions: Vec::new(),
                    sender: self.my_name.clone(),
                    server: self.server_name.clone(),
                    timestamp: ack.ts.into(),
                }));
                self.pending_messages.swap_remove(index);
                return;
            }
        }

        match ::serde_json::from_str::<::slack_api::rtm::Event>(&message) {
            Ok(::slack_api::rtm::Event::Message(::slack_api::rtm::Message::MessageChanged(
                ::slack_api::rtm::MessageMessageChanged {
                    channel,
                    message: Some(message),
                    previous_message: Some(previous_message),
                    ..
                },
            ))) => {
                let _ = self.tui_sender.send(Event::MessageEdited {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&channel)
                        .unwrap_or(&IString::from(channel.as_str()))
                        .clone(),
                    timestamp: previous_message.ts.into(),
                    contents: message.text,
                });
            }
            Ok(::slack_api::rtm::Event::ReactionAdded(::slack_api::rtm::EventReactionAdded {
                item,
                reaction,
                ..
            })) => {
                if let ::slack_api::rtm::Event::Message(message) = *item {
                    if let Some(omnimessage) = self.to_omni(message, None) {
                        let _ = self.tui_sender.send(Event::ReactionAdded {
                            server: omnimessage.server,
                            channel: omnimessage.channel,
                            timestamp: omnimessage.timestamp,
                            reaction: reaction.into(),
                        });
                    }
                }
            }
            Ok(::slack_api::rtm::Event::ReactionRemoved(
                ::slack_api::rtm::EventReactionRemoved { item, reaction, .. },
            )) => {
                if let ::slack_api::rtm::Event::Message(message) = *item {
                    if let Some(omnimessage) = self.to_omni(message, None) {
                        let _ = self.tui_sender.send(Event::ReactionRemoved {
                            server: omnimessage.server,
                            channel: omnimessage.channel,
                            timestamp: omnimessage.timestamp,
                            reaction: reaction.into(),
                        });
                    }
                }
            }
            // Miscellaneous slack messages that should appear as normal messages
            Ok(::slack_api::rtm::Event::Message(slack_message)) => {
                if let Some(omnimessage) = self.to_omni(slack_message.clone(), None) {
                    let _ = self.tui_sender.send(Event::Message(omnimessage));
                } else {
                    error!("Failed to convert message:\n{:#?}", slack_message);
                }
            }

            // Got some other kind of event we haven't handled yet
            Ok(::slack_api::rtm::Event::ChannelMarked(markevent)) => {
                let _ = self.tui_sender.send(Event::MarkChannelRead {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&markevent.channel.into())
                        .unwrap_or(&markevent.channel.as_str().into())
                        .clone(),
                    read_at: markevent.ts.into(),
                });
            }

            Ok(::slack_api::rtm::Event::GroupMarked(markevent)) => {
                let _ = self.tui_sender.send(Event::MarkChannelRead {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&markevent.channel.into())
                        .unwrap_or(&IString::from(markevent.channel.as_str()))
                        .clone(),
                    read_at: markevent.ts.into(),
                });
            }

            Ok(::slack_api::rtm::Event::FileShared(fileshare)) => {
                let _ = self.tui_sender.send(Event::Message(Message {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&fileshare.channel_id)
                        .unwrap_or(&fileshare.channel_id.as_str().into())
                        .clone(),
                    sender: self
                        .users
                        .get_right(&fileshare.user_id)
                        .unwrap_or(&fileshare.channel_id.as_str().into())
                        .clone(),
                    contents: fileshare.file_id.to_string(),
                    is_mention: false,
                    timestamp: fileshare
                        .ts
                        .map(|t| t.into())
                        .unwrap_or_else(::chrono::Utc::now),
                    reactions: Vec::new(),
                }));
            }

            Ok(_) => {}

            // Don't yet support this thing
            Err(e) => {
                let v: ::serde_json::Value = ::serde_json::from_str(&message).unwrap();
                error!(
                    "Failed to parse:\n{}\n{}",
                    ::serde_json::to_string_pretty(&v).unwrap(),
                    e
                );
            }
        }
    }
}

pub struct SlackConn {
    token: String,
    team_name: IString,
    users: BiMap<::slack_api::UserId, IString>,
    channels: BiMap<::slack_api::ConversationId, IString>,
    channel_names: Vec<IString>,
    handler: Arc<RwLock<Handler>>,
    _sender: SyncSender<Event>,
    emoji: Vec<IString>,
}

impl SlackConn {
    pub fn create_on(token: String, sender: SyncSender<Event>) -> Result<(), Error> {
        let emoji_handle = {
            let token = token.clone();
            thread::spawn(move || ::slack_api::http::emoji::list(&*CLIENT, &token))
        };

        let connect_handle = {
            use slack_api::http::rtm;
            let token = token.clone();
            thread::spawn(move || -> Result<rtm::ConnectResponse, Error> {
                Ok(rtm::connect(&*CLIENT, &token, &rtm::ConnectRequest::new())?)
            })
        };

        let users_handle = {
            use slack_api::http::users;
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let users_response = users::list(&*CLIENT, &token, &users::ListRequest::new())?;

                let mut users = BiMap::new();
                for user in users_response.members {
                    users.insert(user.id, IString::from(user.name));
                }

                Ok(users)
            })
        };

        let channels_handle = {
            use slack_api::http::channels;
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let channels = channels::list(&*CLIENT, &token, &channels::ListRequest::new())?;

                let mut channels_map = BiMap::new();
                let mut channel_names = Vec::new();
                for channel in channels
                    .channels
                    .into_iter()
                    .filter(|c| c.is_member.unwrap_or(false) && !c.is_archived.unwrap_or(true))
                {
                    let ::slack_api::rtm::Channel { id, name, .. } = channel;
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
            use slack_api::http::groups;
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                Ok(groups::list(&*CLIENT, &token, &groups::ListRequest::new())?)
            })
        };

        let dms_handle = {
            use slack_api::http::im;
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                Ok(im::list(&*CLIENT, &token, &im::ListRequest::new())?)
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
            let ::slack_api::rtm::Group { id, name, .. } = group;
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
            let ::slack_api::rtm::Im { id, user, .. } = dm;
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

        let websocket_url = response.url.clone();
        //let mut websocket = ::websocket::ClientBuilder::new(&response.url)?.connect_secure(None)?;

        let slf = response.slf;
        let team_name = response.team.name;
        let (input_sender, input_channel) = mpsc::channel(0);

        let handler = Arc::new(RwLock::new(Handler {
            channels: channels.clone(),
            users: users.clone(),
            server_name: team_name.as_str().into(),
            my_name: slf.name.into(),
            input_sender,
            tui_sender: sender.clone(),
            pending_messages: Vec::new(),
        }));

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

        let thread_handler = Arc::clone(&handler);

        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::result::WebSocketError;
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
            let mut core = ::tokio_core::reactor::Core::new().unwrap();
            let runner = ::websocket::ClientBuilder::new(&websocket_url)
                .unwrap()
                .async_connect_secure(None, &core.handle())
                .and_then(|(duplex, _)| {
                    let (sink, stream) = duplex.split();
                    stream
                        .filter_map(|message| match message {
                            Close(_) => {
                                error!("websocket closed");
                                None
                            }
                            Ping(m) => Some(Pong(m)),
                            Text(text) => {
                                thread_handler.write().unwrap().process_slack_message(&text);
                                None
                            }
                            _ => None,
                        }).select(input_channel.map_err(|_| WebSocketError::NoDataAvailable))
                        .forward(sink)
                });

            core.run(runner).unwrap();
        });

        // Launch threads to populate the message history
        for (conversation_id, conversation_name) in channels.clone() {
            let sender = sender.clone();
            let handler = handler.clone();
            let token = token.clone();
            let server_name = team_name.as_str().into();

            thread::spawn(move || {
                use slack_api::http::{channels, groups, im};
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
                        for m in &messages {
                            if let ::slack_api::rtm::Message::Tombstone(ref msg) = m {
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

                let handler_handle = handler.read().unwrap();
                for message in messages.into_iter().rev() {
                    if let Some(message) = handler_handle.to_omni(message, Some(conversation_id)) {
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
        let mut handler_handle = self.handler.write().unwrap();
        let contents = handler_handle.to_slack(contents.to_string());
        let channel_id = match handler_handle.channels.get_left(channel) {
            Some(id) => *id,
            None => {
                error!("Unknown channel: {}", channel);
                return;
            }
        };

        let mut id = 0;
        while handler_handle.pending_messages.iter().any(|m| m.id == id) {
            id += 1;
        }
        handler_handle.pending_messages.push(PendingMessage {
            channel: IString::from(channel),
            id,
        });

        // TODO: need some help from slack-rs-api here with a serialization struct
        let message = json!({
            "id": id,
            "type": "message",
            "channel": channel_id,
            "text": contents,
        });

        let the_json = ::serde_json::to_string(&message).unwrap();
        handler_handle
            .input_sender
            .clone()
            .send(::websocket::OwnedMessage::Text(the_json))
            .wait()
            .unwrap();
    }

    fn mark_read(&self, channel: &str) {
        use slack_api::http::{channels, groups, im};

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
            Some('+') => {
                if word.chars().count() > 2 {
                    self.emoji
                        .iter()
                        .filter(|name| name.starts_with(&word[2..]))
                        .map(|s| format!("+:{}:", s))
                        .collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn add_reaction(&self, reaction: &str, channel: &str, timestamp: ::conn::DateTime) {
        let token = self.token.clone();
        let name = IString::from(reaction);

        let channel = match self.channels.get_left(channel) {
            Some(c) => *c,
            None => {
                error!(
                    "Internal error, no known Slack ConversationId for channel name {}",
                    channel
                );
                return;
            }
        };

        thread::spawn(move || {
            use slack_api::http::reactions::Reactable;
            let request = ::slack_api::http::reactions::AddRequest::new(
                &name,
                Reactable::Message {
                    channel,
                    timestamp: timestamp.into(),
                },
            );

            if let Err(e) = ::slack_api::http::reactions::add(&*CLIENT, &token, &request) {
                error!("{:?}", e);
            }
        });
    }
}
