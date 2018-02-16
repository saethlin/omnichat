use std::sync::mpsc::Sender;
//use std::thread;
//use bimap::{BiMap, BiMapBuilder};
use conn::{Conn, Event, Message, ServerConfig};
use conn::ConnError::DiscordError;
use failure::Error;

use discord;

pub struct DiscordConn {
    sender: Sender<Event>,
    name: String,
    channels: Vec<String>,
}

impl DiscordConn {
    pub fn new(
        token: String,
        server_name: String,
        sender: Sender<Event>,
    ) -> Result<Box<Conn>, Error> {
        use discord::model::PossibleServer::Online;

        let (connection, info) = discord::Discord::from_user_token(&token)?.connect()?;
        /*
        for server in &info.servers {
            if let &Online(ref server) = server {
                if server.name == name {
                    let channels: Vec<String> =
                        server.channels.iter().map(|s| s.name.clone()).collect();
                    return Ok(Box::new(DiscordConn {
                        sender: sender,
                        name: name,
                        channels: channels,
                    }));
                }
            }
        }
        */
        Err(::failure::Error::from(DiscordError))
    }
}

impl Conn for DiscordConn {
    fn send_channel_message(&mut self, channel: &str, contents: &str) {}

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn channels(&self) -> Vec<&String> {
        self.channels.iter().collect()
    }

    fn autocomplete(&self, _word: &str) -> Option<String> {
        None
    }

    fn name(&self) -> &String {
        &self.name
    }
}
