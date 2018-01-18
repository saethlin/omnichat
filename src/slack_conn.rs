use std::sync::mpsc::Sender;
use std::thread;
use std::net::TcpStream;
use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message, ServerConfig};
use conn::ConnError::SlackError;
use slack_api;
use failure::Error;
use websocket;
use serde_json;
use std;

#[derive(Serialize)]
struct SlackMessage {
    id: usize,
    #[serde(rename = "type")] _type: String,
    channel: String,
    text: String,
}

pub struct SlackConn {
    token: String,
    team_name: String,
    users: BiMap,
    channels: BiMap,
    channel_names: Vec<String>,
    last_message_timestamp: String,
    client: slack_api::requests::Client,
    websocket: websocket::sync::Client<websocket::stream::sync::TlsStream<std::net::TcpStream>>,
    message_num: usize,
}

impl Conn for SlackConn {
    fn new(config: ServerConfig, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        println!("creating slack conn");
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

        let url = response.url.ok_or(SlackError)?;
        let websocket = websocket::ClientBuilder::new(&url)?.connect_secure(None)?;
        
        let connect_response = slack_api::rtm::connect(&client, &api_key)?;
        let other_url = connect_response.url.ok_or(SlackError)?;
        let mut thread_websocket = websocket::ClientBuilder::new(&other_url)?.connect_secure(None)?;

        // Spin off a thread that will feed message events back to the TUI
        thread::spawn(move || {
            use websocket::OwnedMessage::Text;
            use slack_api::MessageStandard;
            use slack_api::Message::Standard;
            for message in thread_websocket.incoming_messages() {
                if let Ok(Text(message)) = message {
                    // parse the message and add it to events
                    if let Ok(Standard(MessageStandard {
                        user: Some(user),
                        text: Some(text),
                        channel: Some(channel),
                        ..
                    })) = serde_json::from_str::<slack_api::Message>(&message)
                    {
                        sender
                            .send(Event::Message(Message {
                                channel: channel,
                                sender: user,
                                contents: text,
                            }))
                            .unwrap();
                    }
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
            websocket,
            message_num: 0,
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
        println!("{}", channel);
        use websocket::message::OwnedMessage;
        let channel_id = self.channels.get_id(channel).unwrap();
        let msg = SlackMessage {
            id: self.message_num,
            _type: "message".to_owned(),
            channel: channel_id.to_owned(),
            text: contents.to_owned(),
        };
        self.message_num += 1;
        let message_json = serde_json::to_string(&msg).unwrap();
        self.websocket
            .send_message(&OwnedMessage::Text(message_json))
            .unwrap();
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
