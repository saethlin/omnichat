use failure::Error;
use std::sync::mpsc::Sender;
use termion;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerConfig {
    Client,
    Slack { token: String },
    Discord { token: String, name: String },
}

#[derive(Debug, Clone)]
pub struct Message {
    pub server: String,
    pub channel: String,
    pub sender: String,
    pub contents: String,
}

#[derive(Debug)]
pub enum Event {
    Message(Message),
    HistoryMessage(Message),
    Input(termion::event::Event),
    Error(String),
    Mention(Message),
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Slack response was damaged")] SlackError,
    #[fail(display = "Discord response was damaged")] DiscordError,
}

pub trait Conn: Send {
    fn name(&self) -> &String;

    fn handle_cmd(&mut self, cmd: String, args: Vec<String>);

    fn send_channel_message(&mut self, channel: &str, contents: &str);

    fn channels(&self) -> Vec<&String>;

    fn autocomplete(&self, word: &str) -> Option<String>;
}
