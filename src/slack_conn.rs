use bimap::BiMap;
use conn::{Conn, Event, Message};
use failure::Error;
use regex::Regex;
use serde_json;
use slack_api;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use websocket;

lazy_static! {
    pub static ref MENTION_REGEX: Regex = Regex::new(r"<@U[A-Z0-9]{8}>").unwrap();
    pub static ref CHANNEL_REGEX: Regex = Regex::new(r"<#C[A-Z0-9]{8}\|(?P<n>.*?)>").unwrap();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ChannelOrGroupId {
    Channel(::slack_api::ChannelId),
    Group(::slack_api::GroupId),
}

use std::fmt;
impl fmt::Display for ChannelOrGroupId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChannelOrGroupId::Channel(c) => write!(f, "{}", c),
            ChannelOrGroupId::Group(g) => write!(f, "{}", g),
        }
    }
}

impl Into<ChannelOrGroupId> for ::slack_api::ChannelId {
    fn into(self) -> ChannelOrGroupId {
        ChannelOrGroupId::Channel(self)
    }
}

impl Into<ChannelOrGroupId> for ::slack_api::GroupId {
    fn into(self) -> ChannelOrGroupId {
        ChannelOrGroupId::Group(self)
    }
}

#[derive(Clone)]
struct Handler {
    channels: BiMap<ChannelOrGroupId, String>,
    users: BiMap<::slack_api::UserId, String>,
    server_name: String,
    my_mention: String,
    my_name: String,
}

impl Handler {
    pub fn to_omni(
        &self,
        message: slack_api::Message,
        outer_channel: Option<ChannelOrGroupId>,
    ) -> Option<Message> {
        use slack_api::Message::*;
        use slack_api::{
            MessageBotMessage, MessageFileShare, MessageSlackbotResponse, MessageStandard,
        };
        // TODO: Add more success cases to this
        let (channel, user, mut text, ts) = match message {
            Standard(MessageStandard {
                channel,
                user: Some(user),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (
                outer_channel.or(channel.map(|c| c.into())),
                self.users
                    .get_right(&user)
                    .unwrap_or(&user.to_string())
                    .clone(),
                text,
                ts,
            ),
            BotMessage(MessageBotMessage {
                channel,
                username: Some(name),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (outer_channel.or(channel.map(|c| c.into())), name, text, ts),
            SlackbotResponse(MessageSlackbotResponse {
                channel,
                user: Some(user),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (
                outer_channel.or(channel.map(|c| c.into())),
                self.users
                    .get_right(&user)
                    .unwrap_or(&user.to_string())
                    .clone(),
                text,
                ts,
            ),
            FileShare(MessageFileShare {
                channel,
                user: Some(user),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (
                outer_channel.or(channel.map(|c| c.into())),
                self.users
                    .get_right(&user)
                    .unwrap_or(&user.to_string())
                    .clone(),
                text,
                ts,
            ),
            _ => return None,
        };

        text = text.replace("&amp;", "&");
        text = text.replace("&lt;", "<");
        text = text.replace("&gt;", ">");

        text = MENTION_REGEX
            .replace_all(&text, |caps: &::regex::Captures| {
                if let Some(name) = self.users.get_right(&caps[0].as_bytes()[2..11].into()) {
                    format!("@{}", name)
                } else {
                    format!("@{}", &caps[0][2..11])
                }
            })
            .into_owned();

        text = CHANNEL_REGEX.replace_all(&text, "#$n").into_owned();

        let user = user.to_string();
        if let Some(channel) = channel.and_then(|c| self.channels.get_right(&c)) {
            return Some(Message {
                server: self.server_name.clone(),
                channel: channel.clone(),
                sender: user,
                is_mention: text.contains(&self.my_name),
                contents: text,
                timestamp: ts.to_string(),
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
    team_name: String,
    users: BiMap<::slack_api::UserId, String>,
    channels: BiMap<ChannelOrGroupId, String>,
    channel_names: Vec<String>,
    client: slack_api::requests::Client,
    handler: Arc<Handler>,
    sender: Sender<Event>,
}

impl SlackConn {
    pub fn new(token: String, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        let connect_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<slack_api::rtm::ConnectResponse, Error> {
                let client = slack_api::requests::default_client()?;
                Ok(slack_api::rtm::connect(
                    &client,
                    &token,
                    &slack_api::rtm::ConnectRequest::default(),
                )?)
            })
        };

        let users_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let client = slack_api::requests::default_client()?;
                let users_response = slack_api::users::list(
                    &client,
                    &token,
                    &slack_api::users::ListRequest::default(),
                )?;

                let mut users = BiMap::new();
                for user in users_response.members {
                    users.insert(user.id, user.name);
                }

                Ok(users)
            })
        };

        let channels_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let client = slack_api::requests::default_client()?;
                let channels = slack_api::channels::list(
                    &client,
                    &token,
                    &slack_api::channels::ListRequest::default(),
                )?;

                let mut channels_map = BiMap::new();
                let mut channel_names = Vec::new();
                for channel in channels
                    .channels
                    .into_iter()
                    .filter(|c| c.is_member.unwrap_or(false) && !c.is_archived.unwrap_or(true))
                {
                    let slack_api::Channel { id, name, .. } = channel;
                    channel_names.push(name.clone());
                    channels_map.insert(ChannelOrGroupId::Channel(id), name);
                }

                Ok((channel_names, channels_map))
            })
        };

        let groups_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let client = slack_api::requests::default_client()?;
                Ok(slack_api::groups::list(
                    &client,
                    &token,
                    &slack_api::groups::ListRequest::default(),
                )?)
            })
        };

        // Slack private channels are actually groups
        let (mut channel_names, mut channels) = channels_handle.join().unwrap()?;
        for group in groups_handle
            .join()
            .unwrap()?
            .groups
            .into_iter()
            .filter(|g| !g.is_archived.unwrap())
            .filter(|g| !g.is_mpim.unwrap())
        {
            let slack_api::Group { id, name, .. } = group;
            channel_names.push(name.clone());
            channels.insert(ChannelOrGroupId::Group(id), name);
        }
        channel_names.sort();

        let users = users_handle.join().unwrap()?;

        let response = connect_handle.join().unwrap()?;

        let mut websocket = websocket::ClientBuilder::new(&response.url)?.connect_secure(None)?;

        let slf = response.slf;
        let team_name = response.team.name;

        let handler = Arc::new(Handler {
            channels: channels.clone(),
            users: users.clone(),
            server_name: team_name.clone(),
            my_name: slf.name.clone(),
            my_mention: format!("<@{}>", slf.id.clone()),
        });

        let thread_sender = sender.clone();
        let thread_handler = Arc::clone(&handler);

        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
            loop {
                match websocket.recv_message() {
                    Ok(Text(message)) => {
                        // parse the message and add it to events
                        match serde_json::from_str::<slack_api::Message>(&message) {
                            // Deserialized into a message, try to convert into an omnimessage
                            Ok(message) => {
                                if let Some(omnimessage) = thread_handler.to_omni(message, None) {
                                    thread_sender.send(Event::Message(omnimessage)).unwrap()
                                }
                            }
                            Err(e) => {
                                thread_sender.send(omnierror!(e)).unwrap();
                                thread_sender
                                    .send(Event::Error(message.to_string()))
                                    .unwrap();
                            }
                        }
                    }
                    Ok(Ping(data)) => {
                        websocket.send_message(&Pong(data)).unwrap_or_else(|_| {
                            thread_sender
                                .send(Event::Error("Failed to Pong".to_string()))
                                .unwrap()
                        });
                    }
                    Ok(Close(_)) => {
                        thread_sender
                            .send(Event::Error("Websocket closed".to_owned()))
                            .unwrap();
                    }
                    _ => {}
                }
            }
        });

        // Launch threads to populate the message history
        for (channel_or_group_id, channel_name) in channels.clone().into_iter() {
            let sender = sender.clone();
            let handler = handler.clone();
            let client = slack_api::requests::default_client().unwrap();
            let token = token.clone();
            let server_name = team_name.clone();

            thread::spawn(move || {
                use slack_api::{channels, groups};

                let (messages, unread_count) = match channel_or_group_id {
                    ChannelOrGroupId::Channel(channel_id) => {
                        let mut req = slack_api::channels::InfoRequest::default();
                        req.channel = channel_id;
                        let info = slack_api::channels::info(&client, &token, &req);
                        let unread_count = info.unwrap().channel.unread_count.unwrap();

                        let mut req = channels::HistoryRequest::default();
                        req.channel = channel_id;
                        let messages = match channels::history(&client, &token, &req) {
                            Ok(response) => response.messages,
                            Err(e) => {
                                sender.send(omnierror!(e)).unwrap();
                                Vec::new()
                            }
                        };
                        (messages, unread_count)
                    }
                    ChannelOrGroupId::Group(group_id) => {
                        let mut req = slack_api::groups::InfoRequest::default();
                        req.channel = group_id;
                        let info = slack_api::groups::info(&client, &token, &req);
                        let unread_count = info.unwrap().group.unread_count.unwrap();

                        let mut req = groups::HistoryRequest::default();
                        req.channel = group_id;
                        let messages = match groups::history(&client, &token, &req) {
                            Ok(response) => response.messages,
                            Err(e) => {
                                sender.send(omnierror!(e)).unwrap();
                                Vec::new()
                            }
                        };
                        (messages, unread_count)
                    }
                };

                for mut message in messages.into_iter().rev() {
                    if let Some(message) = handler.to_omni(message, Some(channel_or_group_id)) {
                        sender.send(Event::HistoryMessage(message)).unwrap();
                    }
                }
                sender
                    .send(Event::HistoryLoaded {
                        server: server_name,
                        channel: channel_name,
                        unread_count: unread_count as usize,
                    })
                    .unwrap();
            });
        }

        Ok(Box::new(SlackConn {
            token: token.to_string(),
            client: slack_api::requests::default_client()?,
            users,
            channels,
            channel_names,
            team_name,
            sender,
            handler,
        }))
    }
}

impl Conn for SlackConn {
    fn name(&self) -> &str {
        &self.team_name
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let contents = self.handler.to_slack(contents.to_string());
        use slack_api::chat::post_message;
        let mut request = slack_api::chat::PostMessageRequest::default();
        request.channel = channel;
        request.text = &contents;
        request.as_user = Some(true);
        if post_message(&self.client, &self.token, &request).is_err() {
            if let Err(e) = post_message(&self.client, &self.token, &request) {
                self.sender.send(omnierror!(e)).expect("Sender died");
            }
        }
    }

    fn mark_read(&self, channel: &str, _timestamp: Option<&str>) {
        use slack_api::{channels, groups};

        let channel_or_group_id = self
            .channels
            .get_left(channel)
            .expect("channel not found")
            .clone();

        let client = slack_api::requests::default_client().unwrap();
        let token = self.token.clone();
        let sender = self.sender.clone();
        thread::spawn(move || {
            let unix_ts = ::chrono::offset::Local::now().timestamp() + 1;
            let ts = unix_ts.to_string();

            match channel_or_group_id {
                ChannelOrGroupId::Channel(channel_id) => {
                    let request = channels::MarkRequest {
                        channel: channel_id,
                        ts: &ts,
                    };
                    if let Err(e) = channels::mark(&client, &token, &request) {
                        sender.send(omnierror!(e)).unwrap();
                    }
                }
                ChannelOrGroupId::Group(group_id) => {
                    let request = groups::MarkRequest {
                        channel: group_id,
                        ts: &ts,
                    };
                    if let Err(e) = groups::mark(&client, &token, &request) {
                        sender.send(omnierror!(e)).unwrap();
                    }
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
            _ => Vec::new(),
        }
    }
}
