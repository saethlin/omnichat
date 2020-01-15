use crate::bimap::BiMap;
use crate::conn;
use crate::conn::{ChannelType, Completer, ConnEvent, Message, TuiEvent};
use crate::DFAExtension;
use log::error;
use regex_automata::DenseDFA;
use serde::Deserialize;
use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;
use futures::future::FutureExt;
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

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
            ::serde_json::from_slice::<$type>(&$response.body())
                .map_err(|e| error!("{}\n{:#?}", format_json(&$response.body()), e))
        } else {
            match ::serde_json::from_slice::<::slack::http::Error>(&$response.body()) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!(
                        "{}\n{:#?}",
                        std::str::from_utf8($response.body()).unwrap(),
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
    channel: String,
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

        text = text.replace("<!here>", "@here");
        text = text.replace("<!channel>", "@channel");
        text = text.replace("<!everyone>", "@everyone");
        text = text.replace("&amp;", "&");
        text = text.replace("&lt;", "<");
        text = text.replace("&gt;", ">");

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

        text = text.replace("@here", "<!here>");
        text = text.replace("@channel", "<!channel>");
        text = text.replace("@everyone", "<!everyone>");

        text
    }

    pub async fn process_slack_message(&mut self, message: &str) {
        if let Ok(ack) = ::serde_json::from_str::<MessageAck>(&message) {
            // Remove the message from pending messages
            if let Some(index) = self
                .pending_messages
                .iter()
                .position(|m| m.id == ack.reply_to)
            {
                self.tui_sender
                    .send(ConnEvent::Message(Message {
                        channel: self.pending_messages[index].channel.clone(),
                        contents: self.convert_mentions(&ack.text),
                        reactions: Vec::new(),
                        sender: self.my_name.clone(),
                        server: self.team_name.clone(),
                        timestamp: ack.ts.into(),
                    }))
                    .await
                    .unwrap();
                self.pending_messages.swap_remove(index);
                return;
            }
        }

        let msg = serde_json::from_str::<slack::rtm::Event>(&message);

        use slack::rtm;
        match msg {
            Ok(rtm::Event::ReactionAdded { item, reaction, .. }) => {
                use slack::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    let _ = self
                        .tui_sender
                        .send(ConnEvent::ReactionAdded {
                            server: self.team_name.clone(),
                            channel: channel.clone(),
                            timestamp: timestamp.into(),
                            reaction,
                        })
                        .await
                        .unwrap();
                }
            }
            Ok(rtm::Event::ReactionRemoved { item, reaction, .. }) => {
                use slack::rtm::Reactable;
                let (channel_id, timestamp) = match item {
                    Reactable::Message { channel, ts } => (channel, ts),
                };
                if let Some(channel) = self.channels.get_right(&channel_id) {
                    self.tui_sender
                        .send(ConnEvent::ReactionRemoved {
                            server: self.team_name.clone(),
                            channel: channel.clone(),
                            timestamp: timestamp.into(),
                            reaction,
                        })
                        .await
                        .unwrap();
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
                message: edited_message,
            }) => {
                if let Some(edited_message) = edited_message {
                    // This check is how we verify that this is _actually_ an edit
                    if edited_message.edited.is_some() {
                        self.tui_sender
                            .send(ConnEvent::MessageEdited {
                                server: self.team_name.clone(),
                                channel: self
                                    .channels
                                    .get_right(&channel)
                                    .cloned()
                                    .unwrap_or_else(|| channel.to_string()),
                                contents: self
                                    .convert_mentions(&edited_message.text.unwrap_or_default()),
                                timestamp: edited_message.ts.into(),
                            })
                            .await
                            .unwrap();
                    }
                } else if let Some(sender) = user
                    .and_then(|id| self.users.get_right(&id))
                    .cloned()
                    .or_else(|| username.map(String::from))
                    .or_else(|| bot_id.map(|id| String::from(id.as_str())))
                {
                    use std::fmt::Write;
                    let mut body = match text {
                        Some(ref t) => self.convert_mentions(t),
                        None => String::new(),
                    };

                    for f in &files {
                        f.url_private.as_ref().map(|url| write!(body, "\n{}", url));
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
                            f.url_private.as_ref().map(|url| write!(body, "\n{}", url));
                        }
                    }

                    body = body.replace("&amp;", "&");
                    body = body.replace("&lt;", "<");
                    body = body.replace("&gt;", ">");

                    let contents = body.trim().to_string();

                    self.tui_sender
                        .send(ConnEvent::Message(Message {
                            server: self.team_name.clone(),
                            channel: self
                                .channels
                                .get_right(&channel)
                                .cloned()
                                .unwrap_or_else(|| channel.to_string()),
                            sender,
                            timestamp: ts.into(),
                            reactions: Vec::new(),
                            contents,
                        }))
                        .await
                        .unwrap();
                }
            }
            Ok(rtm::Event::ChannelMarked { channel, ts, .. }) => {
                self.tui_sender
                    .send(ConnEvent::MarkChannelRead {
                        server: self.team_name.clone(),
                        channel: self
                            .channels
                            .get_right(&channel.into())
                            .unwrap_or(&channel.as_str().into())
                            .clone(),
                        read_at: Some(ts.into()),
                        latest: None,
                    })
                    .await
                    .unwrap();
            }

            Ok(::slack::rtm::Event::GroupMarked { channel, ts, .. }) => {
                self.tui_sender
                    .send(ConnEvent::MarkChannelRead {
                        server: self.team_name.clone(),
                        channel: self
                            .channels
                            .get_right(&channel.into())
                            .unwrap_or(&String::from(channel.as_str()))
                            .clone(),
                        read_at: Some(ts.into()),
                        latest: None,
                    })
                    .await
                    .unwrap();
            }
            _ => {}
        }
    }
}

pub struct SlackConn {
    token: String,
    team_name: String,
    users: BiMap<::slack::UserId, String>,
    channels: BiMap<::slack::ConversationId, String>,
    emoji: Vec<String>,
    last_typing_message: chrono::DateTime<chrono::Utc>,
    my_name: String,
    input_sender: UnboundedSender<weebsocket::Message>,
    tui_sender: UnboundedSender<ConnEvent>,
    pending_messages: Vec<PendingMessage>,
}

pub struct SlackCompleter {
    inner: Arc<Mutex<SlackConn>>,
}

impl Completer for SlackCompleter {
    fn autocomplete(&self, _word: &str) -> Vec<String> {
        error!("Autocompleter was stubbed out!");
        Vec::new()
        //self.inner.unwrap().autocomplete(word)
    }
}

impl SlackConn {
    pub async fn create_on(token: &str, mut sender: UnboundedSender<ConnEvent>) -> Result<(), ()> {
        let token = token.to_string();
        // Launch all of the requests
        use slack::http::{conversations, emoji, rtm, users};

        let url = slack_url("emoji.list", &token, &());
        let emoji_recv = tokio::spawn(async move { weeqwest::get(&url).await });
        let url = slack_url("rtm.connect", &token, &());
        let connect_recv = tokio::spawn(async move { weeqwest::get(&url).await });
        let url = slack_url("users.list", &token, users::ListRequest::new());
        let users_recv = tokio::spawn(async move { weeqwest::get(&url).await });

        use slack::http::conversations::ChannelType::*;
        let mut req = conversations::ListRequest::new();
        req.types = vec![PublicChannel, PrivateChannel, Mpim, Im];
        let url = slack_url("conversations.list", &token, req);
        let conversations_recv = tokio::spawn(async move { weeqwest::get(&url).await });

        // We need to know about the users first so that we can digest the list of conversations
        let users_response = users_recv.await.unwrap().map_err(|e| error!("{:#?}", e))?;
        let users_response = deserialize_or_log!(users_response, users::ListResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let mut users: BiMap<::slack::UserId, String> = BiMap::new();
        for user in users_response.members {
            users.insert(user.id, user.name);
        }

        let conversations_response = conversations_recv
            .await
            .unwrap()
            .map_err(|e| error!("{:#?}", e))?;
        let response_channels =
            deserialize_or_log!(conversations_response, conversations::ListResponse)
                .map_err(|e| error!("{:#?}", e))?;

        use slack::http::conversations::Conversation::*;
        let mut channels = BiMap::new();
        let mut tui_channels = Vec::new();
        for (id, name, channel_type) in
            response_channels
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
                    } => Some((id, name, ChannelType::Normal)),
                    Group {
                        id,
                        name,
                        is_member: true,
                        is_im: false,
                        is_mpim: false,
                        is_archived: false,
                        ..
                    } => Some((id, name, ChannelType::Normal)),
                    DirectMessage { id, user, .. } => users
                        .get_right(&user)
                        .map(|name| (id, name.clone(), ChannelType::DirectMessage)),
                    _ => None,
                })
        {
            channels.insert(id, name.clone());
            let now = conn::DateTime::now();
            tui_channels.push(crate::tui::Channel {
                messages: Vec::new(),
                name,
                read_at: now,
                latest: now,
                has_history: false,
                message_scroll_offset: 0,
                message_buffer: String::new(),
                channel_type,
            });
        }

        let connect_response = connect_recv
            .await
            .unwrap()
            .map_err(|e| error!("{:#?}", e))?;
        let connect_response = deserialize_or_log!(connect_response, rtm::ConnectResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let websocket_url = connect_response.url.clone();
        let mut websocket = weebsocket::Client::connect(&websocket_url).await.unwrap();

        let my_name = connect_response.slf.name;
        let team_name = connect_response.team.name;

        // Give the emoji handle as long as possible to complete
        let emoji_response = emoji_recv.await.unwrap().map_err(|e| error!("{:#?}", e))?;
        let emoji = deserialize_or_log!(emoji_response, emoji::ListResponse)
            .map_err(|e| error!("{:#?}", e))?;

        let mut emoji = emoji
            .emoji
            .unwrap_or_default()
            .keys()
            .map(|e| String::from(e.as_str()))
            .collect::<Vec<_>>();
        emoji.sort();

        let (input_sender, mut input_reciever) = futures::channel::mpsc::unbounded();

        let connection = Arc::new(Mutex::new(SlackConn {
            token: token.clone(),
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

        let (tui_send, mut tui_recv) = futures::channel::mpsc::unbounded();

        sender
            .send(ConnEvent::ServerConnected(crate::tui::Server {
                current_channel: 0,
                channels: tui_channels,
                completer: Some(Box::new(SlackCompleter {
                    inner: connection.clone(),
                })),
                name: team_name.clone(),
                channel_scroll_offset: 0,
                sender: tui_send,
            }))
            .await
            .unwrap();

        let conn = connection.clone();
        // Create a background thread that will handle events from the TUI
        tokio::spawn(async move {
            while let Some(event) = tui_recv.next().await {
                match event {
                    TuiEvent::SendMessage {
                        channel, contents, ..
                    } => {
                        conn.lock()
                            .await
                            .send_channel_message(&channel, &contents)
                            .await
                    }
                    TuiEvent::SendTyping { channel, .. } => {
                        conn.lock().await.send_typing(&channel).await
                    }
                    TuiEvent::MarkRead { channel, .. } => conn.lock().await.mark_read(&channel),
                    TuiEvent::Command {
                        channel, command, ..
                    } => conn.lock().await.handle_cmd(&channel, &command),
                    TuiEvent::AddReaction {
                        channel,
                        reaction,
                        timestamp,
                        ..
                    } => conn
                        .lock()
                        .await
                        .add_reaction(&channel, &reaction, timestamp),
                    TuiEvent::GetHistory { channel } => {
                        conn.lock().await.get_history(&channel).await
                    }
                }
            }
        });

        let thread_conn = connection.clone();

        tokio::spawn(async move {
            loop {
                let mut input_fut = input_reciever.next();
                futures::select! {
                    ws_msg = websocket.recv().fuse() => {
                        match ws_msg.unwrap() {
                            weebsocket::Message::Close(_) => {
                                error!("websocket closed");
                                // TODO: Restart
                            }
                            weebsocket::Message::Ping(data) => websocket.send(&weebsocket::Message::Pong(data)).await.unwrap(),
                            weebsocket::Message::Text(text) => {
                                thread_conn.lock().await.process_slack_message(&text).await;
                            }
                            weebsocket::Message::Pong(_) | weebsocket::Message::Binary(_) => error!("unrecognized message"),
                        }
                    }
                    input = input_fut => {
                        if let Some(input) = input {
                            websocket.send(&input).await.unwrap();
                        }
                    }
                }
                tokio::task::yield_now().await;
            }
        });

        let mut pending_requests = Vec::new();

        // Launch all the history requests
        for (conversation_id, conversation_name) in channels.clone() {
            use slack::http::{channels, groups};

            let url = match conversation_id {
                ::slack::ConversationId::Channel(channel_id) => {
                    let req = channels::InfoRequest::new(channel_id);
                    slack_url("channels.info", &token, req)
                }
                ::slack::ConversationId::Group(group_id) => {
                    let req = groups::InfoRequest::new(group_id);
                    slack_url("groups.info", &token, req)
                }
                ::slack::ConversationId::DirectMessage(_) => {
                    let req = conversations::InfoRequest::new(conversation_id);
                    slack_url("conversations.info", &token, req)
                }
            };

            let name = conversation_name.clone();
            let info_response = tokio::spawn(async move {
                let mut res = weeqwest::get(&url).await;
                // TODO: This is not a serious retry loop, should eventually be exponential backoff
                while res.is_err() {
                    error!("retrying info for {:?}", name);
                    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
                    res = weeqwest::get(&url).await;
                }
                res
            });

            pending_requests.push((info_response, conversation_id, conversation_name));
        }

        // Handle all the launched requests
        for (info_response, conversation_id, conversation_name) in pending_requests {
            use slack::http::{channels, groups};
            use slack::ConversationId::*;

            let info_response = info_response.await.unwrap().unwrap();

            let (read_at, latest) = match conversation_id {
                Channel(_) => {
                    let info = deserialize_or_log!(info_response, channels::InfoResponse)?;
                    (
                        info.channel.last_read.unwrap().into(),
                        info.channel.latest.ts.into(),
                    )
                }
                Group(_) => {
                    let info = deserialize_or_log!(info_response, groups::InfoResponse)?;
                    (
                        info.group.last_read.unwrap().into(),
                        info.group.latest.ts.into(),
                    )
                }
                DirectMessage(_) => {
                    let info = deserialize_or_log!(info_response, conversations::InfoResponse)?;
                    match info.channel {
                        slack::http::conversations::ConversationInfo::DirectMessage {
                            last_read,
                            latest,
                            ..
                        } => {
                            let last_read = last_read.into();
                            let latest = latest.map(|l| l.ts.into()).unwrap_or(last_read);
                            (last_read, latest)
                        }
                        _ => {
                            error!(
                                "Tried to get info about a DM but got info about something else"
                            );
                            continue;
                        }
                    }
                }
            };

            sender
                .send(ConnEvent::MarkChannelRead {
                    server: team_name.clone(),
                    channel: conversation_name.clone(),
                    read_at: Some(read_at),
                    latest: Some(latest),
                })
                .await
                .unwrap();
        }

        Ok(())
    }

    fn autocomplete(&self, word: &str) -> Vec<String> {
        match word.chars().next() {
            Some('@') => self
                .users
                .iter()
                .map(|(_, name)| name)
                .chain(&[
                    String::from("channel"),
                    String::from("here"),
                    String::from("everyone"),
                ])
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

    async fn get_history(&self, channel: &str) {
        let mut request = slack::http::conversations::HistoryRequest::new(
            *self.channels.get_left(channel).unwrap(),
        );
        request.limit = Some(1000);
        let url = format!(
            "https://slack.com/api/conversations.history?token={}&{}",
            self.token,
            ::serde_urlencoded::to_string(request).unwrap_or_default()
        );

        let history_response = weeqwest::get(&url).await.unwrap();

        let history_messages = deserialize_or_log!(history_response, HistoryResponse)
            .map(|h| h.messages)
            .unwrap_or_default();

        let messages = history_messages
            .into_iter()
            .map(|msg| {
                let name = msg
                    .user
                    .and_then(|name| self.users.get_right(&name).cloned())
                    .or_else(|| msg.username.clone())
                    .or_else(|| msg.bot_id.map(|b| b.to_string()))
                    .unwrap_or_else(|| "UNKNOWNUSER".into());
                Message {
                    server: self.team_name.clone(),
                    channel: channel.to_string(),
                    sender: name.clone(),
                    timestamp: msg.ts.into(),
                    reactions: msg
                        .reactions
                        .iter()
                        .map(|r| (r.name.clone(), r.count as usize))
                        .collect(),
                    contents: msg.to_omni(self),
                }
            })
            .collect();

        let mut tui_send = self.tui_sender.clone();
        tui_send
            .send(ConnEvent::HistoryLoaded {
                messages,
                server: self.team_name.clone(),
                channel: channel.to_string(),
            })
            .await
            .unwrap();
    }

    async fn send_typing(&mut self, channel: &str) {
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
            channel: String::from(channel),
            id,
        });

        let message = ::serde_json::json!({
            "id": id,
            "type": "typing",
            "channel": channel_id,
        });

        match serde_json::to_string(&message) {
            Err(e) => error!("{:#?}", e),
            Ok(the_json) => self
                .input_sender
                .send(weebsocket::Message::Text(the_json))
                .await
                .unwrap(),
        }
    }

    async fn send_channel_message(&mut self, channel: &str, contents: &str) {
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
            channel: String::from(channel),
            id,
        });

        let message = ::serde_json::json!({
            "id": id,
            "type": "message",
            "channel": channel_id,
            "text": contents,
        });

        match serde_json::to_string(&message) {
            Err(e) => error!("{:#?}", e),
            Ok(the_json) => self
                .input_sender
                .send(weebsocket::Message::Text(the_json))
                .await
                .unwrap(),
        }
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
        tokio::spawn(async move {
            if let Ok(r) = weeqwest::post(&url).await.map_err(|e| error!("{:#?}", e)) {
                use slack::http::Error;
                if let Ok(Error { ok: false, error }) = ::serde_json::from_slice::<Error>(r.body())
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
        let name = String::from(reaction);

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

        // TODO: Pretty sure I can just call slack_url here
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

        tokio::spawn(async move {
            let _ = weeqwest::post(&url).await.map_err(|e| error!("{:#?}", e));
        });
    }

    fn handle_cmd(&self, channel: &str, cmd: &str) {
        let args: Vec<_> = cmd.split_whitespace().collect();
        match args.as_slice() {
            ["upload", path] => {
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

                let req = weeqwest::Request::post(&url)
                    .unwrap()
                    .file_form(path, &content);

                tokio::spawn(
                    async move { weeqwest::send(&req).await.map_err(|e| error!("{:#?}", e)) },
                );
            }
            _ => {
                error!("unsupported command: {}", cmd);
            }
        }
    }
}

#[derive(Deserialize)]
struct Reaction {
    name: String,
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
    url_private: Option<String>,
}

#[derive(Deserialize)]
struct HistoryMessage {
    text: Option<String>,
    user: Option<slack::UserId>,
    username: Option<String>,
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
            if let Some(url) = &f.url_private {
                let _ = write!(body, "\n{}", url);
            }
        }

        for a in &self.attachments {
            if let Some(ref title) = a.title {
                let _ = write!(body, "\n{}", title);
            }
            if let Some(ref pretext) = a.pretext {
                let _ = write!(body, "\n{}", pretext);
            }
            if let Some(ref text) = a.text {
                let mut it = text.splitn(2, '\n');
                let _ = write!(body, "\n{}", it.next().unwrap_or_default());
                if it.next().is_some() {
                    body.extend("\n...".chars());
                }
            }
            for f in &a.files {
                if let Some(url) = &f.url_private {
                    let _ = write!(body, "\n{}", url);
                }
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
