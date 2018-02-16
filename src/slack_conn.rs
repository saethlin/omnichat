use std::sync::mpsc::Sender;
use std::thread;
use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message};
use conn::ConnError::SlackError;
use slack_api;
use failure::Error;
use websocket;
use serde_json;

//use tokio_core::reactor::Core;
/*
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures::sync::mpsc;
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};
use tokio_core::reactor::Handle;
*/
#[derive(Clone)]
struct Handler {
    channels: BiMap,
    users: BiMap,
    mention_patterns: Vec<(String, String)>,
    channel_patterns: Vec<(String, String)>,
    server_name: String,
}

impl Handler {
    pub fn to_omni(&self, message: slack_api::MessageStandard) -> Option<Message> {
        use slack_api::MessageStandard;
        if let MessageStandard {
            user: Some(user),
            text: Some(mut text),
            channel: Some(channel),
            ..
        } = message
        {
            for &(ref code, ref replacement) in &self.mention_patterns {
                text = text.replace(code, replacement);
            }

            for &(ref code, ref replacement) in &self.channel_patterns {
                text = text.replace(code, replacement);
            }

            if let Some(channel) = self.channels.get_human(&channel) {
                return Some(Message {
                    server: self.server_name.clone(),
                    channel: channel.clone(),
                    sender: self.users.get_human(&user).unwrap_or(&user).clone(),
                    contents: text,
                });
            } else {
                return None;
            }
        } else {
            None
        }
    }
}

pub struct SlackConn {
    token: String,
    team_name: String,
    users: BiMap,
    channels: BiMap,
    channel_names: Vec<String>,
    last_message_timestamp: String,
    client: slack_api::requests::Client,
    sender: Sender<Event>,
}

impl SlackConn {
    pub fn new(token: String, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        let client = slack_api::requests::Client::new()?;
        use slack_api::rtm::StartRequest;
        let response = slack_api::rtm::start(&client, &token, &StartRequest::default())?;

        // We use the team name as a unique name for the TUI tab and logs
        let team_name = response.team.ok_or(SlackError)?.name.ok_or(SlackError)?;

        // Slack users are identified by an internal ID
        // Create a HashMap so we can display their real name instead
        let members = response.users.ok_or(SlackError)?;
        let mut user_names = Vec::new();
        let mut user_ids = Vec::new();
        for member in members {
            if let slack_api::User {
                id: Some(id),
                name: Some(name),
                ..
            } = member
            {
                user_ids.push(id);
                user_names.push(name);
            } else {
                return Err(SlackError.into());
            }
        }

        let users = BiMap::new(BiMapBuilder {
            human: &user_names,
            id: &user_ids,
        });

        let mut mention_patterns = Vec::new();
        for (id, human) in user_ids.iter().zip(user_names.iter()) {
            mention_patterns.push((format!("<@{}>", id), format!("@{}", human)));
        }

        // We also need a map from channel names to internal ID, so that we can join and leave
        let mut channel_names = Vec::new();
        let mut channel_ids = Vec::new();
        for channel in response
            .channels
            .ok_or(SlackError)?
            .iter()
            .filter(|c| c.is_member.unwrap_or(false) && !c.is_archived.unwrap_or(true))
        {
            channel_names.push(channel.name.clone().ok_or(SlackError)?);
            channel_ids.push(channel.id.clone().ok_or(SlackError)?);
        }

        let channels = BiMap::new(BiMapBuilder {
            human: &channel_names,
            id: &channel_ids,
        });
        channel_names.sort();

        let mut channel_patterns = Vec::new();
        for (id, human) in channel_ids.iter().zip(channel_names.iter()) {
            channel_patterns.push((format!("<#{}|{}>", id, human), format!("#{}", human)));
        }

        let url = response.url.ok_or(SlackError)?;

        let mut websocket = websocket::ClientBuilder::new(&url)?.connect_secure(None)?;
        let thread_sender = sender.clone();

        let handler = Handler {
            channel_patterns: channel_patterns,
            mention_patterns: mention_patterns,
            channels: channels.clone(),
            users: users.clone(),
            server_name: team_name.clone(),
        };

        let thread_handler = handler.clone();

        /*
        use slack_api::Message::Standard;
        let runner = ClientBuilder::new(&url)
            .unwrap()
            .add_protocol("rust-websocket")
            .async_connect_secure(None, &handle)
            .and_then(|(duplex, _)| {
                let (sink, stream) = duplex.split();
                stream.filter_map(|message| {
                    match message {
                        OwnedMessage::Text(t) => {
                            if let Ok(Standard(slackmessage)) = serde_json::from_str::<slack_api::Message>(&t) {
                                if let Some(omnimessage) = thread_handler.to_omni(slackmessage) {
                                    thread_sender
                                        .send(Event::Message(omnimessage))
                                        .expect("Sender died");
                                }
                            }
                            None
                        }
                        // Close when we're told to
                        OwnedMessage::Close(e) => Some(OwnedMessage::Close(e)),
                        // Acknowledge Ping messages to keep the connection alive
                        OwnedMessage::Ping(d) => Some(OwnedMessage::Pong(d)),
                        _ => None,
                    }
                })
			    // merges with stdin
                //.select(stdin_ch.map_err(|_| WebSocketError::NoDataAvailable))
			    .forward(sink)
            })
            .map(|_| ()).map_err(|_| ())
        ;
        handle.spawn(runner);
        */

        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::OwnedMessage::{Ping, Pong, Text};
            use slack_api::Message::Standard;
            loop {
                let message = websocket.recv_message();
                if let Ok(Text(message)) = message {
                    // parse the message and add it to events
                    if let Ok(Standard(slackmessage)) =
                        serde_json::from_str::<slack_api::Message>(&message)
                    {
                        if let Some(omnimessage) = thread_handler.to_omni(slackmessage) {
                            thread_sender
                                .send(Event::Message(omnimessage))
                                .expect("Sender died")
                        }
                    }
                } else if let Ok(Ping(data)) = message {
                    websocket.send_message(&Pong(data)).unwrap_or_else(|_| {
                        thread_sender
                            .send(Event::Error("Failed to Pong".to_string()))
                            .expect("Sender died")
                    });
                }
            }
        });

        // Launch threads to populate the message history
        for id in channel_ids.iter().cloned() {
            let sender = sender.clone();
            let handler = handler.clone();
            let client = slack_api::requests::Client::new().unwrap();
            let token = token.clone();

            /*
            use slack_api::channels::{history, HistoryRequest};
            use slack_api::Message::Standard;
            let mut req = HistoryRequest::default();
            req.channel = &id;
            let runner = slack_api::requests::Client::new()
                .map_err(|e| e.into())
                .and_then(|client| history(&client, &token, &req))
                .map(|response| response.messages)
                .map(|messages| {
                for message in messages.iter().rev().cloned() {
                    if let Standard(mut slackmessage) = message {
                        slackmessage.channel = Some(id.clone());
                        if let Some(omnimessage) = handler.to_omni(slackmessage) {
                            sender
                                .send(Event::HistoryMessage(omnimessage))
                                .expect("Sender died");
                        }
                    }
                }                                             
            }).map_err(|_| ());
            handle.spawn(runner);
            */

            thread::spawn(move || {
                use slack_api::channels::{history, HistoryRequest};
                use slack_api::Message::Standard;
                let mut req = HistoryRequest::default();
                req.channel = &id;
                let response = history(&client, &token, &req).unwrap();
                for message in response.messages.unwrap().iter().rev().cloned() {
                    if let Standard(mut slackmessage) = message {
                        slackmessage.channel = Some(id.clone());
                        if let Some(omnimessage) = handler.to_omni(slackmessage) {
                            sender
                                .send(Event::HistoryMessage(omnimessage))
                                .expect("Sender died");
                        }
                    }
                }
            });
        }

        Ok(Box::new(SlackConn {
            token: token,
            client: client,
            users: users,
            channels: channels,
            channel_names: channel_names,
            team_name: team_name,
            last_message_timestamp: "".to_owned(),
            sender: sender,
        }))
    }
}

impl Conn for SlackConn {
    fn handle_cmd(&mut self, cmd: String, args: Vec<String>) {
        match (cmd.as_ref(), args.len()) {
            ("join", 1) => {
                use slack_api::channels::JoinRequest;
                //let channel_id = &self.channels.get(&args[0]).expect("Unknown channel");
                if let Err(e) = slack_api::channels::join(
                    &self.client,
                    &self.token,
                    &JoinRequest {
                        name: &args[0],
                        validate: Some(true),
                    },
                ) {
                    println!("{:#?}", e);
                    panic!("Join request failed");
                };
            }
            ("leave", 1) => {
                use slack_api::channels::LeaveRequest;
                let channel_id = &self.channels.get_id(&args[0]).expect("Unknown channel");
                if let Err(e) = slack_api::channels::leave(
                    &self.client,
                    &self.token,
                    &LeaveRequest {
                        channel: channel_id,
                    },
                ) {
                    println!("{:#?}", e);
                    panic!("Leave request failed");
                    // Notify tiny
                }
            }
            ("delete", 0) => {
                use slack_api::chat::DeleteRequest;
                let request = DeleteRequest {
                    ts: &self.last_message_timestamp,
                    channel: &"".to_owned(), // Get from the TUI?
                    as_user: Some(true),
                };
                let response = slack_api::chat::delete(&self.client, &self.token, &request);
                if response.is_err() {
                    // Notify tiny
                }
            }
            ("update", 1) => {
                use slack_api::chat::UpdateRequest;
                let request = UpdateRequest {
                    ts: &self.last_message_timestamp,
                    channel: &"".to_owned(), // Get from the TUI?
                    text: &args[0],
                    attachments: None,
                    parse: None,
                    link_names: None,
                    as_user: Some(true),
                };

                let response = slack_api::chat::update(&self.client, &self.token, &request);
                if response.is_err() {
                    // Notify tiny
                }
            }
            ("search", 1) => {
                use slack_api::search::{MessagesRequest, MessagesResponse,
                                        MessagesResponseMessages};
                let mut request = MessagesRequest::default();
                request.query = &args[0];
                let response = slack_api::search::messages(&self.client, &self.token, &request);
                if let Ok(MessagesResponse {
                    messages:
                        Some(MessagesResponseMessages {
                            matches: Some(_matches),
                            ..
                        }),
                    ..
                }) = response
                {
                    // Send stuff in matches to the TUI
                } else {
                    // Notify tiny
                }
            }
            ("users", 0) => {
                use slack_api::users::{ListRequest, ListResponse};
                let request = slack_api::users::list(
                    &self.client,
                    &self.token,
                    &ListRequest {
                        presence: Some(true),
                    },
                );

                if let Ok(ListResponse {
                    members: Some(members),
                    ..
                }) = request
                {
                    members
                        .iter()
                        .filter(|u| u.deleted.unwrap_or(true))
                        .filter_map(|user| user.name.clone())
                        // TODO This seems wrong?
                        .filter_map(|name| self.users.get_human(&name))
                        .for_each(
                            |_name| {}, // write to TUI
                        )
                } else {
                    // Notify tiny
                }
            }
            _ => {}
        }
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        use slack_api::chat::post_message;
        let mut request = slack_api::chat::PostMessageRequest::default();
        request.channel = channel;
        request.text = contents;
        request.as_user = Some(true);
        if post_message(&self.client, &self.token, &request).is_err() {
            if let Err(e) = post_message(&self.client, &self.token, &request) {
                self.sender
                    .send(Event::Error(format!("{:?}", e)))
                    .expect("Sender died");
            }
        }
    }

    fn channels(&self) -> Vec<&String> {
        self.channel_names.iter().collect()
    }

    fn autocomplete(&self, word: &str) -> Option<String> {
        match word.chars().next() {
            Some('#') => {
                // Autocomplete from channels
                Some(String::from("#channel_auto"))
            }
            Some('@') => {
                // Autocomplete from users
                Some(String::from("@user_auto"))
            }
            Some(':') => {
                // Autocomplete from emoji
                Some(String::from(":emoji_auto:"))
            }
            Some('+') => Some(String::from("+:emoji_auto:")),
            _ => None,
        }
    }

    fn name(&self) -> &String {
        &self.team_name
    }
}
