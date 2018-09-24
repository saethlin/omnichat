pub use inlinable_string::InlinableString as IString;

pub type DateTime = ::chrono::DateTime<::chrono::Utc>;

#[derive(Debug, Clone)]
pub struct Message {
    pub server: IString,
    pub channel: IString,
    pub sender: IString,
    pub contents: String,
    pub is_mention: bool,
    pub timestamp: DateTime,
    pub reactions: Vec<(IString, usize)>,
}

/// Events that a connection can send to a frontend
pub enum Event {
    Message(Message),
    MessageEdited {
        server: IString,
        channel: IString,
        contents: String,
        timestamp: DateTime,
    },
    HistoryLoaded {
        server: IString,
        channel: IString,
        read_at: DateTime,
    },
    Error(String),
    Connected(Box<Conn>),
    MarkChannelRead {
        server: IString,
        channel: IString,
        read_at: DateTime,
    },
    ReactionAdded {
        server: IString,
        channel: IString,
        timestamp: DateTime,
        reaction: IString,
    },
    ReactionRemoved {
        server: IString,
        channel: IString,
        timestamp: DateTime,
        reaction: IString,
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

    fn channels(&self) -> &[IString];

    fn send_channel_message(&mut self, _channel: &str, _contents: &str) {}

    fn mark_read(&self, _channel: &str) {}

    fn handle_cmd(&mut self, _cmd: &str) {}

    fn autocomplete(&self, _word: &str) -> Vec<String> {
        Vec::new()
    }

    fn add_reaction(&self, _reaction: &str, _channel: &str, _timestamp: DateTime) {}
}
