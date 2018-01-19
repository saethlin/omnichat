use std::sync::mpsc::Sender;
use std::thread;
use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message, ServerConfig};
use conn::ConnError::SlackError;
use slack_api;
use failure::Error;
use websocket;
use serde_json;

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

impl Conn for SlackConn {
    fn new(config: ServerConfig, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        let api_key = match config {
            ServerConfig::Slack { token } => token,
            //_ => return Err(Error::from(SlackError)),
        };

        let client = slack_api::requests::Client::new()?;
        use slack_api::rtm::StartRequest;
        let response = slack_api::rtm::start(&client, &api_key, &StartRequest::default())?;

        // We use the team name as a unique name for the TUI tab and logs
        let team_name = response.team.ok_or(SlackError)?.name.ok_or(SlackError)?;

        // Slack users are identified by an internal ID
        // Create a HashMap so we can display their real name instead
        let members = response.users.ok_or(SlackError)?;
        let mut user_names = Vec::new();
        let mut user_ids = Vec::new();
        for member in members {
            user_ids.push(member.id.ok_or(SlackError)?);
            user_names.push(member.name.ok_or(SlackError)?);
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
        let name = team_name.clone();
        let handler_channels = channels.clone();
        let handler_users = users.clone();
        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::OwnedMessage::{Ping, Pong, Text};
            use slack_api::MessageStandard;
            use slack_api::Message::Standard;
            loop {
                let message = websocket.recv_message();
                if let Ok(Text(message)) = message {
                    // parse the message and add it to events
                    if let Ok(Standard(MessageStandard {
                        user: Some(user),
                        text: Some(mut text),
                        channel: Some(channel),
                        ..
                    })) = serde_json::from_str::<slack_api::Message>(&message)
                    {
                        for &(ref code, ref replacement) in mention_patterns.iter() {
                            text = text.replace(code, replacement);
                        }

                        for &(ref code, ref replacement) in channel_patterns.iter() {
                            text = text.replace(code, replacement);
                        }

                        thread_sender
                            .send(Event::Message(Message {
                                server: name.clone(),
                                channel: handler_channels
                                    .get_human(&channel)
                                    .expect(&format!("Unknown channel ID {}", channel))
                                    .clone(),
                                sender: handler_users.get_human(&user).unwrap_or(&user).clone(),
                                contents: text,
                            }))
                            .unwrap();
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

        Ok(Box::new(SlackConn {
            token: api_key,
            client: client,
            users: users,
            channels: channels,
            channel_names: channel_names,
            team_name: team_name,
            last_message_timestamp: "".to_owned(),
            sender: sender,
        }))
    }

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
                if let Err(_) = response {
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
                if let Err(_) = response {
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
        if let Err(_) = post_message(&self.client, &self.token, &request) {
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
