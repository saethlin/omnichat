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
        use slack_api::{MessageBotMessage, MessageSlackbotResponse, MessageStandard};
        // TODO: Add more success cases to this
        let (channel, user, mut text) = match message {
            Standard(MessageStandard {
                user: Some(user),
                channel: Some(channel),
                text: Some(text),
                ..
            }) => (channel, user, text),
            BotMessage(MessageBotMessage {
                username: Some(name),
                channel: Some(channel),
                text: Some(text),
                ..
            }) => (channel, name, text),
            SlackbotResponse(MessageSlackbotResponse {
                user: Some(user),
                channel: Some(channel),
                text: Some(text),
                ..
            }) => (channel, user, text),
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
        for group in groups_handle.join().unwrap()?
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
                use slack_api::channels::{history, HistoryRequest};
                use slack_api::Message::{BotMessage, SlackbotResponse, Standard};
                let mut req = HistoryRequest::default();
                req.channel = &channel_id;
                let response = history(&client, &token, &req);
                match response {
                    // This is a disgusting hack to handle how slack treats private channels as groups
                    Err(slack_api::channels::HistoryError::ChannelNotFound) => {
                        let mut req = slack_api::groups::HistoryRequest::default();
                        req.channel = &channel_id;
                        match slack_api::groups::history(&client, &token, &req) {
                            Ok(response) => {
                                for message in response
                                    .messages
                                    .unwrap()
                                    .iter()
                                    .rev()
                                    .cloned()
                                    .map(|m| match m {
                                        Standard(mut msg) => {
                                            msg.channel = Some(channel_id.clone());
                                            Standard(msg)
                                        }
                                        BotMessage(mut msg) => {
                                            msg.channel = Some(channel_id.clone());
                                            BotMessage(msg)
                                        }
                                        SlackbotResponse(mut msg) => {
                                            msg.channel = Some(channel_id.clone());
                                            SlackbotResponse(msg)
                                        }

                                        _ => m,
                                    })
                                    .filter_map(|m| handler.to_omni(m))
                                {
                                    sender
                                        .send(Event::HistoryMessage(message))
                                        .expect("Sender died");
                                }
                                sender
                                    .send(Event::HistoryLoaded {
                                        server: server_name,
                                        channel: format!("{}", channel_name),
                                    })
                                    .expect("Sender died");
                            }
                            Err(e) => {
                                sender.send(omnierror!(e)).unwrap();
                            }
                        }
                    }
                    Err(e) => {
                        sender.send(omnierror!(e)).unwrap();
                    }
                    Ok(response) => {
                        for message in response
                            .messages
                            .unwrap()
                            .into_iter()
                            .rev()
                            .map(|m| match m {
                                Standard(mut msg) => {
                                    msg.channel = Some(channel_id.clone());
                                    Standard(msg)
                                }
                                BotMessage(mut msg) => {
                                    msg.channel = Some(channel_id.clone());
                                    BotMessage(msg)
                                }
                                SlackbotResponse(mut msg) => {
                                    msg.channel = Some(channel_id.clone());
                                    SlackbotResponse(msg)
                                }
                                _ => m,
                            })
                            .filter_map(|m| handler.to_omni(m))
                        {
                            sender.send(Event::HistoryMessage(message)).unwrap();
                        }
                        sender
                            .send(Event::HistoryLoaded {
                                server: server_name,
                                channel: format!("{}", channel_name),
                            })
                            .unwrap();
                    }
                }
            });
        }

        Ok(Box::new(SlackConn {
            token: token.to_string(),
            client: slack_api::requests::Client::new()?,
            users: users,
            channels: channels,
            channel_names: channel_names,
            team_name: team_name,
            //last_message_timestamp: "".to_owned(),
            sender: sender,
            handler: handler,
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
}
