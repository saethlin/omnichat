use bimap::BiMap;
use conn::{Conn, Event, IString, Message};
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

macro_rules! deserialize_or_log {
    ($response:expr, $type:ty) => {{
        if $response.status.is_success() {
            ::serde_json::from_str::<$type>(&$response.text)
                .map_err(|e| error!("{}\n{:#?}", format_json(&$response.text), e))
        } else {
            match ::serde_json::from_str::<::discord::Error>(&$response.text) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!("{}\n{:#?}", $response.text, e);
                    Err(())
                }
            }
        }
    }};
}

struct Response {
    text: String,
    status: ::reqwest::StatusCode,
}

fn format_json(text: &str) -> String {
    ::serde_json::from_str::<::serde_json::Value>(text)
        .and_then(|v| ::serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| String::from(text))
}

use std::thread::JoinHandle;
fn get_slack<T, R>(endpoint: &'static str, token: &str, request: T) -> JoinHandle<Result<R, ()>>
where
    T: ::serde::Serialize + Send + 'static,
    R: ::serde::de::DeserializeOwned + Send + 'static,
{
    use slack::http::SlackError;
    let token = token.to_string();
    thread::spawn(move || {
        let url = format!(
            "https://slack.com/api/{}?token={}&{}",
            endpoint,
            token,
            ::serde_urlencoded::to_string(request).unwrap_or_default()
        ).parse::<::reqwest::Url>()
        .unwrap();

        CLIENT
            .get(url.clone())
            .send()
            .map_err(|e| error!("{:#?}", e))
            .and_then(|mut response| response.text().map_err(|e| error!("{:#?}", e)))
            .and_then(|body| {
                match ::serde_json::from_str::<SlackError>(&body)
                    .map_err(|e| error!("{}\n{:#?}", format_json(&body), e))
                {
                    Ok(SlackError { ok: true, .. }) => ::serde_json::from_str::<R>(&body)
                        .map_err(|e| error!("{}\n{:#?}", format_json(&body), e)),
                    Ok(SlackError { ok: false, error }) => Err(error!(
                        "{}",
                        error.unwrap_or_else(|| "no error given".into())
                    )),
                    Err(e) => Err(e),
                }
            })
    })
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

struct Handler {
    channels: BiMap<::slack::ConversationId, IString>,
    users: BiMap<::slack::UserId, IString>,
    server_name: IString,
    my_name: IString,
    input_sender: ::futures::sync::mpsc::Sender<::websocket::OwnedMessage>,
    tui_sender: SyncSender<Event>,
    pending_messages: Vec<PendingMessage>,
}

impl Handler {
    pub fn convert_mentions(&self, original: &str) -> String {
        let text = MENTION_REGEX
            .replace_all(original, |caps: &::regex::Captures| {
                if let Some(name) = self.users.get_right(&caps[0][2..11].into()) {
                    format!("@{}", name)
                } else {
                    format!("@{}", &caps[0][2..11])
                }
            }).into_owned();

        CHANNEL_REGEX.replace_all(&text, "#$n").into_owned()
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
                let _ = self.tui_sender.send(Event::Message(Message {
                    channel: self.pending_messages[index].channel.clone(),
                    contents: ack.text,
                    reactions: Vec::new(),
                    sender: self.my_name.clone(),
                    server: self.server_name.clone(),
                    timestamp: ack.ts.into(),
                }));
                self.pending_messages.swap_remove(index);
                return;
            }
        }

        use slack::rtm;
        match ::serde_json::from_str::<::slack::rtm::Event>(&message) {
            /*
            // TODO: Fix this by implementing the tiny slack variant
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
            */
            Ok(rtm::Event::ReactionAdded { item, reaction, .. }) => {
                use slack::rtm::Reactable;
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
                use slack::rtm::Reactable;
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
            Ok(rtm::Event::Message {
                user,
                username,
                channel,
                text,
                ts,
                attachments,
                files,
                ..
            }) => {
                if let Some(sender) = user
                    .and_then(|id| self.users.get_right(&id))
                    .cloned()
                    .or_else(|| username.map(IString::from))
                {
                    use std::fmt::Write;
                    let mut body = match text {
                        Some(ref t) => self.convert_mentions(t),
                        None => String::new(),
                    };

                    for f in &files {
                        write!(body, "\n{}", f.url_private);
                    }

                    for a in &attachments {
                        if let Some(ref title) = a.title {
                            write!(body, "\n{}", title);
                        }
                        if let Some(ref pretext) = a.pretext {
                            write!(body, "\n{}", pretext);
                        }
                        if let Some(ref text) = a.text {
                            write!(body, "\n{}", text);
                        }
                        for f in &a.files {
                            write!(body, "\n{}", f.url_private);
                        }
                    }

                    body = body.replace("&amp;", "&");
                    body = body.replace("&lt;", "<");
                    body = body.replace("&gt;", ">");

                    let contents = body.trim().to_string();

                    let _ = self.tui_sender.send(Event::Message(Message {
                        server: self.server_name.clone(),
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

            Ok(::slack::rtm::Event::GroupMarked { channel, ts, .. }) => {
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

            _ => {}
        }
    }
}

pub struct SlackConn {
    token: String,
    team_name: IString,
    users: BiMap<::slack::UserId, IString>,
    channels: BiMap<::slack::ConversationId, IString>,
    channel_names: Vec<IString>,
    handler: Arc<RwLock<Handler>>,
    _sender: SyncSender<Event>,
    emoji: Vec<IString>,
}

impl SlackConn {
    pub fn create_on(token: &str, sender: SyncSender<Event>) -> Result<(), ()> {
        // Launch all of the request
        use slack::http::{conversations, emoji, rtm, users};
        let emoji_recv = get_slack("emoji.list", &token, &());
        let connect_recv = get_slack("rtm.connect", &token, &());
        let users_recv = get_slack("users.list", &token, users::ListRequest::new());

        use slack::http::conversations::ChannelType::*;
        let mut req = conversations::ListRequest::new();
        req.types = vec![PublicChannel, PrivateChannel, Mpim, Im];
        let conversations_recv = get_slack("conversations.list", &token, req);

        // We need to know about the users first so that we can digest the list of conversations
        let users_response: users::ListResponse = users_recv.join().unwrap()?;

        let mut users: BiMap<::slack::UserId, IString> = BiMap::new();
        for user in users_response.members {
            users.insert(user.id, IString::from(user.name));
        }

        let response_channels: conversations::ListResponse = conversations_recv.join().unwrap()?;

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
                /*
                DirectMessage { id, user, .. } => {
                    users.get_right(&user).map(|name| (id, name.clone()))
                }
                */
                _ => None,
            }) {
            let name: IString = name;
            channel_names.push(name.clone());
            channels.insert(id, name);
        }

        channel_names.sort();

        let connect_response: rtm::ConnectResponse = connect_recv.join().unwrap()?;

        let websocket_url = connect_response.url.clone();

        let my_name = IString::from(connect_response.slf.name);
        let team_name = IString::from(connect_response.team.name);
        let (input_sender, input_channel) = mpsc::channel(0);

        let handler = Arc::new(RwLock::new(Handler {
            channels: channels.clone(),
            users: users.clone(),
            server_name: team_name.clone(),
            my_name: my_name.clone(),
            input_sender,
            tui_sender: sender.clone(),
            pending_messages: Vec::new(),
        }));

        // Give the emoji handle as long as possible to complete
        let emoji: emoji::ListResponse = emoji_recv.join().unwrap()?;

        let mut emoji = emoji
            .emoji
            .unwrap_or_default()
            .keys()
            .map(|e| IString::from(e.as_str()))
            .collect::<Vec<_>>();
        emoji.sort();

        let _ = sender.send(Event::Connected(Box::new(SlackConn {
            token: String::from(token),
            users,
            channels: channels.clone(),
            channel_names,
            team_name: team_name.clone(),
            _sender: sender.clone(),
            handler: handler.clone(),
            emoji,
        })));

        let thread_handler = Arc::clone(&handler);

        // Spin off a thread that will feed message events back to the TUI
        // websocket does not support the new tokio :(
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

        for (conversation_id, conversation_name) in channels.clone() {
            let token = token.to_string();
            let handler = Arc::clone(&handler);
            let sender = sender.clone();
            let team_name = team_name.clone();
            thread::spawn(move || {
                use slack::http::conversations;

                let url = format!(
                    "https://slack.com/api/conversations.info?token={}&{}",
                    token,
                    ::serde_urlencoded::to_string(&conversations::InfoRequest::new(
                        conversation_id
                    )).unwrap_or_default()
                ).parse::<::reqwest::Url>()
                .unwrap();

                let info_response = CLIENT
                    .get(url)
                    .send()
                    .map_err(|e| error!("{:#?}", e))
                    .map(|mut r| Response {
                        text: r.text().unwrap(),
                        status: r.status(),
                    }).unwrap();

                let info = deserialize_or_log!(info_response, conversations::InfoResponse).unwrap();
                use slack::http::conversations::ConversationInfo;
                let read_at = match info.channel {
                    ConversationInfo::Channel { last_read, .. } => last_read
                        .map(|t| t.into())
                        .unwrap_or_else(::conn::DateTime::now),
                    ConversationInfo::Group { last_read, .. } => last_read.into(),
                    ConversationInfo::ClosedDirectMessage { .. } => ::conn::DateTime::now(),
                    ConversationInfo::OpenDirectMessage { last_read, .. } => last_read.into(),
                };

                let mut request = conversations::HistoryRequest::new(conversation_id);
                request.limit = Some(1000);
                let url = format!(
                    "https://slack.com/api/conversations.history?token={}&{}",
                    token,
                    ::serde_urlencoded::to_string(request).unwrap_or_default()
                ).parse::<::reqwest::Url>()
                .unwrap();

                let history_response = CLIENT
                    .get(url)
                    .send()
                    .map_err(|e| error!("{:#?}", e))
                    .map(|mut r| Response {
                        text: r.text().unwrap(),
                        status: r.status(),
                    }).unwrap();

                let history = deserialize_or_log!(history_response, HistoryResponse).unwrap();

                let handle = handler.read().unwrap();
                history.messages.into_iter().rev().for_each(|msg| {
                    if let Some(name) = msg
                        .user
                        .and_then(|name| handle.users.get_right(&name).cloned())
                        .or_else(|| msg.username.clone())
                        .or_else(|| msg.bot_id.map(|b| IString::from(b.to_string())))
                    {
                        let _ = sender.send(Event::Message(Message {
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
                        }));
                    }
                });

                let _ = sender.send(Event::HistoryLoaded {
                    server: team_name.clone(),
                    channel: conversation_name,
                    read_at,
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
        use slack::http::{channels, groups, im};

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

        let timestamp = ::conn::DateTime::now().into();

        use slack::http::SlackError;
        match channel_or_group_id {
            ::slack::ConversationId::Channel(channel_id) => {
                let req = channels::MarkRequest::new(channel_id, timestamp);
                let _ =
                    get_slack::<channels::MarkRequest, SlackError>("channels.mark", &token, req);
            }
            ::slack::ConversationId::Group(group_id) => {
                let req = groups::MarkRequest::new(group_id, timestamp);
                let _ = get_slack::<groups::MarkRequest, SlackError>("groups.mark", &token, req);
            }
            ::slack::ConversationId::DirectMessage(dm_id) => {
                let req = im::MarkRequest::new(dm_id, timestamp);
                let _ = get_slack::<im::MarkRequest, SlackError>("im.mark", &token, req);
            }
        }
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
        ).parse::<::reqwest::Url>()
        .unwrap();

        let _ = CLIENT.post(url).send().map_err(|e| error!("{:#?}", e));
    }

    fn handle_cmd(&mut self, channel: &str, cmd: &str) {
        let args: Vec<_> = cmd.split_whitespace().collect();
        match args.as_slice() {
            ["upload", path] => {
                let url = self.channels.get_left(channel).map(|id| {
                    format!(
                        "https://slack.com/api/files.upload?token={}&channels={}",
                        self.token, id
                    )
                });
                let form = reqwest::multipart::Form::new()
                    .file("file", path)
                    .map_err(|e| error!("{:#?}", e));

                thread::spawn(move || {
                    if let (Some(url), Ok(form)) = (url, form) {
                        CLIENT
                            .post(&url)
                            .multipart(form)
                            .send()
                            .map(|r| {
                                if !r.status().is_success() {
                                    error!("{:#?}", r);
                                }
                            }).unwrap_or_else(|e| error!("{:#?}", e));
                    }
                });
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
    fn to_omni(&self, handler: &Handler) -> String {
        use std::fmt::Write;
        let mut body = match self.text {
            Some(ref t) => handler.convert_mentions(t),
            None => String::new(),
        };

        for f in &self.files {
            write!(body, "\n{}", f.url_private);
        }

        for a in &self.attachments {
            if let Some(ref title) = a.title {
                write!(body, "\n{}", title);
            }
            if let Some(ref pretext) = a.pretext {
                write!(body, "\n{}", pretext);
            }
            if let Some(ref text) = a.text {
                write!(body, "\n{}", text);
            }
            for f in &a.files {
                write!(body, "\n{}", f.url_private);
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
