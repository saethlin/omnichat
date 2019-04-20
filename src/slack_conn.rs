use crate::bimap::BiMap;
use crate::conn;
use crate::conn::{Completer, ConnEvent, IString, Message, TuiEvent};
use crate::DFAExtension;
use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use log::error;
use regex_automata::DenseDFA;
use serde_derive::Deserialize;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, RwLock};
use std::thread;

lazy_static::lazy_static! {
    static ref MENTION_REGEX_DATA: Vec<u16> = {
        let raw = include_bytes!("../mention_regex");
        raw.chunks_exact(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect()
    };
    pub static ref MENTION_REGEX: DenseDFA<&'static [u16], u16> = unsafe {
        DenseDFA::from_bytes(std::slice::from_raw_parts(
            MENTION_REGEX_DATA.as_ptr() as *const u8, MENTION_REGEX_DATA.len() * 2)
        )
    };

    static ref CHANNEL_REGEX_DATA: Vec<u16> = {
        let raw = include_bytes!("../channel_regex");
        raw.chunks_exact(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect()
    };
    pub static ref CHANNEL_REGEX: DenseDFA<&'static [u16], u16> = unsafe {
        DenseDFA::from_bytes(std::slice::from_raw_parts(
            CHANNEL_REGEX_DATA.as_ptr() as *const u8, CHANNEL_REGEX_DATA.len() * 2)
        )
    };
}

macro_rules! deserialize_or_log {
    ($response:expr, $type:ty) => {{
        if $response.status().is_success() {
            ::serde_json::from_slice::<$type>(&$response.bytes())
                .map_err(|e| error!("{}\n{:#?}", format_json(&$response.bytes()), e))
        } else {
            match ::serde_json::from_slice::<::slack::http::Error>(&$response.bytes()) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!(
                        "{}\n{:#?}",
                        std::str::from_utf8($response.bytes()).unwrap(),
                        e
                    );
                    Err(())
                }
            }
        }
    }};
}

fn format_json(text: &[u8]) -> String {
    ::serde_json::from_slice::<::serde_json::Value>(text)
        .and_then(|v| ::serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| String::from_utf8(text.to_vec()).unwrap_or_default())
}

fn slack_url<T: ::serde::Serialize>(endpoint: &'static str, token: &str, request: T) -> String {
    format!(
        "https://slack.com/api/{}?token={}&{}",
        endpoint,
        token,
        ::serde_urlencoded::to_string(request).unwrap_or_default()
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageAck {
    #[allow(unused)]
    ok: bool,
    reply_to: u32,
    text: String,
    ts: ::slack::Timestamp,
}

struct PendingMessage {
    id: u32,
    channel: IString,
}

impl SlackConn {
    pub fn convert_mentions(&self, original: &str) -> String {
        let mut text = original.to_string();
        while let Some(mention) = MENTION_REGEX.get_first(text.as_bytes()) {
            let mention = std::str::from_utf8(mention).unwrap();
            let replacement = if let Some(name) = self.users.get_right(&mention[2..11].into()) {
                format!("@{}", name)
            } else {
                format!("@{}", &mention[2..11])
            };
            text = text.replace(mention, &replacement);
        }

        while let Some(mention) = CHANNEL_REGEX.get_first(text.as_bytes()) {
            let mention = std::str::from_utf8(mention).unwrap();
            let name_start = mention.rfind('|').unwrap();
            let replacement = format!("#{}", &mention[name_start + 1..mention.len() - 1]);
            text = text.replace(mention, &replacement);
        }

        text
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
        if let Ok(ack) = ::serde_json::from_str::<MessageAck>(&message) {
            // Remove the message from pending messages
            if let Some(index) = self
                .pending_messages
                .iter()
                .position(|m| m.id == ack.reply_to)
            {
                let mut body = ack.text.clone();
                body = body.replace("&amp;", "&");
                body = body.replace("&lt;", "<");
                body = body.replace("&gt;", ">");

                let _ = self.tui_sender.send(ConnEvent::Message(Message {
                    channel: self.pending_messages[index].channel.clone(),
                    contents: self.convert_mentions(&body),
                    reactions: Vec::new(),
                    sender: self.my_name.clone(),
                    server: self.team_name.clone(),
                    timestamp: ack.ts.into(),
                }));
                self.pending_messages.swap_remove(index);
                return;
            }
        }

        use slack::rtm;
        match ::serde_json::from_str::<::slack::rtm::Event>(&message) {
            Ok(rtm::Event::ReactionAdded { item, reaction, .. }) => {
                use slack::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    let _ = self.tui_sender.send(ConnEvent::ReactionAdded {
                        server: self.team_name.clone(),
                        channel: channel.clone(),
                        timestamp: timestamp.into(),
                        reaction: reaction.into(),
                    });
                }
            }
            Ok(rtm::Event::ReactionRemoved { item, reaction, .. }) => {
                use slack::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    let _ = self.tui_sender.send(ConnEvent::ReactionRemoved {
                        server: self.team_name.clone(),
                        channel: channel.clone(),
                        timestamp: timestamp.into(),
                        reaction: reaction.into(),
                    });
                }
            }
            Ok(rtm::Event::Message {
                user,
                username,
                channel,
                text,
                ts,
                attachments,
                files,
                bot_id,
            }) => {
                if let Some(sender) = user
                    .and_then(|id| self.users.get_right(&id))
                    .cloned()
                    .or_else(|| username.map(IString::from))
                    .or_else(|| bot_id.map(|id| IString::from(id.as_str())))
                {
                    use std::fmt::Write;
                    let mut body = match text {
                        Some(ref t) => self.convert_mentions(t),
                        None => String::new(),
                    };

                    for f in &files {
                        let _ = write!(body, "\n{}", f.url_private);
                    }

                    for a in &attachments {
                        if let Some(ref title) = a.title {
                            let _ = write!(body, "\n{}", title);
                        }
                        if let Some(ref pretext) = a.pretext {
                            let _ = write!(body, "\n{}", pretext);
                        }
                        if let Some(ref text) = a.text {
                            let _ = write!(body, "\n{}", text);
                        }
                        for f in &a.files {
                            let _ = write!(body, "\n{}", f.url_private);
                        }
                    }

                    body = body.replace("&amp;", "&");
                    body = body.replace("&lt;", "<");
                    body = body.replace("&gt;", ">");

                    let contents = body.trim().to_string();

                    let _ = self.tui_sender.send(ConnEvent::Message(Message {
                        server: self.team_name.clone(),
                        channel: self
                            .channels
                            .get_right(&channel)
                            .cloned()
                            .unwrap_or_else(|| channel.to_string().into()),
                        sender,
                        timestamp: ts.into(),
                        reactions: Vec::new(),
                        contents,
                    }));
                }
            }
            Ok(rtm::Event::ChannelMarked { channel, ts, .. }) => {
                let _ = self.tui_sender.send(ConnEvent::MarkChannelRead {
                    server: self.team_name.clone(),
                    channel: self
                        .channels
                        .get_right(&channel.into())
                        .unwrap_or(&channel.as_str().into())
                        .clone(),
                    read_at: ts.into(),
                });
            }

            Ok(::slack::rtm::Event::GroupMarked { channel, ts, .. }) => {
                let _ = self.tui_sender.send(ConnEvent::MarkChannelRead {
                    server: self.team_name.clone(),
                    channel: self
                        .channels
                        .get_right(&channel.into())
                        .unwrap_or(&IString::from(channel.as_str()))
                        .clone(),
                    read_at: ts.into(),
                });
            }

            _ => {}
        }
    }
}

pub struct SlackConn {
    token: String,
    team_name: IString,
    users: BiMap<::slack::UserId, IString>,
    channels: BiMap<::slack::ConversationId, IString>,
    emoji: Vec<IString>,
    last_typing_message: chrono::DateTime<chrono::Utc>,
    my_name: IString,
    input_sender: ::futures::sync::mpsc::Sender<::websocket::OwnedMessage>,
    tui_sender: SyncSender<ConnEvent>,
    pending_messages: Vec<PendingMessage>,
}

pub struct SlackCompleter {
    inner: Arc<RwLock<SlackConn>>,
}

impl Completer for SlackCompleter {
    fn autocomplete(&self, word: &str) -> Vec<String> {
        self.inner.read().unwrap().autocomplete(word)
    }
}

impl SlackConn {
    pub fn create_on(token: &str, sender: SyncSender<ConnEvent>) -> Result<(), ()> {
        let mut client = ::weeqwest::Client::new();
        // Launch all of the requests
        use slack::http::{conversations, emoji, rtm, users};

        let emoji_recv = client.get(&slack_url("emoji.list", &token, &())).unwrap();
        let connect_recv = client.get(&slack_url("rtm.connect", &token, &())).unwrap();
        let users_recv = client
            .get(&slack_url("users.list", &token, users::ListRequest::new()))
            .unwrap();

        use slack::http::conversations::ChannelType::*;
        let mut req = conversations::ListRequest::new();
        req.types = vec![PublicChannel, PrivateChannel, Mpim, Im];
        let conversations_recv = client
            .get(&slack_url("conversations.list", &token, req))
            .unwrap();

        // We need to know about the users first so that we can digest the list of conversations
        let users_response = users_recv.wait().map_err(|e| error!("{:#?}", e))?;
        let users_response = deserialize_or_log!(users_response, users::ListResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let mut users: BiMap<::slack::UserId, IString> = BiMap::new();
        for user in users_response.members {
            users.insert(user.id, IString::from(user.name));
        }

        let conversations_response = conversations_recv.wait().map_err(|e| error!("{:#?}", e))?;
        let response_channels =
            deserialize_or_log!(conversations_response, conversations::ListResponse)
                .map_err(|e| error!("{:#?}", e))?;

        use slack::http::conversations::Conversation::*;
        let mut channels = BiMap::new();
        let mut channel_names: Vec<IString> = Vec::new();
        for (id, name) in response_channels
            .channels
            .into_iter()
            .filter_map(|channel| match channel {
                Channel {
                    id,
                    name,
                    is_member: true,
                    is_im: false,
                    is_mpim: false,
                    is_archived: false,
                    ..
                } => Some((id, name.into())),
                Group {
                    id,
                    name,
                    is_member: true,
                    is_im: false,
                    is_mpim: false,
                    is_archived: false,
                    ..
                } => Some((id, name.into())),
                DirectMessage { id, user, .. } => {
                    users.get_right(&user).map(|name| (id, name.clone()))
                }
                _ => None,
            })
        {
            let name: IString = name;
            channel_names.push(name.clone());
            channels.insert(id, name);
        }

        channel_names.sort();

        let connect_response = connect_recv.wait().map_err(|e| error!("{:#?}", e))?;
        let connect_response = deserialize_or_log!(connect_response, rtm::ConnectResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let websocket_url = connect_response.url.clone();

        let my_name = IString::from(connect_response.slf.name);
        let team_name = IString::from(connect_response.team.name);
        let (input_sender, input_channel) = mpsc::channel(0);

        // Give the emoji handle as long as possible to complete
        let emoji_response = emoji_recv.wait().map_err(|e| error!("{:#?}", e))?;
        let emoji = deserialize_or_log!(emoji_response, emoji::ListResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let mut emoji = emoji
            .emoji
            .unwrap_or_default()
            .keys()
            .map(|e| IString::from(e.as_str()))
            .collect::<Vec<_>>();
        emoji.sort();

        let connection = Arc::new(RwLock::new(SlackConn {
            token: String::from(token),
            users,
            channels: channels.clone(),
            team_name: team_name.clone(),
            emoji,
            last_typing_message: chrono::Utc::now(),
            my_name: my_name.clone(),
            input_sender,
            tui_sender: sender.clone(),
            pending_messages: Vec::new(),
        }));

        let (tui_send, tui_recv) = std::sync::mpsc::sync_channel(100);
        let _ = sender.send(ConnEvent::ServerConnected {
            name: team_name.clone(),
            channels: channel_names.clone(),
            completer: Some(Box::new(SlackCompleter {
                inner: connection.clone(),
            })),
            sender: tui_send,
        });

        let conn = connection.clone();
        // Create a background thread that will handle events from the TUI
        thread::spawn(move || {
            while let Ok(event) = tui_recv.recv() {
                match event {
                    TuiEvent::SendMessage {
                        channel, contents, ..
                    } => conn
                        .write()
                        .unwrap()
                        .send_channel_message(&channel, &contents),
                    TuiEvent::SendTyping { channel, .. } => {
                        conn.write().unwrap().send_typing(&channel)
                    }
                    TuiEvent::MarkRead { channel, .. } => conn.read().unwrap().mark_read(&channel),
                    TuiEvent::Command {
                        channel, command, ..
                    } => conn.read().unwrap().handle_cmd(&channel, &command),
                    TuiEvent::AddReaction {
                        channel,
                        reaction,
                        timestamp,
                        ..
                    } => conn
                        .read()
                        .unwrap()
                        .add_reaction(&channel, &reaction, timestamp),
                }
            }
        });

        let thread_conn = connection.clone();

        thread::spawn(move || {
            use websocket::result::WebSocketError;
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
            let runner = ::websocket::ClientBuilder::new(&websocket_url)
                .unwrap()
                .async_connect_secure(None)
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
                                thread_conn.write().unwrap().process_slack_message(&text);
                                None
                            }
                            _ => None,
                        })
                        .select(input_channel.map_err(|_| WebSocketError::NoDataAvailable))
                        .forward(sink)
                });
            ::tokio::runtime::current_thread::block_on_all(runner).unwrap();
        });

        let mut pending_requests = Vec::new();

        // Launch all the history requests
        for (conversation_id, conversation_name) in channels.clone() {
            use slack::http::conversations;

            let url = format!(
                "https://slack.com/api/conversations.info?token={}&{}",
                token,
                ::serde_urlencoded::to_string(&conversations::InfoRequest::new(conversation_id))
                    .unwrap_or_default()
            );

            let info_response = client.get(&url).unwrap();

            let mut request = conversations::HistoryRequest::new(conversation_id);
            request.limit = Some(1000);
            let url = format!(
                "https://slack.com/api/conversations.history?token={}&{}",
                token,
                ::serde_urlencoded::to_string(request).unwrap_or_default()
            );

            let history_response = client.get(&url).unwrap();

            pending_requests.push((info_response, history_response, conversation_name));
        }

        // Handle all the launched requests
        for (info_response, history_response, conversation_name) in pending_requests {
            use slack::http::conversations::ConversationInfo;

            let info_response = info_response.wait().unwrap();
            let read_at = deserialize_or_log!(info_response, conversations::InfoResponse)
                .map(|info| match info.channel {
                    ConversationInfo::Channel { last_read, .. } => last_read
                        .map(|t| t.into())
                        .unwrap_or_else(conn::DateTime::now),
                    ConversationInfo::Group { last_read, .. } => last_read.into(),
                    ConversationInfo::ClosedDirectMessage { .. } => conn::DateTime::now(),
                    ConversationInfo::OpenDirectMessage { last_read, .. } => last_read.into(),
                })
                .unwrap_or_else(|_| conn::DateTime::now());

            let handle = connection.read().unwrap();
            // TODO: This is sometimes an error because url_private is missing???
            // The error message is awful and shouldn't even happen
            // TODO: replace the deserialize_or_log macro with something that reports errors
            // better?
            let history_response = history_response.wait().unwrap();
            let history_messages = deserialize_or_log!(history_response, HistoryResponse)
                .map(|h| h.messages)
                .unwrap_or_default();

            let messages = history_messages
                .into_iter()
                .map(|msg| {
                    let name = msg
                        .user
                        .and_then(|name| handle.users.get_right(&name).cloned())
                        .or_else(|| msg.username.clone())
                        .or_else(|| msg.bot_id.map(|b| IString::from(b.to_string())))
                        .unwrap_or_else(|| "UNKNOWNUSER".into());
                    Message {
                        server: team_name.clone(),
                        channel: conversation_name.clone(),
                        sender: name.clone(),
                        timestamp: msg.ts.into(),
                        reactions: msg
                            .reactions
                            .iter()
                            .map(|r| (r.name.clone(), r.count as usize))
                            .collect(),
                        contents: msg.to_omni(&handle),
                    }
                })
                .collect();

            let _ = sender.send(ConnEvent::HistoryLoaded {
                messages,
                server: team_name.clone(),
                channel: conversation_name.clone(),
                read_at,
            });
        }

        Ok(())
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

    fn send_typing(&mut self, channel: &str) {
        let now = chrono::Utc::now();
        if (now - self.last_typing_message) < chrono::Duration::seconds(3) {
            return;
        } else {
            self.last_typing_message = chrono::Utc::now();
        }
        let channel_id = match self.channels.get_left(channel) {
            Some(id) => *id,
            None => {
                error!("Unknown channel: {}", channel);
                return;
            }
        };

        let mut id = 0;
        while self.pending_messages.iter().any(|m| m.id == id) {
            id += 1;
        }
        self.pending_messages.push(PendingMessage {
            channel: IString::from(channel),
            id,
        });

        let message = ::serde_json::json!({
            "id": id,
            "type": "typing",
            "channel": channel_id,
        });

        let _ = ::serde_json::to_string(&message)
            .map_err(|e| error!("{:#?}", e))
            .and_then(|the_json| {
                self.input_sender
                    .clone()
                    .send(::websocket::OwnedMessage::Text(the_json))
                    .wait()
                    .map_err(|e| error!("{:#?}", e))
            });
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let contents = self.to_slack(contents.to_string());
        let channel_id = match self.channels.get_left(channel) {
            Some(id) => *id,
            None => {
                error!("Unknown channel: {}", channel);
                return;
            }
        };

        let mut id = 0;
        while self.pending_messages.iter().any(|m| m.id == id) {
            id += 1;
        }
        self.pending_messages.push(PendingMessage {
            channel: IString::from(channel),
            id,
        });

        let message = ::serde_json::json!({
            "id": id,
            "type": "message",
            "channel": channel_id,
            "text": contents,
        });

        let _ = ::serde_json::to_string(&message)
            .map_err(|e| error!("{:#?}", e))
            .and_then(|the_json| {
                self.input_sender
                    .clone()
                    .send(::websocket::OwnedMessage::Text(the_json))
                    .wait()
                    .map_err(|e| error!("{:#?}", e))
            });
    }

    fn mark_read(&self, channel: &str) {
        use slack::http::{channels, groups, im};

        let channel_or_group_id = match self.channels.get_left(channel) {
            Some(s) => *s,
            None => {
                error!(
                    "Tried to mark unread for channel {} in server {} but channel does not exist",
                    channel, self.team_name
                );
                return;
            }
        };

        let token = self.token.clone();
        let timestamp = conn::DateTime::now().into();

        let url = match channel_or_group_id {
            ::slack::ConversationId::Channel(channel_id) => {
                let req = channels::MarkRequest::new(channel_id, timestamp);
                slack_url("channels.mark", &token, req)
            }
            ::slack::ConversationId::Group(group_id) => {
                let req = groups::MarkRequest::new(group_id, timestamp);
                slack_url("groups.mark", &token, req)
            }
            ::slack::ConversationId::DirectMessage(dm_id) => {
                let req = im::MarkRequest::new(dm_id, timestamp);
                slack_url("im.mark", &token, req)
            }
        };

        let channel = channel.to_string();
        std::thread::spawn(move || {
            if let Ok(r) = ::weeqwest::post(&url).map_err(|e| error!("{:#?}", e)) {
                use slack::http::Error;
                if let Ok(Error { ok: false, error }) = ::serde_json::from_slice::<Error>(r.bytes())
                {
                    error!(
                        "Couldn't mark {} as read: {}",
                        channel,
                        error.unwrap_or_default()
                    );
                }
            }
        });
    }

    fn add_reaction(&self, channel: &str, reaction: &str, timestamp: conn::DateTime) {
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

        use slack::http::reactions::Reactable;
        let req = ::slack::http::reactions::AddRequest::new(
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
            ::serde_urlencoded::to_string(req).unwrap_or_default()
        );

        thread::spawn(move || {
            let _ = ::weeqwest::post(&url).map_err(|e| error!("{:#?}", e));
        });
    }

    fn handle_cmd(&self, channel: &str, cmd: &str) {
        let args: Vec<_> = cmd.split_whitespace().collect();
        match args.as_slice() {
            ["upload", path] => {
                error!("uploading {}", path);
                let url = match self.channels.get_left(channel).map(|id| {
                    format!(
                        "https://slack.com/api/files.upload?token={}&channels={}",
                        self.token, id,
                    )
                }) {
                    Some(v) => v,
                    None => {
                        error!("unknown channel {}", channel);
                        return;
                    }
                };

                let content = match std::fs::read(path) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("{:#?}", e);
                        return;
                    }
                };

                let req = ::weeqwest::Request::post(&url).unwrap().form(&[
                    ("content", &content),
                    ("filename", path.as_bytes()),
                    ("filetype", b"png"),
                ]);

                match ::weeqwest::send(&req) {
                    Ok(response) => {
                        if !response.status().is_success() {
                            error!("{:?}", std::str::from_utf8(response.bytes()))
                        }
                        error!("response: {:?}", std::str::from_utf8(response.bytes()));
                    }
                    Err(e) => error!("{:#?}", e),
                }
            }
            _ => {
                error!("unsupported command: {}", cmd);
            }
        }
    }
}

#[derive(Deserialize)]
struct Reaction {
    name: IString,
    count: u32,
}

#[derive(Deserialize)]
struct Attachment {
    pretext: Option<String>,
    text: Option<String>,
    title: Option<String>,
    #[serde(default)]
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    url_private: String,
}

#[derive(Deserialize)]
struct HistoryMessage {
    text: Option<String>,
    user: Option<slack::UserId>,
    username: Option<IString>,
    bot_id: Option<slack::BotId>,
    ts: slack::Timestamp,
    #[serde(default)]
    reactions: Vec<Reaction>,
    #[serde(default)]
    attachments: Vec<Attachment>,
    #[serde(default)]
    files: Vec<File>,
}

impl HistoryMessage {
    fn to_omni(&self, handler: &SlackConn) -> String {
        use std::fmt::Write;
        let mut body = match self.text {
            Some(ref t) => handler.convert_mentions(t),
            None => String::new(),
        };

        for f in &self.files {
            let _ = write!(body, "\n{}", f.url_private);
        }

        for a in &self.attachments {
            if let Some(ref title) = a.title {
                let _ = write!(body, "\n{}", title);
            }
            if let Some(ref pretext) = a.pretext {
                let _ = write!(body, "\n{}", pretext);
            }
            if let Some(ref text) = a.text {
                let _ = write!(body, "\n{}", text);
            }
            for f in &a.files {
                let _ = write!(body, "\n{}", f.url_private);
            }
        }

        body = body.replace("&amp;", "&");
        body = body.replace("&lt;", "<");
        body = body.replace("&gt;", ">");

        body.trim().to_string()
    }
}

#[derive(Deserialize)]
struct HistoryResponse {
    messages: Vec<HistoryMessage>,
}
