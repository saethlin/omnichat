pub use inlinable_string::InlinableString as IString;
use termion;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime(::chrono::DateTime<::chrono::Utc>);

impl From<::slack::Timestamp> for DateTime {
    fn from(ts: ::slack::Timestamp) -> DateTime {
        let seconds = ts.microseconds / 1_000_000;
        let nanoseconds = (ts.microseconds % 1_000_000) * 1_000;
        let naive =
            ::chrono::naive::NaiveDateTime::from_timestamp(seconds as i64, nanoseconds as u32);
        DateTime(::chrono::DateTime::from_utc(naive, ::chrono::Utc))
    }
}

impl From<DateTime> for ::slack::Timestamp {
    fn from(datetime: DateTime) -> ::slack::Timestamp {
        let as_chrono = datetime.0;
        ::slack::Timestamp {
            microseconds: as_chrono.timestamp() * 1_000_000
                + i64::from(as_chrono.timestamp_subsec_micros()),
        }
    }
}

// Can make one from a chrono datetime, shouldn't be necessary hm
impl From<::chrono::DateTime<::chrono::Utc>> for DateTime {
    fn from(datetime: ::chrono::DateTime<::chrono::Utc>) -> DateTime {
        DateTime(datetime)
    }
}

impl ::std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl DateTime {
    pub fn now() -> Self {
        DateTime(::chrono::offset::Utc::now())
    }

    pub fn as_chrono(&self) -> &::chrono::DateTime<::chrono::Utc> {
        &self.0
    }
}

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
    Input(termion::event::Event),
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
