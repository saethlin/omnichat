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

impl Conn for DiscordConn {
    fn new(config: ServerConfig, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        let api_key = match config {
            ServerConfig::Discord { token, .. } => token,
            _ => return Err(Error::from(DiscordError)),
        };

        let connection = discord::Discord::from_user_token(&api_key)?;

        let servers = connection.get_servers().unwrap();
        let channels = servers.into_iter().map(|s| s.name).collect::<Vec<_>>();

        Ok(Box::new(DiscordConn {
            sender: sender,
            name: "Discord".to_string(),
            channels: channels,
        }))
    }

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
