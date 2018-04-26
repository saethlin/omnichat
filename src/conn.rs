#![macro_use]

use termion;

#[derive(Debug, Clone)]
pub struct Message {
    pub server: String,
    pub channel: String,
    pub sender: String,
    pub contents: String,
    pub is_mention: bool,
    //pub timestamp: f64,
}

pub enum Event {
    Message(Message),
    HistoryMessage(Message),
    HistoryLoaded { server: String, channel: String },
    Input(termion::event::Event),
    Error(String),
    Connected(Box<Conn>),
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Slack response was damaged")]
    SlackError,
    #[fail(display = "Discord response was damaged")]
    DiscordError,
}

macro_rules! omnierror {
    ($e:expr) => {
        Event::Error(format!("{:#?}\nfile {}, line {}", $e, file!(), line!()))
    };
}

pub trait Conn: Send {
    fn name(&self) -> &str;

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn send_channel_message(&mut self, _channel: &str, _contents: &str) {}

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a>;

    fn autocomplete(&self, _word: &str) -> Option<&str> {
        None
    }

    fn mark_read(&self, _channel: &str) {}
}
