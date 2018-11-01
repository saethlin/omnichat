use chrono::Timelike;
use conn;
use conn::{DateTime, IString};

lazy_static! {
    static ref COLORS: Vec<::termion::color::AnsiValue> = {
        let mut c = Vec::with_capacity(45);
        for r in 1..6 {
            for g in 1..6 {
                for b in 1..6 {
                    if r < 2 || g < 2 || b < 2 {
                        c.push(::termion::color::AnsiValue::rgb(r, g, b));
                    }
                }
            }
        }
        c
    };
}

fn djb2(input: &str) -> u64 {
    let mut hash: u64 = 5381;

    for c in input.bytes() {
        hash = (hash << 5).wrapping_add(hash).wrapping_add(u64::from(c));
    }
    hash
}

pub struct ChanMessage {
    formatted_width: Option<usize>,
    pub raw: String,
    formatted: String,
    sender: IString,
    timestamp: DateTime,
    reactions: Vec<(IString, usize)>,
}

impl From<conn::Message> for ChanMessage {
    fn from(message: conn::Message) -> ChanMessage {
        ChanMessage {
            formatted_width: None,
            raw: message.contents,
            formatted: String::new(),
            sender: message.sender,
            timestamp: message.timestamp,
            reactions: message.reactions,
        }
    }
}

impl ChanMessage {
    // Prevent mutating the timestamp but make it visible
    pub fn timestamp(&self) -> &DateTime {
        &self.timestamp
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
        use termion::color::{AnsiValue, Fg, Reset};
        use textwrap::{NoHyphenation, Wrapper};

        if Some(width) == self.formatted_width {
            return &self.formatted;
        }

        use chrono::TimeZone;
        let timezone = ::chrono::offset::Local::now().timezone();
        let localtime = timezone.from_utc_datetime(&self.timestamp.as_chrono().naive_utc());

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
                            "{}({:02}:{:02}) ",
                            Fg(AnsiValue::grayscale(8)),
                            localtime.time().hour(),
                            localtime.time().minute(),
                        );

                        let _ = write!(
                            self.formatted,
                            "{}{}{}: ",
                            Fg(COLORS[djb2(&self.sender) as usize % COLORS.len()]),
                            self.sender,
                            Fg(Reset),
                        );

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
            let _ = write!(
                self.formatted,
                "{}{}",
                indent_str,
                Fg(AnsiValue::grayscale(12))
            );

            for (r, count) in &self.reactions {
                let _ = write!(self.formatted, "{}({}) ", r, count);
            }

            let _ = write!(self.formatted, "{}", Fg(Reset));
        }

        // Clean trailing whitespace from messages
        while self.formatted.ends_with(|p: char| p.is_whitespace()) {
            self.formatted.pop();
        }

        &self.formatted
    }
}
