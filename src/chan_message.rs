use chrono::Timelike;
use conn::{DateTime, IString};

lazy_static!{}

pub fn djb2(input: &str) -> u64 {
    let mut hash: u64 = 5381;

    for c in input.bytes() {
        hash = (hash << 5).wrapping_add(hash).wrapping_add(u64::from(c));
    }
    hash
}

pub struct ChanMessage {
    formatted_width: Option<usize>,
    raw: String,
    formatted: String,
    pub sender: IString,
    timestamp: DateTime,
    reactions: Vec<(IString, usize)>,
    timestamp_formatted: String,
}

impl From<::conn::Message> for ChanMessage {
    fn from(message: ::conn::Message) -> ChanMessage {
        ChanMessage {
            formatted_width: None,
            raw: message.contents,
            formatted: String::new(),
            sender: message.sender,
            timestamp: message.timestamp,
            reactions: message.reactions,
            timestamp_formatted: String::new(),
        }
    }
}

impl ChanMessage {
    // Prevent mutating the timestamp but make it visible
    pub fn timestamp(&self) -> &DateTime {
        &self.timestamp
    }

    pub fn timestamp_str(&self) -> &str {
        &self.timestamp_formatted
    }

    pub fn add_reaction(&mut self, reaction: &str) {
        let mut found = false;
        if let Some(r) = self.reactions.iter_mut().find(|rxn| rxn.0 == reaction) {
            r.1 += 1;
            found = true;
        }
        if !found {
            self.reactions.push((reaction.into(), 1));
        }
        self.formatted_width = None;
    }

    pub fn remove_reaction(&mut self, reaction: &str) {
        if let Some(r) = self.reactions.iter_mut().find(|rxn| rxn.0 == reaction) {
            r.1 = r.1.saturating_sub(1);
            self.formatted_width = None;
        }
        self.reactions = self.reactions.iter().cloned().filter(|r| r.1 > 0).collect();
    }

    pub fn edit_to(&mut self, contents: String) {
        self.raw = contents;
        self.formatted_width = None;
    }

    pub fn formatted_to(&mut self, width: usize) -> &str {
        use std::fmt::Write;
        use textwrap::{NoHyphenation, Wrapper};

        if Some(width) == self.formatted_width {
            return &self.formatted;
        }

        use chrono::TimeZone;
        let timezone = ::chrono::offset::Local::now().timezone();
        let localtime = timezone.from_utc_datetime(&self.timestamp.naive_utc());

        self.timestamp_formatted = format!(
            "({:02}:{:02})",
            localtime.time().hour(),
            localtime.time().minute()
        );

        self.formatted_width = Some(width);
        self.formatted.clear();
        let indent_str = "    ";
        // 2 for the `: ` after the name, 8 for the time
        let sender_spacer = " ".repeat(self.sender.chars().count() + 2 + 8);
        let wrapper = Wrapper::with_splitter(width, NoHyphenation)
            .subsequent_indent(indent_str)
            .initial_indent(indent_str)
            .break_words(true);
        let first_line_wrapper = Wrapper::with_splitter(width, NoHyphenation)
            .subsequent_indent(indent_str)
            .initial_indent(&sender_spacer)
            .break_words(true);

        for (l, line) in self.raw.lines().enumerate() {
            // wrap_iter produces nothing on an empty line, so we have to supply the required newline
            if line == "" {
                self.formatted.push('\n');
            }

            if l == 0 {
                for (l, wrapped_line) in first_line_wrapper.wrap_iter(line.trim_left()).enumerate()
                {
                    if l == 0 {
                        let _ = write!(
                            self.formatted,
                            "({:02}:{:02}) ",
                            localtime.time().hour(),
                            localtime.time().minute(),
                        );

                        let _ = write!(self.formatted, "{}: ", self.sender,);

                        self.formatted
                            .extend(wrapped_line.chars().skip_while(|c| c.is_whitespace()));
                    } else {
                        self.formatted.push_str(&wrapped_line);
                    }
                    self.formatted.push('\n');
                }
            } else {
                for wrapped_line in wrapper.wrap_iter(&line) {
                    self.formatted.push_str(&wrapped_line);
                    self.formatted.push('\n');
                }
            }
        }

        if !self.reactions.is_empty() {
            self.formatted.push_str(indent_str);

            for (r, count) in &self.reactions {
                let _ = write!(self.formatted, "{}({}) ", r, count);
            }
        }

        // Clean trailing whitespace from messages
        while self.formatted.ends_with(|p: char| p.is_whitespace()) {
            self.formatted.pop();
        }

        &self.formatted
    }
}
