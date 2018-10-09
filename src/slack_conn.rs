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
    pub static ref HYPERCLIENT: ::hyper::Client<::hyper_rustls::HttpsConnector<::hyper::client::HttpConnector>, ::hyper::Body> = {
        let https = ::hyper_rustls::HttpsConnector::new(4);
        ::hyper::client::Client::builder().build::<_, ::hyper::Body>(https)
    };
    pub static ref RUNTIME: ::std::sync::Mutex<::tokio::runtime::Runtime> =
        ::std::sync::Mutex::new(::tokio::runtime::Runtime::new().unwrap());
}

/*
fn to_url<T: ::serde::Serialize>(base: &str, params: &T) -> ::hyper::Uri {
    format!("{}&{}", base, ::serde_urlencoded::to_string(params))
        .parse()
        .unwrap();
}
*/

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
                    if text.is_empty() {
                        text = file.url_private.unwrap();
                    } else {
                        text = format!("{}\n{}", text, file.url_private.unwrap());
                    }
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
                if let (
                    rtm::Message::Standard(rtm::MessageStandard { text, .. }),
                    rtm::Message::Standard(rtm::MessageStandard { ts: Some(ts), .. }),
                ) = (*message, *previous_message)
                {
                    let _ = self.tui_sender.send(Event::MessageEdited {
                        server: self.server_name.clone(),
                        channel: self
                            .channels
                            .get_right(&channel)
                            .unwrap_or(&IString::from(channel.as_str()))
                            .clone(),
                        timestamp: ts.into(),
                        contents: text,
                    });
                }
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
        // Request information on the group's emoji

        let emoji_recv = {
            use slack_api::http::emoji;
            let (send, recv) = ::std::sync::mpsc::sync_channel(1);
            let url = format!("https://slack.com/api/{}?token={}&", "emoji.list", token)
                .parse()
                .unwrap();

            RUNTIME.lock().unwrap().spawn(
                HYPERCLIENT
                    .get(url)
                    .and_then(|res| res.into_body().concat2())
                    .map(|body| ::serde_json::from_slice::<emoji::ListResponse>(&body).unwrap())
                    .and_then(move |response| {
                        send.send(response).unwrap();
                        Ok(())
                    }).map_err(|e| error!("{:?}", e)),
            );
            recv
        };

        let connect_recv = {
            use slack_api::http::rtm;
            // Get the websocket URL we can talk to
            let url = format!("https://slack.com/api/{}?token={}&", "rtm.connect", token)
                .parse()
                .unwrap();

            let (send, recv) = ::std::sync::mpsc::sync_channel(1);
            RUNTIME.lock().unwrap().spawn(
                HYPERCLIENT
                    .get(url)
                    .and_then(|res| res.into_body().concat2())
                    .map(|body| ::serde_json::from_slice::<rtm::ConnectResponse>(&body).unwrap())
                    .and_then(move |response| {
                        send.send(response).unwrap();
                        Ok(())
                    }).map_err(|e| error!("{:?}", e)),
            );
            recv
        };

        let users_recv = {
            use slack_api::http::users;
            let url = format!(
                "https://slack.com/api/{}?token={}&{}",
                "users.list",
                token,
                ::serde_urlencoded::to_string(&users::ListRequest::new()).unwrap()
            ).parse()
            .unwrap();

            let (send, recv) = ::std::sync::mpsc::sync_channel(1);
            RUNTIME.lock().unwrap().spawn(
                HYPERCLIENT
                    .get(url)
                    .and_then(|res| res.into_body().concat2())
                    .map(|body| ::serde_json::from_slice::<users::ListResponse>(&body).unwrap())
                    .map(|resp| {
                        let mut users: BiMap<::slack_api::UserId, IString> = BiMap::new();
                        for user in resp.members {
                            users.insert(user.id, IString::from(user.name));
                        }
                        users
                    }).and_then(move |response| {
                        send.send(response).unwrap();
                        Ok(())
                    }).map_err(|e| error!("{:?}", e)),
            );
            recv
        };

        let conversations_recv = {
            use slack_api::http::conversations;
            use slack_api::http::conversations::ChannelType::*;
            let mut req = conversations::ListRequest::new();
            req.types = vec![PublicChannel, PrivateChannel, Mpim, Im];
            let url = format!(
                "https://slack.com/api/{}?token={}&{}",
                "conversations.list",
                token,
                ::serde_urlencoded::to_string(&req).unwrap()
            ).parse()
            .unwrap();

            let (send, recv) = ::std::sync::mpsc::sync_channel(1);
            RUNTIME.lock().unwrap().spawn(
                HYPERCLIENT
                    .get(url)
                    .and_then(|res| res.into_body().concat2())
                    .map(|body| {
                        ::serde_json::from_slice::<conversations::ListResponse>(&body).unwrap()
                    }).map(|resp| resp.channels)
                    .and_then(move |response| {
                        send.send(response).unwrap();
                        Ok(())
                    }).map_err(|e| error!("{:?}", e)),
            );
            recv
        };

        // Must have users before we can figure out who each DM is for
        let users = users_recv.recv()?;

        let response_channels = conversations_recv.recv()?;

        use slack_api::http::conversations::Conversation::*;
        let mut channels = BiMap::new();
        let mut channel_names = Vec::new();
        for (id, name) in response_channels
            .into_iter()
            .filter_map(|channel| match channel {
                Channel {
                    id,
                    name,
                    is_member: true,
                    ..
                } => Some((id, name.into())),
                Group {
                    id,
                    name,
                    is_member: true,
                    ..
                } => Some((id, name.into())),
                DirectMessage { id, user, .. } => {
                    users.get_right(&user).map(|name| (id, name.clone()))
                }
                _ => None,
            }) {
            channel_names.push(name.clone());
            channels.insert(id, name);
        }

        channel_names.sort();

        let response = connect_recv.recv()?;
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
        let mut emoji = emoji_recv
            .recv()?
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
        // No idea how to do this with new tokio
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
            let handler = handler.clone();
            let token = token.clone();
            let server_name = team_name.as_str().into();

            use slack_api::http::conversations;
            use slack_api::http::conversations::ConversationInfo;

            let info_recv = {
                let (send, recv) = ::std::sync::mpsc::sync_channel(1);
                let info_url = format!(
                    "https://slack.com/api/{}?token={}&{}",
                    "conversations.info",
                    token,
                    ::serde_urlencoded::to_string(&conversations::InfoRequest::new(
                        conversation_id
                    )).unwrap()
                ).parse()
                .unwrap();

                RUNTIME.lock().unwrap().spawn(
                    HYPERCLIENT
                        .get(info_url)
                        .and_then(|res| res.into_body().concat2())
                        .map(|body| {
                            ::serde_json::from_slice::<conversations::InfoResponse>(&body).unwrap()
                        }).map(|info| match info.channel {
                            ConversationInfo::Channel { last_read, .. } => {
                                last_read.map(|t| t.into()).unwrap_or(::chrono::Utc::now())
                            }
                            ConversationInfo::Group { last_read, .. } => last_read.into(),
                            ConversationInfo::ClosedDirectMessage { .. } => ::chrono::Utc::now(),
                            ConversationInfo::OpenDirectMessage { last_read, .. } => {
                                last_read.into()
                            }
                        }).and_then(move |response| {
                            send.send(response).unwrap();
                            Ok(())
                        }).map_err(|e| error!("{:?}", e)),
                );
                recv
            };

            // Load the message history
            let history_recv = {
                let sender = sender.clone();
                let (send, recv) = ::std::sync::mpsc::sync_channel(1);
                let mut req = conversations::HistoryRequest::new(conversation_id);
                req.limit = Some(1000);
                let info_url = format!(
                    "https://slack.com/api/{}?token={}&{}",
                    "conversations.history",
                    token,
                    ::serde_urlencoded::to_string(&req).unwrap()
                ).parse()
                .unwrap();

                RUNTIME.lock().unwrap().spawn(
                    HYPERCLIENT
                        .get(info_url)
                        .and_then(|res| res.into_body().concat2())
                        .map(|body| {
                            ::serde_json::from_slice::<conversations::HistoryResponse>(&body)
                                .unwrap()
                        }).map(|history| history.messages)
                        .map(move |messages| {
                            let handler_handle = handler.read().unwrap();
                            for message in messages.into_iter().rev() {
                                if let Some(message) =
                                    handler_handle.to_omni(message, Some(conversation_id))
                                {
                                    let _ = sender.send(Event::Message(message));
                                }
                            }
                        }).and_then(move |response| {
                            send.send(response).unwrap();
                            Ok(())
                        }).map_err(|e| error!("{:?}", e)),
                );
                recv
            };

            if let Err(e) = history_recv.recv() {
                error!("{:?}", e);
            } else {
                let read_at = info_recv.recv()?;
                let _ = sender.send(Event::HistoryLoaded {
                    server: server_name,
                    channel: conversation_name,
                    read_at: read_at.into(),
                });
            }
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

        let url = match channel_or_group_id {
            ::slack_api::ConversationId::Channel(channel_id) => {
                let req = channels::MarkRequest::new(channel_id, timestamp);
                format!(
                    "https://slack.com/api/{}?token={}&{}",
                    "channels.mark",
                    token,
                    ::serde_urlencoded::to_string(&req).unwrap()
                ).parse()
                .unwrap()
            }
            ::slack_api::ConversationId::Group(group_id) => {
                let req = groups::MarkRequest::new(group_id, timestamp);
                format!(
                    "https://slack.com/api/{}?token={}&{}",
                    "groups.mark",
                    token,
                    ::serde_urlencoded::to_string(&req).unwrap()
                ).parse()
                .unwrap()
            }
            ::slack_api::ConversationId::DirectMessage(dm_id) => {
                let req = im::MarkRequest::new(dm_id, timestamp);
                format!(
                    "https://slack.com/api/{}?token={}&{}",
                    "im.mark",
                    token,
                    ::serde_urlencoded::to_string(&req).unwrap()
                ).parse()
                .unwrap()
            }
        };

        #[derive(Deserialize)]
        struct SlackResponse {
            #[allow(dead_code)]
            ok: bool,
            error: Option<String>,
        }

        RUNTIME.lock().unwrap().spawn(
            HYPERCLIENT
                .get(url)
                .and_then(|resp| resp.into_body().concat2())
                .map(|body| ::serde_json::from_slice::<SlackResponse>(&body).unwrap())
                .map(|resp| {
                    if let Some(err) = resp.error {
                        error!("{}", err);
                    }
                }).map_err(|e| error!("{:?}", e)),
        );
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

        #[derive(Deserialize)]
        struct SlackResponse {
            #[allow(dead_code)]
            ok: bool,
            error: Option<String>,
        }

        use slack_api::http::reactions::Reactable;
        let request = ::slack_api::http::reactions::AddRequest::new(
            &name,
            Reactable::Message {
                channel,
                timestamp: timestamp.into(),
            },
        );

        let url = format!(
            "https://slack.com/api/{}?token={}&{}",
            "reactions.add",
            token,
            ::serde_urlencoded::to_string(&request).unwrap()
        ).parse()
        .unwrap();

        RUNTIME.lock().unwrap().spawn(
            HYPERCLIENT
                .get(url)
                .and_then(|resp| resp.into_body().concat2())
                .map(|body| ::serde_json::from_slice::<SlackResponse>(&body).unwrap())
                .map(|resp| {
                    if let Some(err) = resp.error {
                        error!("{}", err);
                    }
                }).map_err(|e| error!("{:?}", e)),
        );
    }
}
