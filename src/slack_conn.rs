use bimap::BiMap;
use conn::ConnError::SlackError;
use conn::{Conn, Event, Message};
use failure::Error;
use regex::Regex;
use serde_json;
use slack_api;
use std::sync::mpsc::Sender;
use std::thread;
use websocket;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, Display)]
struct UserId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, Display)]
struct ChannelId(String);

lazy_static! {
    pub static ref MENTION_REGEX: Regex = Regex::new(r"<@U[A-Z0-9]{8}>").unwrap();
    pub static ref CHANNEL_REGEX: Regex = Regex::new(r"<#C[A-Z0-9]{8}\|(?P<n>.*?)>").unwrap();
}

#[derive(Clone)]
struct Handler {
    channels: BiMap<ChannelId, String>,
    users: BiMap<UserId, String>,
    server_name: String,
    my_mention: String,
    my_name: String,
}

impl Handler {
    pub fn to_omni(&self, message: slack_api::Message) -> Option<Message> {
        use slack_api::Message::*;
        use slack_api::{MessageBotMessage, MessageFileShare, MessageSlackbotResponse,
                        MessageStandard};
        // TODO: Add more success cases to this
        let (channel, user, mut text, ts) = match message {
            Standard(MessageStandard {
                user: Some(user),
                channel: Some(channel),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (channel, user, text, ts),
            BotMessage(MessageBotMessage {
                username: Some(name),
                channel: Some(channel),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (channel, name, text, ts),
            SlackbotResponse(MessageSlackbotResponse {
                user: Some(user),
                channel: Some(channel),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (channel, user, text, ts),
            FileShare(MessageFileShare {
                user: Some(user),
                channel: Some(channel),
                text: Some(text),
                ts: Some(ts),
                ..
            }) => (channel, user, text, ts),
            _ => return None,
        };

        text = text.replace("&amp;", "&");
        text = text.replace("&lt;", "<");
        text = text.replace("&gt;", ">");

        text = MENTION_REGEX
            .replace_all(&text, |caps: &::regex::Captures| {
                if let Some(name) = self.users.get_right(&caps[0][2..11].to_string().into()) {
                    format!("@{}", name)
                } else {
                    format!("@{}", &caps[0][2..11])
                }
            })
            .into_owned();

        text = CHANNEL_REGEX.replace_all(&text, "#$n").into_owned();

        let user = user.to_string();
        if let Some(channel) = self.channels.get_right(&channel.clone().into()) {
            return Some(Message {
                server: self.server_name.clone(),
                channel: channel.clone(),
                sender: self.users
                    .get_right(&UserId(user.clone()))
                    .unwrap_or(&user)
                    .clone(),
                is_mention: text.contains(&self.my_name),
                contents: text,
                timestamp: ts,
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

use std::sync::Arc;

pub struct SlackConn {
    token: String,
    team_name: String,
    users: BiMap<UserId, String>,
    channels: BiMap<ChannelId, String>,
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
                let client = slack_api::requests::Client::new()?;
                Ok(slack_api::rtm::connect(&client, &token)?)
            })
        };

        let users_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let client = slack_api::requests::Client::new()?;
                let users_response = slack_api::users::list(
                    &client,
                    &token,
                    &slack_api::users::ListRequest::default(),
                )?;

                let mut users = BiMap::new();
                for user in users_response.members.ok_or(SlackError)? {
                    if let slack_api::User {
                        id: Some(id),
                        name: Some(name),
                        ..
                    } = user
                    {
                        users.insert(UserId(id), name);
                    } else {
                        return Err(SlackError.into());
                    }
                }

                Ok(users)
            })
        };

        let channels_handle =
            {
                let token = token.clone();
                thread::spawn(move || -> Result<_, Error> {
                    let client = slack_api::requests::Client::new()?;
                    let channels = slack_api::channels::list(
                        &client,
                        &token,
                        &slack_api::channels::ListRequest::default(),
                    )?;

                    let mut channels_map = BiMap::new();
                    let mut channel_names = Vec::new();
                    for channel in
                        channels.channels.ok_or(SlackError)?.iter().filter(|c| {
                            c.is_member.unwrap_or(false) && !c.is_archived.unwrap_or(true)
                        }) {
                        let name = channel.name.clone().ok_or(SlackError)?;
                        let id = ChannelId(channel.id.clone().ok_or(SlackError)?);
                        channel_names.push(name.clone());
                        channels_map.insert(id, name);
                    }

                    Ok((channel_names, channels_map))
                })
            };

        let groups_handle = {
            let token = token.clone();
            thread::spawn(move || -> Result<_, Error> {
                let client = slack_api::requests::Client::new()?;
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
            .ok_or(SlackError)?
            .iter()
            .filter(|g| !g.is_archived.unwrap())
            .filter(|g| !g.is_mpim.unwrap())
        {
            let name = group.name.clone().ok_or(SlackError)?;
            let id = ChannelId(group.id.clone().ok_or(SlackError)?);
            channel_names.push(name.clone());
            channels.insert(id, name);
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
                                if let Some(omnimessage) = thread_handler.to_omni(message) {
                                    thread_sender.send(Event::Message(omnimessage)).unwrap()
                                }
                            }

                            Err(e) => thread_sender.send(omnierror!(e)).unwrap(),
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
                    Err(e) => thread_sender.send(omnierror!(e)).unwrap(),
                    _ => {}
                }
            }
        });

        // Launch threads to populate the message history
        for (channel_id, channel_name) in channels.clone().into_iter() {
            let sender = sender.clone();
            let handler = handler.clone();
            let client = slack_api::requests::Client::new().unwrap();
            let token = token.clone();
            let server_name = team_name.clone();

            let channel_id = format!("{}", channel_id);
            thread::spawn(move || {
                use slack_api::Message::{BotMessage, FileShare, SlackbotResponse, Standard};
                use slack_api::{channels, groups};

                let messages = if channel_id.starts_with('C') {
                    let mut req = channels::HistoryRequest::default();
                    req.channel = &channel_id;
                    match channels::history(&client, &token, &req) {
                        Ok(response) => response.messages.unwrap(),
                        Err(e) => {
                            sender.send(omnierror!(e)).unwrap();
                            Vec::new()
                        }
                    }
                } else if channel_id.starts_with('G') {
                    let mut req = groups::HistoryRequest::default();
                    req.channel = &channel_id;
                    match groups::history(&client, &token, &req) {
                        Ok(response) => response.messages.unwrap(),
                        Err(e) => {
                            sender.send(omnierror!(e)).unwrap();
                            Vec::new()
                        }
                    }
                } else {
                    sender
                        .send(Event::Error(format!(
                            "Don't understand this channel ID {}",
                            channel_id
                        )))
                        .unwrap();
                    Vec::new()
                };

                for mut message in messages.into_iter().rev() {
                    match message {
                        Standard(ref mut msg) => {
                            msg.channel = Some(channel_id.clone());
                        }
                        BotMessage(ref mut msg) => {
                            msg.channel = Some(channel_id.clone());
                        }
                        SlackbotResponse(ref mut msg) => {
                            msg.channel = Some(channel_id.clone());
                        }
                        FileShare(ref mut msg) => {
                            msg.channel = Some(channel_id.clone());
                        }
                        _ => {}
                    }
                    if let Some(message) = handler.to_omni(message) {
                        sender.send(Event::HistoryMessage(message)).unwrap();
                    }
                }
                sender
                    .send(Event::HistoryLoaded {
                        server: server_name,
                        channel: channel_name,
                    })
                    .unwrap();
            });
        }

        Ok(Box::new(SlackConn {
            token: token.to_string(),
            client: slack_api::requests::Client::new()?,
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

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }

    fn name(&self) -> &str {
        &self.team_name
    }

    fn mark_read(&self, channel: &str, _timestamp: Option<&str>) {
        use slack_api::{channels, groups};

        let channel_id = self.channels.get_left(channel).expect("channel not found");
        let channel_id_str = format!("{}", channel_id);

        let client = slack_api::requests::Client::new().unwrap();
        let token = self.token.clone();
        let sender = self.sender.clone();
        thread::spawn(move || {
            let unix_ts = ::chrono::offset::Local::now().timestamp() + 1;
            let ts = unix_ts.to_string();

            if channel_id_str.starts_with('C') {
                let request = channels::MarkRequest {
                    channel: &channel_id_str,
                    ts: &ts,
                };
                if let Err(e) = channels::mark(&client, &token, &request) {
                    sender.send(omnierror!(e)).unwrap();
                }
            } else if channel_id_str.starts_with('G') {
                let request = groups::MarkRequest {
                    channel: &channel_id_str,
                    ts: &ts,
                };
                if let Err(e) = groups::mark(&client, &token, &request) {
                    sender.send(omnierror!(e)).unwrap();
                }
            } else {
                sender
                    .send(Event::Error(format!(
                        "Don't understand this channel ID {}",
                        channel_id_str
                    )))
                    .unwrap();
            }
        });
    }
}
