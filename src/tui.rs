use chrono::Timelike;
use conn::{Conn, DateTime, Event, Message};
use cursor_vec::CursorVec;
use inlinable_string::InlinableString as IString;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};

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

const CHAN_WIDTH: u16 = 20;

fn djb2(input: &str) -> u64 {
    let mut hash: u64 = 5381;

    for c in input.bytes() {
        hash = (hash << 5).wrapping_add(hash).wrapping_add(u64::from(c));
    }
    hash
}

pub struct TUI {
    // Create a special server and channels, which will enable
    // all the get-methods to be nofail since we can fall back to them
    servers: CursorVec<Server>,
    longest_channel_name: u16,
    shutdown: bool,
    events: Receiver<Event>,
    sender: SyncSender<Event>,
    server_scroll_offset: usize,
    autocompletions: Vec<String>,
    autocomplete_index: usize,
    cursor_pos: usize,
    _guards: (
        ::termion::screen::AlternateScreen<::std::io::Stdout>,
        ::termion::raw::RawTerminal<::std::io::Stdout>,
    ),
    previous_terminal_height: u16,
    truncate_buffer_to: usize,
}

struct Server {
    channels: Vec<Channel>,
    connection: Box<Conn>,
    name: IString,
    current_channel: usize,
    channel_scroll_offset: usize,
}

impl Server {
    fn has_unreads(&self) -> bool {
        self.channels.iter().any(|c| c.num_unreads() > 0)
    }
}

struct Channel {
    messages: Vec<ChanMessage>,
    name: String,
    read_at: DateTime,
    message_scroll_offset: usize,
    message_buffer: String,
}

impl Channel {
    fn num_unreads(&self) -> usize {
        self.messages
            .iter()
            .rev()
            .filter(|m| m.timestamp > self.read_at)
            .count()
    }
}

struct ChanMessage {
    formatted_width: Option<usize>,
    raw: String,
    pub formatted: String,
    pub sender: IString,
    timestamp: DateTime,
    reactions: Vec<(IString, usize)>,
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
        }
    }
}

impl ChanMessage {
    fn format(&mut self, width: usize) {
        use std::fmt::Write;
        use termion::color::{AnsiValue, Fg, Reset};
        use textwrap::{NoHyphenation, Wrapper};

        if Some(width) == self.formatted_width {
            return;
        }

        use chrono::TimeZone;
        let timezone = ::chrono::offset::Local::now().timezone();
        let localtime = timezone.from_utc_datetime(&self.timestamp.naive_utc());

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
                        write!(
                            self.formatted,
                            "{}({:02}:{:02}) ",
                            Fg(AnsiValue::grayscale(8)),
                            localtime.time().hour(),
                            localtime.time().minute(),
                        ).unwrap();

                        write!(
                            self.formatted,
                            "{}{}{}: ",
                            Fg(COLORS[djb2(&self.sender) as usize % COLORS.len()]),
                            self.sender,
                            Fg(Reset),
                        ).unwrap();

                        self.formatted
                            .extend(wrapped_line.chars().skip_while(|c| c.is_whitespace()));
                    } else {
                        self.formatted.extend(wrapped_line.chars());
                    }
                    self.formatted.push('\n');
                }
            } else {
                for wrapped_line in wrapper.wrap_iter(&line) {
                    self.formatted.extend(wrapped_line.chars());
                    self.formatted.push('\n');
                }
            }
        }

        if !self.reactions.is_empty() {
            let _ = write!(self.formatted, "{}", indent_str);
        }

        for (r, count) in &self.reactions {
            let _ = write!(self.formatted, "{}({}) ", r, count);
        }

        // Clean trailing whitespace from messages
        while self.formatted.ends_with(|p: char| p.is_whitespace()) {
            self.formatted.pop();
        }
    }
}

impl TUI {
    pub fn new() -> Self {
        use std::thread;
        use termion::input::TermRead;
        use termion::raw::IntoRawMode;

        let screenguard = ::termion::screen::AlternateScreen::from(::std::io::stdout());
        let rawguard = ::std::io::stdout().into_raw_mode().unwrap();

        let (sender, reciever) = sync_channel(100);

        // Must be called before any threads are launched
        let signal = ::chan_signal::notify(&[::chan_signal::Signal::WINCH]);
        let winch_send = sender.clone();
        thread::spawn(move || {
            for _ in signal {
                winch_send.send(Event::Resize).unwrap();
            }
        });

        let send = sender.clone();
        thread::spawn(move || {
            for event in ::std::io::stdin().events() {
                if let Ok(ev) = event {
                    send.send(Event::Input(ev)).unwrap();
                }
            }
        });

        Self {
            servers: CursorVec::new(Server {
                channels: vec!["Errors", "Mentions"]
                    .iter()
                    .map(|name| Channel {
                        messages: Vec::new(),
                        name: name.to_string(),
                        read_at: ::chrono::Utc::now(),
                        message_scroll_offset: 0,
                        message_buffer: String::new(),
                    }).collect(),
                connection: ClientConn::new(sender.clone()),
                channel_scroll_offset: 0,
                current_channel: 0,
                name: IString::from("Client"),
            }),
            longest_channel_name: 0,
            shutdown: false,
            events: reciever,
            sender,
            server_scroll_offset: 0,
            autocompletions: Vec::new(),
            autocomplete_index: 0,
            cursor_pos: 0,
            _guards: (screenguard, rawguard),
            truncate_buffer_to: 0,
            previous_terminal_height: 0,
        }
    }

    pub fn sender(&self) -> SyncSender<Event> {
        self.sender.clone()
    }

    fn current_channel(&self) -> &Channel {
        let server = self.servers.get();
        &server.channels[server.current_channel]
    }

    fn current_channel_mut(&mut self) -> &mut Channel {
        let server = self.servers.get_mut();
        &mut server.channels[server.current_channel]
    }

    fn reset_current_unreads(&mut self) {
        let server = self.servers.get_mut();
        if server.channels[server.current_channel].num_unreads() > 0 {
            server.channels[server.current_channel].read_at = ::chrono::Utc::now();
            let current_channel = &server.channels[server.current_channel];

            server.connection.mark_read(&current_channel.name, None);
        }
    }

    fn next_server(&mut self) {
        self.reset_current_unreads();
        self.servers.next();
    }

    fn previous_server(&mut self) {
        self.reset_current_unreads();
        self.servers.prev();
    }

    fn next_channel_unread(&mut self) {
        //NLL HACK
        let index = {
            let server = self.servers.get_mut();
            (0..server.channels.len())
                .map(|i| (server.current_channel + i) % server.channels.len())
                .find(|i| server.channels[*i].num_unreads() > 0 && *i != server.current_channel)
        };
        match index {
            None => {}
            Some(index) => {
                self.reset_current_unreads();
                self.servers.get_mut().current_channel = index;
            }
        }
    }

    fn previous_channel_unread(&mut self) {
        //NLL HACK
        let index = {
            let server = self.servers.get_mut();
            (0..server.channels.len())
                .map(|i| {
                    (server.current_channel + server.channels.len() - i) % server.channels.len()
                }).find(|i| server.channels[*i].num_unreads() > 0 && *i != server.current_channel)
        };
        match index {
            None => {}
            Some(index) => {
                self.reset_current_unreads();
                self.servers.get_mut().current_channel = index;
            }
        }
    }

    fn next_channel(&mut self) {
        self.reset_current_unreads();
        let server = self.servers.get_mut();
        server.current_channel += 1;
        if server.current_channel >= server.channels.len() {
            server.current_channel = 0;
        }
    }

    fn previous_channel(&mut self) {
        self.reset_current_unreads();
        let server = &mut self.servers.get_mut();
        if server.current_channel > 0 {
            server.current_channel -= 1;
        } else {
            server.current_channel = server.channels.len() - 1;
        }
    }

    // Take by value because we need to own the allocation
    fn add_client_message(&mut self, message: String) {
        self.servers.get_first_mut().channels[0]
            .messages
            .push(ChanMessage {
                formatted: String::new(),
                formatted_width: None,
                raw: message,
                sender: "Client".into(),
                timestamp: ::chrono::Utc::now(),
                reactions: Vec::new(),
            });
    }

    pub fn add_server(&mut self, connection: Box<Conn>) {
        let mut channels: Vec<String> = connection.channels().map(|s| s.to_string()).collect();
        channels.sort();

        self.servers.push(Server {
            channels: channels
                .iter()
                .map(|name| Channel {
                    messages: Vec::new(),
                    name: name.to_string(),
                    read_at: ::chrono::Utc::now(), // This is a Bad Idea; we've marked everything as read by default, when we have no right to.
                    message_scroll_offset: 0,
                    message_buffer: String::new(),
                }).collect(),
            name: connection.name().into(),
            connection,
            current_channel: 0,
            channel_scroll_offset: 0,
        });

        self.longest_channel_name = self
            .servers
            .iter()
            .flat_map(|s| s.channels.iter().map(|c| c.name.len()))
            .max()
            .unwrap_or(0) as u16
            + 1;

        let previous_server_name = self.servers.get().name.clone();
        self.servers.sort_by_key(|s| s.name.clone());
        while self.servers.get().name != previous_server_name {
            self.servers.next();
        }
    }

    fn add_message(&mut self, message: Message) -> Result<(), Message> {
        if message.is_mention {
            self.servers.get_first_mut().channels[1]
                .messages
                .push(message.clone().into());
        }

        let server = match self.servers.iter_mut().find(|s| s.name == message.server) {
            Some(server) => server,
            None => return Err(message),
        };
        let channel = match server
            .channels
            .iter_mut()
            .find(|c| c.name == message.channel)
        {
            Some(channel) => channel,
            None => return Err(message),
        };

        let needs_sort = channel
            .messages
            .last()
            .map(|m| m.timestamp)
            .unwrap_or(message.timestamp.clone())
            > message.timestamp;

        channel.messages.push(message.into());

        if needs_sort {
            channel
                .messages
                .sort_unstable_by(|m1, m2| m1.timestamp.cmp(&m2.timestamp));
        }

        Ok(())
    }

    fn send_message(&mut self) {
        let contents = self.current_channel().message_buffer.clone();
        let current_channel_name = self.current_channel().name.clone();
        self.servers
            .get_mut()
            .connection
            .send_channel_message(&current_channel_name, &contents);
    }

    fn draw(&mut self, render_buffer: &mut String) {
        use std::fmt::Write;
        use termion::color::Fg;
        use termion::cursor::Goto;
        use termion::{color, style};

        let (terminal_width, terminal_height) =
            ::termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        if terminal_height != self.previous_terminal_height {
            render_buffer.clear();
            write!(render_buffer, "{}", ::termion::clear::All).unwrap();

            for i in 1..terminal_height + 1 {
                write!(render_buffer, "{}|", Goto(CHAN_WIDTH, i)).unwrap();
            }
            self.truncate_buffer_to = render_buffer.len();
            self.previous_terminal_height = terminal_height;
        } else {
            render_buffer.truncate(self.truncate_buffer_to);
        }

        let remaining_width = (terminal_width - CHAN_WIDTH) as usize;
        for message in &mut self.current_channel_mut().messages {
            message.format(remaining_width);
        }

        // Draw the message input area
        // We need this message area height to render the channel messages
        // More NLL hacking
        let total_chars = self.current_channel().message_buffer.chars().count();
        let rows = (total_chars / remaining_width) + 1;
        for row in (0..rows).rev() {
            write!(
                render_buffer,
                "{}",
                Goto(CHAN_WIDTH + 1, terminal_height - (rows - row - 1) as u16)
            ).unwrap();
            render_buffer.extend(
                self.current_channel()
                    .message_buffer
                    .chars()
                    .skip(remaining_width * row)
                    .take(remaining_width),
            );
        }
        let message_area_height = terminal_height - rows as u16 + 1;

        // Draw all the messages by looping over them in reverse
        let num_unreads = self.current_channel().num_unreads();
        let mut draw_unread_marker = num_unreads > 0;

        let offset = self.current_channel().message_scroll_offset;

        let mut row = message_area_height - 1;
        let mut skipped = 0;
        'outer: for (m, message) in self.current_channel().messages.iter().rev().enumerate() {
            // Unread marker
            if (draw_unread_marker) && (m == num_unreads) {
                write!(
                    render_buffer,
                    "{}{}",
                    Goto(CHAN_WIDTH + 1, row),
                    Fg(color::Red)
                ).unwrap();
                render_buffer.extend(::std::iter::repeat('-').take(remaining_width));
                write!(render_buffer, "{}", Fg(color::Reset)).unwrap();
                row -= 1;
                draw_unread_marker = false;
                if row == 1 {
                    break 'outer;
                }
            }

            for line in message.formatted.lines().rev() {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, row)).unwrap();
                render_buffer.push_str(line);
                row -= 1;
                if row == 1 {
                    break 'outer;
                }
            }
        }

        // If we didn't draw the unread marker, put it at the top of the screen
        if draw_unread_marker {
            use std::cmp::max;
            write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, max(2, row))).unwrap();
            write!(render_buffer, "{}", Fg(color::Red)).unwrap();
            render_buffer.extend(::std::iter::repeat('-').take(remaining_width));
            write!(render_buffer, "{}", Fg(color::Reset)).unwrap();
        }

        // Draw all the server names across the top
        write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, 1)).unwrap(); // Move to the top-right corner
        let num_servers = self.servers.len();
        for (s, server) in self
            .servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            if s == self.servers.tell() {
                write!(
                    render_buffer,
                    "{}{}{}",
                    style::Bold,
                    server.name,
                    style::Reset
                ).unwrap();
            } else if server.has_unreads() {
                write!(
                    render_buffer,
                    "{}{}{}",
                    Fg(color::Red),
                    server.name,
                    Fg(color::Reset),
                ).unwrap();
            } else {
                write!(
                    render_buffer,
                    "{}{}{}",
                    Fg(color::AnsiValue::rgb(3, 3, 3)),
                    server.name,
                    Fg(color::Reset),
                ).unwrap();
            }
            write!(
                render_buffer,
                "{}",
                if s == num_servers - 1 { "" } else { " • " }
            ).unwrap();
        }

        {
            // Draw all the channels for the current server down the left side
            let server = self.servers.get_mut();
            {
                let height = terminal_height as usize;
                if server.current_channel + 1 > height + server.channel_scroll_offset {
                    server.channel_scroll_offset = server.current_channel - height + 1
                } else if server.current_channel < server.channel_scroll_offset {
                    server.channel_scroll_offset = server.current_channel;
                }
            }

            fn write_shortened_name(f: &mut String, name: &str, max_len: usize) {
                if name.chars().count() < max_len {
                    write!(f, "{}", name).unwrap()
                } else {
                    f.extend(name.chars().take(max_len - 4).chain("...".chars()));
                }
            }

            for (c, channel) in server
                .channels
                .iter_mut()
                .enumerate()
                .skip(server.channel_scroll_offset)
                .take(terminal_height as usize)
            {
                if c == server.current_channel {
                    write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        style::Bold
                    ).unwrap();
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    write!(render_buffer, "{}", style::Reset).unwrap();
                } else if channel.num_unreads() > 0 {
                    write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(color::Red)
                    ).unwrap();
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    write!(render_buffer, "{}", style::Reset).unwrap();
                } else {
                    let gray = color::AnsiValue::rgb(3, 3, 3);
                    write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(gray)
                    ).unwrap();
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    write!(render_buffer, "{}", style::Reset).unwrap();
                }
            }
        }

        write!(
            render_buffer,
            "{}",
            Goto(
                CHAN_WIDTH + 1 + (self.cursor_pos % remaining_width) as u16,
                terminal_height - (total_chars / remaining_width) as u16
                    + (self.cursor_pos / remaining_width) as u16
            )
        ).unwrap();
        {
            use std::io::Write;
            let out = ::std::io::stdout();
            let mut lock = out.lock();
            lock.write_all(render_buffer.as_bytes()).unwrap();
            lock.flush().unwrap();
        }
    }

    fn handle_input(&mut self, event: &::termion::event::Event) {
        use termion::event::Event::*;
        use termion::event::Key::*;
        use termion::event::{MouseButton, MouseEvent};

        match *event {
            Key(Char('\n')) => {
                if !self.current_channel().message_buffer.is_empty() {
                    self.send_message();
                    self.current_channel_mut().message_buffer.clear();
                    self.cursor_pos = 0;
                }
            }
            Key(Backspace) => {
                if self.cursor_pos > 0 {
                    let remove_pos = self.cursor_pos as usize - 1;
                    self.current_channel_mut().message_buffer.remove(remove_pos);
                    self.cursor_pos -= 1;
                }
            }
            Key(Delete) => {
                let buffer_chars = self.current_channel().message_buffer.chars().count();
                if buffer_chars > 0 && self.cursor_pos < buffer_chars {
                    let remove_pos = self.cursor_pos;
                    self.current_channel_mut().message_buffer.remove(remove_pos);
                }
            }
            Key(Ctrl('c')) => self.shutdown = true,
            Key(Up) => {
                self.previous_channel();
            }
            Key(Down) => {
                self.next_channel();
            }
            Key(Ctrl('d')) => {
                self.next_server();
            }
            Key(Ctrl('a')) => {
                self.previous_server();
            }
            Key(PageDown) | Key(Ctrl('s')) => {
                self.next_channel_unread();
            }
            Key(PageUp) | Key(Ctrl('w')) => {
                self.previous_channel_unread();
            }
            Key(Ctrl('q')) | Mouse(MouseEvent::Press(MouseButton::WheelUp, ..)) => {
                self.current_channel_mut().message_scroll_offset += 1;
            }
            Key(Ctrl('e')) | Mouse(MouseEvent::Press(MouseButton::WheelDown, ..)) => {
                let chan = self.current_channel_mut();
                let previous_offset = chan.message_scroll_offset;
                chan.message_scroll_offset = previous_offset.saturating_sub(1);
            }
            Key(Left) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            Key(Right) => {
                if self.cursor_pos < self.current_channel().message_buffer.len() {
                    self.cursor_pos += 1;
                }
            }
            Key(Char('\t')) => {
                if self.autocompletions.is_empty() {
                    self.autocompletions = if let Some(last_word) = self
                        .current_channel()
                        .message_buffer
                        .split_whitespace()
                        .last()
                    {
                        self.servers.get().connection.autocomplete(last_word)
                    } else {
                        Vec::new()
                    }
                }
                if !self.autocompletions.is_empty() {
                    while let Some(c) = self.current_channel().message_buffer.chars().last() {
                        if c.is_whitespace() {
                            break;
                        } else {
                            self.current_channel_mut().message_buffer.pop();
                        }
                    }
                    self.autocomplete_index %= self.autocompletions.len();
                    let chosen_completion = self.autocompletions[self.autocomplete_index].clone();
                    self.current_channel_mut()
                        .message_buffer
                        .push_str(&chosen_completion);
                    self.cursor_pos = self.current_channel().message_buffer.len();
                    self.autocomplete_index += 1;
                }
            }
            Key(Char(c)) => {
                self.autocompletions.clear();
                self.autocomplete_index = 0;
                let current_pos = self.cursor_pos as usize;
                self.current_channel_mut()
                    .message_buffer
                    .insert(current_pos, c);
                self.cursor_pos += 1;
            }
            Unsupported(ref bytes) => match bytes.as_slice() {
                [27, 79, 65] => self
                    .sender
                    .send(Event::Input(Mouse(MouseEvent::Press(
                        MouseButton::WheelUp,
                        1,
                        1,
                    )))).unwrap(),
                [27, 79, 66] => self
                    .sender
                    .send(Event::Input(Mouse(MouseEvent::Press(
                        MouseButton::WheelDown,
                        1,
                        1,
                    )))).unwrap(),

                _ => {
                    self.add_client_message(format!("{:?}", bytes));
                }
            },
            _ => {}
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Resize => {} // Will be redrawn because we got an event
            Event::Input(event) => {
                self.handle_input(&event);
            }
            Event::Message(message) => {
                if let Err(message) = self.add_message(message) {
                    self.add_client_message(format!(
                        "Failed to add message from {}, {}",
                        message.channel, message.server
                    ));
                }
            }
            Event::MessageEdited {
                server,
                channel,
                contents,
                timestamp,
            } => {
                if let Some(msg) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                    .and_then(|c| {
                        c.messages
                            .iter_mut()
                            .rev()
                            .find(|m| m.timestamp == timestamp)
                    }) {
                    msg.raw = contents;
                    msg.formatted_width = None; // Force a reformat on next draw
                } else {
                    error!(
                        "Failed to process edit request: channel: {}, server: {}, timestamp: {}",
                        channel, server, timestamp
                    );
                }
            }
            Event::ReactionAdded {
                server,
                channel,
                timestamp,
                reaction,
            } => {
                if let Some(msg) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                    .and_then(|c| {
                        c.messages
                            .iter_mut()
                            .rev()
                            .find(|m| m.timestamp == timestamp)
                    }) {
                    let mut found = false;
                    if let Some(r) = msg.reactions.iter_mut().find(|rxn| rxn.0 == reaction) {
                        r.1 += 1;
                        found = true;
                    }
                    if !found {
                        msg.reactions.push((reaction, 1));
                    }
                } else {
                    error!(
                        "Failed to process edit request: channel: {}, server: {}, timestamp: {}",
                        channel, server, timestamp
                    );
                }
            }
            Event::Error(message) => {
                self.add_client_message(message);
            }
            Event::HistoryLoaded {
                server,
                channel,
                read_at,
            } => if let Some(c) = self
                .servers
                .iter_mut()
                .find(|s| s.name == server)
                .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
            {
                c.read_at = read_at;
            } else {
                error!("Failed to load history from {}, {}", channel, server);
            },
            Event::Connected(conn) => {
                self.add_server(conn);
            }
            Event::MarkChannelRead {
                server,
                channel,
                read_at,
            } => {
                let current_channel_name = self.current_channel().name.clone();
                if let Some(c) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                {
                    if current_channel_name != c.name {
                        c.read_at = read_at;
                    }
                }
            }
        }
    }

    // This is basically a game loop, we could use a temporary storage allocator
    // If that were possible
    pub fn run(mut self) {
        use std::time::{Duration, Instant};
        let mut render_buffer = String::new();
        self.draw(&mut render_buffer);
        while let Ok(event) = self.events.recv() {
            self.handle_event(event);

            // Now we have another 16 miliseconds to handle other events before anyone notices
            let start_instant = Instant::now();
            while let Some(remaining_time) =
                Duration::from_millis(16).checked_sub(start_instant.elapsed())
            {
                let event = match self.events.recv_timeout(remaining_time) {
                    Ok(ev) => ev,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(_) => {
                        self.shutdown = true;
                        break;
                    }
                };

                self.handle_event(event);
            }

            self.draw(&mut render_buffer);

            if self.shutdown {
                break;
            }
        }
    }
}

pub struct ClientConn {
    name: String,
    channel_names: Vec<String>,
    sender: SyncSender<Event>,
}

impl ClientConn {
    pub fn new(sender: SyncSender<Event>) -> Box<Conn> {
        Box::new(ClientConn {
            name: "Client".to_string(),
            channel_names: vec!["Errors".to_owned(), "Mentions".to_owned()],
            sender,
        })
    }
}

impl Conn for ClientConn {
    fn name(&self) -> &str {
        &self.name
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        self.sender
            .send(Event::Message(Message {
                server: "Client".into(),
                channel: channel.into(),
                contents: contents.into(),
                sender: "You".into(),
                is_mention: false,
                timestamp: ::chrono::Utc::now(),
                reactions: Vec::new(),
            })).expect("Sender died");
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }
}
