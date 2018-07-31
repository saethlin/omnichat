use inlinable_string::InlinableString as IString;
use termion;

pub type DateTime = ::chrono::DateTime<::chrono::Utc>;
pub type LocalDateTime = ::chrono::DateTime<::chrono::offset::Local>;

#[derive(Debug, Clone)]
pub struct Message {
    pub server: IString,
    pub channel: IString,
    pub sender: IString,
    pub contents: String,
    pub is_mention: bool,
    pub timestamp: DateTime,
}

/// Events that a connection can send to a frontend
pub enum Event {
    Message(Message),
    HistoryLoaded {
        server: IString,
        channel: IString,
        read_at: DateTime,
    },
    Input(termion::event::Event),
    Error(String),
    Connected(Box<Conn>),
    MarkChannelRead {
        server: IString,
        channel: IString,
        read_at: DateTime,
    },
    Resize,
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Could not connect to the server")]
    ConnectError,
}

pub trait Conn: Send {
    fn name(&self) -> &str;

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a>;

    fn send_channel_message(&mut self, _channel: &str, _contents: &str) {}

    fn mark_read(&self, _channel: &str, _timestamp: Option<&str>) {}

    fn handle_cmd(&mut self, _cmd: &str) {}

    fn autocomplete(&self, _word: &str) -> Vec<String> {
        Vec::new()
    }
}
