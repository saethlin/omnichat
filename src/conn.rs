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

pub struct Message {
    pub server: String,
    pub channel: String,
    pub sender: String,
    pub contents: String,
    pub timestamp: DateTime,
    pub reactions: Vec<(String, usize)>,
}

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq, Eq, Ord)]
pub enum ChannelType {
    Normal,
    DirectMessage,
}

/// Events that a connection can send to a frontend
pub enum ConnEvent {
    Message(Message),
    MessageEdited {
        server: String,
        channel: String,
        contents: String,
        timestamp: DateTime,
    },
    HistoryLoaded {
        server: String,
        channel: String,
        messages: Vec<Message>,
    },
    Input(termion::event::Event),
    Error(String),
    ServerConnected(crate::tui::Server),
    MarkChannelRead {
        server: String,
        channel: String,
        read_at: Option<DateTime>,
        latest: Option<DateTime>,
    },
    ReactionAdded {
        server: String,
        channel: String,
        timestamp: DateTime,
        reaction: String,
    },
    ReactionRemoved {
        server: String,
        channel: String,
        timestamp: DateTime,
        reaction: String,
    },
    Resize,
}

/// Messages sent by a frontend to a connection
#[derive(Debug)]
pub enum TuiEvent {
    SendMessage {
        server: String,
        channel: String,
        contents: String,
    },
    MarkRead {
        server: String,
        channel: String,
    },
    Command {
        server: String,
        channel: String,
        command: String,
    },
    AddReaction {
        server: String,
        channel: String,
        reaction: String,
        timestamp: DateTime,
    },
    SendTyping {
        server: String,
        channel: String,
    },
    GetHistory {
        channel: String,
    },
}

// This is very derpy
// Don't want to do this with channels, because autocompletion should happen instantly
pub trait Completer: Send {
    fn autocomplete(&self, _word: &str) -> Vec<String>;
}
