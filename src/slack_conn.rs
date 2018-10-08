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
                mut text,
                ts: Some(ts),
                reactions,
                files,
                ..
            }) => {
                let user = user.unwrap_or("UNKNOWNUS".into());
                for file in files.unwrap_or_default() {
                    text = format!("{}\n{}", text, file.url_private.unwrap());
                }
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

        use slack_api::rtm;
        match ::serde_json::from_str::<::slack_api::rtm::Event>(&message) {
            Ok(rtm::Event::Message {
                message:
                    rtm::Message::MessageChanged(rtm::MessageMessageChanged {
                        channel,
                        message: Some(message),
                        previous_message: Some(previous_message),
                        ..
                    }),
                ..
            }) => {
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
            Ok(rtm::Event::ReactionAdded { item, reaction, .. }) => {
                use slack_api::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    let _ = self.tui_sender.send(Event::ReactionAdded {
                        server: self.server_name.clone(),
                        channel: channel.clone(),
                        timestamp: timestamp.into(),
                        reaction: reaction.into(),
                    });
                }
            }
            Ok(rtm::Event::ReactionRemoved { item, reaction, .. }) => {
                use slack_api::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    let _ = self.tui_sender.send(Event::ReactionRemoved {
                        server: self.server_name.clone(),
                        channel: channel.clone(),
                        timestamp: timestamp.into(),
                        reaction: reaction.into(),
                    });
                }
            }
            // Miscellaneous slack messages that should appear as normal messages
            Ok(rtm::Event::Message {
                message: slack_message,
                ..
            }) => {
                if let Some(omnimessage) = self.to_omni(slack_message.clone(), None) {
                    let _ = self.tui_sender.send(Event::Message(omnimessage));
                } else {
                    error!("Failed to convert message:\n{:#?}", slack_message);
                }
            }

            // Got some other kind of event we haven't handled yet
            Ok(rtm::Event::ChannelMarked { channel, ts, .. }) => {
                let _ = self.tui_sender.send(Event::MarkChannelRead {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&channel.into())
                        .unwrap_or(&channel.as_str().into())
                        .clone(),
                    read_at: ts.into(),
                });
            }

            Ok(::slack_api::rtm::Event::GroupMarked { channel, ts, .. }) => {
                let _ = self.tui_sender.send(Event::MarkChannelRead {
                    server: self.server_name.clone(),
                    channel: self
                        .channels
                        .get_right(&channel.into())
                        .unwrap_or(&IString::from(channel.as_str()))
                        .clone(),
                    read_at: ts.into(),
                });
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

        let conversations_handle = {
            use slack_api::http::conversations;
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                use slack_api::http::conversations::ChannelType::*;
                let mut req = conversations::ListRequest::new();
                req.types = vec![PublicChannel, PrivateChannel, Mpim, Im];
                let channels = conversations::list(&*CLIENT, &token, &req);
                let channels = match channels {
                    Ok(c) => c,
                    Err(e) => {
                        use std::error::Error;
                        error!("{:?}, {:?}", e.cause(), e);
                        return Err(e)?;
                    }
                };

                let mut channels_map = BiMap::new();
                let mut channel_names = Vec::new();
                for channel in channels.channels.into_iter() {
                    // TODO: Filter for is_member and is_archived
                    use slack_api::http::conversations::Conversation::*;
                    let (id, name) = match channel {
                        Channel { id, name, .. } => (id, name),
                        Group { id, name, .. } => (id, name),
                        DirectMessage { id, .. } => (id, id.to_string()),
                    };
                    channel_names.push(name.as_str().into());
                    channels_map.insert(id, IString::from(name));
                }

                channel_names.sort();

                Ok((channel_names, channels_map))
            })
        };

        let (channel_names, channels) = conversations_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??;

        // Must have users before we can figure out who each DM is for
        let users: BiMap<::slack_api::UserId, IString> =
            users_handle.join().map_err(|e| format_err!("{:?}", e))??;

        let response = connect_handle
            .join()
            .map_err(|e| format_err!("{:?}", e))??;

        let websocket_url = response.url.clone();

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
                use slack_api::http::conversations;
                use slack_api::http::conversations::ConversationInfo;
                let req = conversations::InfoRequest::new(conversation_id);
                let read_at = conversations::info(&*CLIENT, &token, &req)
                    .map(|info| match info.channel {
                        ConversationInfo::Channel { last_read, .. } => {
                            last_read.map(|t| t.into()).unwrap_or(::chrono::Utc::now())
                        }
                        ConversationInfo::Group { last_read, .. } => last_read.into(),
                        ConversationInfo::ClosedDirectMessage { .. } => ::chrono::Utc::now(),
                        ConversationInfo::OpenDirectMessage { last_read, .. } => last_read.into(),
                    }).unwrap_or_else(|e| {
                        error!("{:?}", e);
                        ::chrono::Utc::now().into()
                    });
                let mut req = conversations::HistoryRequest::new(conversation_id);
                req.limit = Some(1000);
                let messages = match conversations::history(&*CLIENT, &token, &req) {
                    Ok(response) => response.messages,
                    Err(e) => {
                        error!("{:?}", e);
                        Vec::new()
                    }
                };
                for m in &messages {
                    if let ::slack_api::rtm::Message::Tombstone(ref msg) = m {
                        error!("{:#?}", msg);
                    }
                }

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
