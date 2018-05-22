use termion;

#[derive(Debug, Clone)]
pub struct Message {
    pub server: String,
    pub channel: String,
    pub sender: String,
    pub contents: String,
    pub is_mention: bool,
    pub timestamp: String,
}

pub enum Event {
    Message(Message),
    HistoryMessage(Message),
    HistoryLoaded {
        server: String,
        channel: String,
        unread_count: usize,
    },
    Input(termion::event::Event),
    Error(String),
    Connected(Box<Conn>),
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Could not connect to the server")]
    ConnectError,
}

macro_rules! omnierror {
    ($e:expr) => {
        Event::Error(format!("{}\nfile {}, line {}", $e, file!(), line!()))
    };
}

pub trait Conn: Send {
    fn name(&self) -> &str;

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a>;

    fn send_channel_message(&mut self, _channel: &str, _contents: &str) {}

    fn mark_read(&self, _channel: &str, _timestamp: Option<&str>) {}

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn autocomplete(&self, _word: &str) -> Vec<String> {
        Vec::new()
    }
}
