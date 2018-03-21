use termion;

#[derive(Debug, Clone)]
pub struct Message {
    pub server: String,
    pub channel: String,
    pub sender: String,
    pub contents: String,
    pub is_mention: bool,
    //pub timestamp: f32,
}

#[derive(Debug, Clone)]
pub enum Event {
    Message(Message),
    HistoryMessage(Message),
    HistoryLoaded { server: String, channel: String },
    Input(termion::event::Event),
    Error(String),
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Slack response was damaged")] SlackError,
    #[fail(display = "Discord response was damaged")] DiscordError,
}

pub trait Conn: Send {
    fn name(&self) -> &str;

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn send_channel_message(&mut self, _channel: &str, _contents: &str) {}

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a>;

    fn autocomplete(&self, _word: &str) -> Option<&str> {
        None
    }
}
