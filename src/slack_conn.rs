use tui::TUI;
use std::sync::{Arc, Mutex};

use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, ServerConfig};
use conn::ConnError::SlackError;
use std::thread;
use slack;
use failure::Error;

enum WsMessage {
    Text(String),
    Close,
}

struct SlackHandler {
    tui_handle: Arc<Mutex<TUI>>,
    team_name: String,
    channels: BiMap,
    users: BiMap,
}

impl slack::EventHandler for SlackHandler {
    fn on_event(&mut self, _cli: &slack::RtmClient, event: slack::Event) {
        use slack::Event::{Message, MessageError, MessageSent};
        use slack::Message::Standard;
        use slack::api::MessageStandard;
        // Just handle the plain old messages for now
        match event {
            Message(box Standard(MessageStandard {
                text: Some(ref text),
                user: Some(ref user),
                channel: Some(ref channel),
                ..
            })) => {
                // Write the message to the frontend
                self.tui_handle
                    .lock()
                    .expect("TUI lock poisoned")
                    .add_message(
                        &self.team_name,
                        self.channels
                            .get_human(channel)
                            .expect(&format!("Unknown channel: {}", channel)),
                        self.users
                            .get_human(user)
                            .expect(&format!("Unknown user: {}", user)),
                        text,
                    );
            }
            MessageError(_) => self.tui_handle
                .lock()
                .expect("TUI lock was poisoned")
                .add_client_message("Message failed to send"),
            MessageSent(m) => println!("{:?}", m),
            _ => {}
        }
    }
    fn on_connect(&mut self, _cli: &slack::RtmClient) {}
    fn on_close(&mut self, _cli: &slack::RtmClient) {}
}

pub struct SlackConn {
    tui_handle: Arc<Mutex<TUI>>,
    token: String,
    client: slack::api::requests::Client,
    team_name: String,
    users: BiMap,
    channels: BiMap,
    channel_names: Vec<String>,
    channel_index: usize,
    last_message_timestamp: String,
    sender: slack::Sender,
    joinhandle: Option<thread::JoinHandle<()>>,
}

impl Conn for SlackConn {
    fn new(tui_handle: Arc<Mutex<TUI>>, config: ServerConfig) -> Result<Box<Conn>, Error> {
        let api_key = match config {
            ServerConfig::Slack { token } => token,
            _ => return Err(Error::from(SlackError)),
        };

        let mut rtmclient = slack::RtmClient::login(&api_key).map_err(|_| SlackError)?;
        let response = rtmclient.start_response().clone();

        // We use the server domain as a unique name for the TUI tab and logs
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
        for channel in response.channels.ok_or(SlackError)? {
            channel_names.push(channel.name.ok_or(SlackError)?);
            channel_ids.push(channel.id.ok_or(SlackError)?);
        }

        let channels = BiMap::new(BiMapBuilder {
            human: &channel_names,
            id: &channel_ids,
        });
        channel_names.sort();

        let mut slackhandler = SlackHandler {
            tui_handle: tui_handle.clone(),
            team_name: team_name.clone(),
            channels: channels.clone(),
            users: users.clone(),
        };

        // Clone the sender so we can move the rtmclient
        let sender = rtmclient.sender().clone();

        let joinhandle = thread::spawn(move || {
            rtmclient
                .run(&mut slackhandler)
                .expect("RtmClient exited with an error")
        });

        // TODO
        // Create a channel
        // Clone the sender

        // Create and split a websocket
        // Launch a thread that reads from the websocket, and handles events, adding messages to the
        // TUI

        // Launch another thread that reads from the channel, and handles shutdown and send_message
        // events using the sender from the websocket
        

        Ok(Box::new(SlackConn {
            tui_handle: tui_handle.clone(),
            token: api_key.to_owned(),
            client: slack::api::requests::Client::new().unwrap(),
            users: users,
            channels: channels,
            channel_names: channel_names.clone(),
            channel_index: 0,
            team_name: team_name.clone(),
            last_message_timestamp: "".to_owned(),
            joinhandle: Some(joinhandle),
            sender: sender,
        }))
    }

    fn handle_cmd(&mut self, cmd: String, args: Vec<String>) {
        match (cmd.as_ref(), args.len()) {
            ("join", 1) => {
                use slack::api::channels::JoinRequest;
                //let channel_id = &self.channels.get(&args[0]).expect("Unknown channel");
                if let Err(e) = slack::api::channels::join(
                    &self.client,
                    &self.token,
                    &JoinRequest {
                        name: &args[0],
                        validate: Some(true),
                    },
                ) {
                    println!("{:#?}", e);
                    panic!("Join request failed");
                    // Notify tiny
                };
            }
            ("leave", 1) => {
                use slack::api::channels::LeaveRequest;
                let channel_id = &self.channels.get_id(&args[0]).expect("Unknown channel");
                if let Err(e) = slack::api::channels::leave(
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
                use slack::api::chat::DeleteRequest;
                let request = DeleteRequest {
                    ts: &self.last_message_timestamp,
                    channel: &"".to_owned(), // Get from the TUI?
                    as_user: Some(true),
                };
                let response = slack::api::chat::delete(&self.client, &self.token, &request);
                if let Err(_) = response {
                    // Notify tiny
                }
            }
            ("update", 1) => {
                use slack::api::chat::UpdateRequest;
                let request = UpdateRequest {
                    ts: &self.last_message_timestamp,
                    channel: &"".to_owned(), // Get from the TUI?
                    text: &args[0],
                    attachments: None,
                    parse: None,
                    link_names: None,
                    as_user: Some(true),
                };

                let response = slack::api::chat::update(&self.client, &self.token, &request);
                if let Err(_) = response {
                    // Notify tiny
                }
            }
            ("search", 1) => {
                use slack::api::search::{MessagesRequest, MessagesResponse,
                                         MessagesResponseMessages};
                let mut request = MessagesRequest::default();
                request.query = &args[0];
                let response = slack::api::search::messages(&self.client, &self.token, &request);
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
                use slack::api::users::{ListRequest, ListResponse};
                let request = slack::api::users::list(
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

    fn send_channel_message(&self, channel: &str, contents: &str) {
        let channel_id = self.channels.get_id(channel).unwrap();
        self.sender.send_message(channel_id, contents).unwrap();
        //self.sender.send_message("slackbots", "slackbots").unwrap();
    }

    fn channels(&self) -> Vec<String> {
        self.channel_names.clone()
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

    fn name(&self) -> String {
        self.team_name.clone()
    }
}

impl Drop for SlackConn {
    fn drop(&mut self) {
        self.sender.shutdown();
        self.joinhandle.take().unwrap().join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use tempdir::TempDir;

    fn api_key() -> String {
        let mut file = File::open("/home/ben/slack_api_key").expect("Couldn't find API key");
        let mut api_key = String::new();
        file.read_to_string(&mut api_key).unwrap();
        api_key.trim().to_owned()
    }

    /*
    #[test]
    fn test_leave_cmd() {
        let tempdir = TempDir::new("tmp_logs").unwrap();
        CONN.lock().unwrap().handle_cmd(
            String::from("leave"),
            vec![String::from("slackbots")],
            &mut Vec::new(),
            &mut Logger::new(tempdir.into_path()),
        );
    }
    */

    /*
#[test]
    fn test_join_cmd() {
        let tempdir = TempDir::new("tmp_logs").unwrap();
        let mut logger = Logger::new(tempdir.into_path());
        CONN.lock().unwrap().handle_cmd(
            String::from("leave"),
            vec![String::from("slackbots")],
            &mut Vec::new(),
            &mut logger,
        );
        CONN.lock().unwrap().handle_cmd(
            String::from("join"),
            vec![String::from("slackbots")],
            &mut Vec::new(),
            &mut logger,
        );
    }
*/
    #[test]
    fn test_send_message() {
        let mut conn = SlackConn::new(&api_key()).unwrap();
        use std::{thread, time};
        for _ in 0..5 {
            conn.send_message("slackbots", "slackbots");
        }
        // Give slack time to respond with an error?
        thread::sleep(time::Duration::from_millis(5000));
    }
}
