use conn::{Conn, Event, Message};
use std::sync::mpsc::{channel, Receiver, Sender};
use termion;

use termion::color::{AnsiValue, Fg};
use termion::cursor::Goto;
use termion::event::Event::*;
use termion::event::Key::*;
use termion::input::TermRead;
use termion::{color, style};

use std::iter::FromIterator;

lazy_static! {
    static ref COLORS: Vec<AnsiValue> = {
        let mut c = Vec::with_capacity(45);
        for r in 1..6 {
            for g in 1..6 {
                for b in 1..6 {
                    if r < 2 || g < 2 || g < 2 {
                        c.push(AnsiValue::rgb(r, g, b));
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
        hash = (hash << 5).wrapping_add(hash).wrapping_add(c as u64);
    }
    return hash;
}

#[derive(Debug, Fail)]
enum TuiError {
    #[fail(display = "Got a message from an unknown channel: {}", channel)]
    UnknownChannel { channel: String },
    #[fail(display = "Got a message from an unknown server: {}", server)]
    UnknownServer { server: String },
}

pub struct TUI {
    servers: Vec<Server>,
    current_server: usize,
    longest_channel_name: u16,
    shutdown: bool,
    events: Receiver<Event>,
    sender: Sender<Event>,
    server_scroll_offset: usize,
    autocompletions: Vec<String>,
    autocomplete_index: usize,
    cursor_pos: usize,
    _guards: (
        termion::screen::AlternateScreen<::std::io::Stdout>,
        termion::raw::RawTerminal<::std::io::Stdout>,
    ),
}

struct Server {
    channels: Vec<Channel>,
    connection: Box<Conn>,
    name: String,
    current_channel: usize,
    channel_scroll_offset: usize,
}

impl Server {
    fn has_unreads(&self) -> bool {
        self.channels.iter().any(|c| c.num_unreads > 0)
    }
}

struct Channel {
    messages: Vec<ChanMessage>,
    name: String,
    num_unreads: usize,
    message_scroll_offset: usize,
    message_buffer: String,
    message_buffer_formatted: String,
}

struct ChanMessage {
    formatted_width: Option<usize>,
    raw: String,
    pub contents: String,
    pub sender: String,
    timestamp: String,
}

impl ChanMessage {
    fn new(sender: String, contents: String, timestamp: String) -> Self {
        ChanMessage {
            formatted_width: None,
            raw: contents,
            contents: String::new(),
            sender,
            timestamp,
        }
    }

    fn format(&mut self, width: usize) {
        if Some(width) == self.formatted_width {
            return;
        }

        self.formatted_width = Some(width);
        self.contents.clear();
        let indent_str = "    ";
        let sender_spacer = " ".repeat(self.sender.chars().count() + 2);
        let wrapper = ::textwrap::Wrapper::new(width)
            .subsequent_indent(indent_str)
            .initial_indent(indent_str)
            .break_words(true);
        let first_line_wrapper = ::textwrap::Wrapper::new(width)
            .subsequent_indent(indent_str)
            .initial_indent(&sender_spacer)
            .break_words(true);

        for (l, line) in self.raw.lines().enumerate() {
            // wrap_iter produces nothing on an empty line, so we have to supply the required newline
            if line == "" {
                self.contents.push('\n');
            }

            if l == 0 {
                for (l, wrapped_line) in first_line_wrapper.wrap_iter(line.trim_left()).enumerate()
                {
                    if l == 0 {
                        self.contents
                            .extend(wrapped_line.chars().skip_while(|c| c.is_whitespace()));
                    } else {
                        self.contents.extend(wrapped_line.chars());
                    }
                    self.contents.push('\n');
                }
            } else {
                for wrapped_line in wrapper.wrap_iter(&line) {
                    self.contents.extend(wrapped_line.chars());
                    self.contents.push('\n');
                }
            }
        }
        // Clean trailing whitespace from messages
        while self.contents.ends_with(|p: char| p.is_whitespace()) {
            self.contents.pop();
        }
    }
}

impl TUI {
    pub fn new() -> Self {
        use std::io::stdin;
        use std::thread;
        use termion::raw::IntoRawMode;

        let screenguard = termion::screen::AlternateScreen::from(::std::io::stdout());
        let rawguard = ::std::io::stdout().into_raw_mode().unwrap();

        let (sender, reciever) = channel();
        let send = sender.clone();
        thread::spawn(move || {
            for event in stdin().events() {
                if let Ok(ev) = event {
                    send.send(Event::Input(ev)).expect("IO event sender died");
                }
            }
        });

        let mut tui = Self {
            servers: Vec::new(),
            current_server: 0,
            longest_channel_name: 0,
            shutdown: false,
            events: reciever,
            sender: sender,
            server_scroll_offset: 0,
            autocompletions: Vec::new(),
            autocomplete_index: 0,
            cursor_pos: 0,
            _guards: (screenguard, rawguard),
        };
        let sender = tui.sender();
        tui.add_server(ClientConn::new(sender));
        tui
    }

    pub fn sender(&self) -> Sender<Event> {
        self.sender.clone()
    }

    fn current_channel(&self) -> &Channel {
        let server = &self.servers[self.current_server];
        &server.channels[server.current_channel]
    }

    fn current_channel_mut(&mut self) -> &mut Channel {
        let server = &mut self.servers[self.current_server];
        &mut server.channels[server.current_channel]
    }

    fn reset_current_unreads(&mut self) {
        let server = &mut self.servers[self.current_server];
        if server.channels[server.current_channel].num_unreads > 0 {
            server.channels[server.current_channel].num_unreads = 0;
            let current_channel = &server.channels[server.current_channel];

            server.connection.mark_read(
                &current_channel.name,
                current_channel
                    .messages
                    .last()
                    .map(|m| m.timestamp.as_str()),
            );
        }
    }

    fn next_server(&mut self) {
        self.reset_current_unreads();
        self.current_server += 1;
        if self.current_server >= self.servers.len() {
            self.current_server = 0;
        }
    }

    fn previous_server(&mut self) {
        self.reset_current_unreads();
        if self.current_server > 0 {
            self.current_server -= 1;
        } else {
            self.current_server = self.servers.len() - 1;
        }
    }

    fn next_channel_unread(&mut self) {
        //NLL HACK
        let index = {
            let server = &self.servers[self.current_server];
            (0..server.channels.len())
                .map(|i| (server.current_channel + i) % server.channels.len())
                .find(|i| server.channels[*i].num_unreads > 0 && *i != server.current_channel)
        };
        match index {
            None => {}
            Some(index) => {
                self.reset_current_unreads();
                self.servers[self.current_server].current_channel = index;
            }
        }
    }

    fn previous_channel_unread(&mut self) {
        //NLL HACK
        let index = {
            let server = &self.servers[self.current_server];
            (0..server.channels.len())
                .map(|i| {
                    (server.current_channel + server.channels.len() - i) % server.channels.len()
                })
                .find(|i| server.channels[*i].num_unreads > 0 && *i != server.current_channel)
        };
        match index {
            None => {}
            Some(index) => {
                self.reset_current_unreads();
                self.servers[self.current_server].current_channel = index;
            }
        }
    }

    fn next_channel(&mut self) {
        self.reset_current_unreads();
        let server = &mut self.servers[self.current_server];
        server.current_channel += 1;
        if server.current_channel >= server.channels.len() {
            server.current_channel = 0;
        }
    }

    fn previous_channel(&mut self) {
        self.reset_current_unreads();
        let server = &mut self.servers[self.current_server];
        if server.current_channel > 0 {
            server.current_channel -= 1;
        } else {
            server.current_channel = server.channels.len() - 1;
        }
    }

    fn add_client_message(&mut self, message: &str) {
        self.servers[0].channels[0].messages.push(ChanMessage::new(
            String::from("Client"),
            message.to_owned(),
            0.0.to_string(),
        ));
        if !((self.current_server == 0) & (self.servers[0].current_channel == 0)) {
            self.servers[0].channels[0].num_unreads += 1;
        }
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
                    num_unreads: 0,
                    message_scroll_offset: 0,
                    message_buffer: String::new(),
                    message_buffer_formatted: String::new(),
                })
                .collect(),
            name: connection.name().to_string(),
            connection: connection,
            current_channel: 0,
            channel_scroll_offset: 0,
        });

        self.longest_channel_name = self
            .servers
            .iter()
            .flat_map(|s| s.channels.iter().map(|c| c.name.len()))
            .max()
            .unwrap_or(0) as u16 + 1;

        let previous_server_name = self.servers[self.current_server].name.clone();
        self.servers.sort_by_key(|s| s.name.clone());
        self.current_server = self
            .servers
            .iter()
            .position(|s| s.name == previous_server_name)
            .unwrap();
    }

    fn add_message(&mut self, message: &Message, set_unread: bool) -> Result<(), Error> {
        use tui::TuiError::*;
        //NLL HACK
        {
            let server = self
                .servers
                .iter_mut()
                .find(|s| s.name == message.server)
                .ok_or(UnknownServer {
                    server: message.server.clone(),
                })?;
            let channel = server
                .channels
                .iter_mut()
                .find(|c| c.name == message.channel)
                .ok_or(UnknownChannel {
                    channel: message.channel.clone(),
                })?;

            if set_unread {
                channel.num_unreads += 1;
            }

            channel.messages.push(ChanMessage::new(
                message.sender.clone(),
                message.contents.clone(),
                message.timestamp.clone(),
            ));
        }

        if message.is_mention {
            self.servers[0].channels[1].messages.push(ChanMessage::new(
                message.sender.clone(),
                message.contents.clone(),
                message.timestamp.clone(),
            ));
            if set_unread {
                self.servers[0].channels[1].num_unreads += 1;
            }
        }

        Ok(())
    }

    fn send_message(&mut self) {
        {
            let contents = self.current_channel().message_buffer.clone();
            let server = &mut self.servers[self.current_server];
            let current_channel_name = &server.channels[server.current_channel].name;
            server
                .connection
                .send_channel_message(current_channel_name, &contents);
        }
    }

    fn message_area_height(&self) -> u16 {
        let (_, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        let message_area_lines = self
            .current_channel()
            .message_buffer_formatted
            .lines()
            .count() as u16;
        if message_area_lines > 1 {
            height - message_area_lines + 1
        } else {
            height
        }
    }

    fn draw(&mut self) {
        use std::fmt::Write;
        let mut lock = String::new();
        let (width, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        write!(lock, "{}", termion::clear::All).unwrap();

        for i in 1..height + 1 {
            write!(lock, "{}|", Goto(CHAN_WIDTH, i)).unwrap();
        }

        let remaining_width = (width - CHAN_WIDTH) as usize;
        // Reformat all the messages, inside their own block because NLLs
        {
            let server = &mut self.servers[self.current_server];
            for message in server.channels[server.current_channel].messages.iter_mut() {
                message.format(remaining_width);
            }
        }

        // NLL HACK
        {
            // Draw all the messages by looping over them in reverse
            let message_area_height = self.message_area_height();
            let server = &self.servers[self.current_server];

            let num_unreads = server.channels[server.current_channel].num_unreads;
            let mut draw_unread_marker = num_unreads > 0;

            let offset = self.current_channel().message_scroll_offset;

            let mut row = message_area_height - 1;
            let mut skipped = 0;
            'outer: for (m, message) in server.channels[server.current_channel]
                .messages
                .iter()
                .rev()
                .enumerate()
            {
                // Unread marker
                if (draw_unread_marker) && (m == num_unreads) {
                    write!(lock, "{}", Goto(CHAN_WIDTH + 1, row)).unwrap();
                    write!(
                        lock,
                        "{}{}{}",
                        Fg(color::Red),
                        ::std::iter::repeat('-')
                            .take((width - CHAN_WIDTH) as usize)
                            .collect::<String>(),
                        Fg(color::Reset)
                    ).unwrap();
                    row -= 1;
                    draw_unread_marker = false;
                    if row == 1 {
                        break 'outer;
                    }
                }

                for (l, line) in message.contents.lines().rev().enumerate() {
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    let num_lines = message.contents.lines().count();
                    write!(lock, "{}", Goto(CHAN_WIDTH + 1, row)).unwrap();
                    row -= 1;
                    if l == num_lines - 1 {
                        write!(
                            lock,
                            "{}{}{}: {}",
                            Fg(COLORS[djb2(&message.sender) as usize % COLORS.len()]),
                            message.sender,
                            Fg(color::Reset),
                            line
                        ).unwrap();
                    } else {
                        write!(lock, "{}", line).unwrap();
                    }
                    if row == 1 {
                        break 'outer;
                    }
                }
            }
            // If we didn't draw the unread marker, put it at the top of the screen
            if draw_unread_marker {
                use std::cmp::max;
                write!(lock, "{}", Goto(CHAN_WIDTH + 1, max(2, row))).unwrap();
                write!(
                    lock,
                    "{}{}{}",
                    Fg(color::Red),
                    ::std::iter::repeat('-')
                        .take((width - CHAN_WIDTH) as usize)
                        .collect::<String>(),
                    Fg(color::Reset)
                ).unwrap();
            }
        }

        // Draw all the server names across the top
        write!(lock, "{}", Goto(CHAN_WIDTH + 1, 1)).unwrap(); // Move to the top-right corner
        let num_servers = self.servers.len();
        for (s, server) in self
            .servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            if s == self.current_server {
                write!(lock, "{}{}{}", style::Bold, server.name, style::Reset).unwrap();
            } else if server.has_unreads() {
                write!(
                    lock,
                    "{}{}{}",
                    Fg(color::Red),
                    server.name,
                    Fg(color::Reset),
                ).unwrap();
            } else {
                write!(
                    lock,
                    "{}{}{}",
                    Fg(color::AnsiValue::rgb(3, 3, 3)),
                    server.name,
                    Fg(color::Reset),
                ).unwrap();
            }
            write!(lock, "{}", if s == num_servers - 1 { "" } else { " • " }).unwrap();
        }

        {
            // Draw all the channels for the current server down the left side
            let server = &mut self.servers[self.current_server];
            {
                let height = height as usize;
                if server.current_channel + 1 > height + server.channel_scroll_offset {
                    server.channel_scroll_offset = server.current_channel - height + 1
                } else if server.current_channel < server.channel_scroll_offset {
                    server.channel_scroll_offset = server.current_channel;
                }
            }

            for (c, channel) in server
                .channels
                .iter_mut()
                .enumerate()
                .skip(server.channel_scroll_offset)
                .take(height as usize)
            {
                let shortened_name = if channel.name.chars().count() < CHAN_WIDTH as usize {
                    channel.name.clone()
                } else {
                    String::from_iter(
                        channel
                            .name
                            .chars()
                            .take(CHAN_WIDTH as usize - 4)
                            .chain("...".chars()),
                    )
                };
                if c == server.current_channel {
                    write!(
                        lock,
                        "{}{}{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        style::Bold,
                        shortened_name,
                        style::Reset
                    ).unwrap();
                } else if channel.num_unreads > 0 {
                    write!(
                        lock,
                        "{}{}{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(color::Red),
                        shortened_name,
                        Fg(color::Reset)
                    ).unwrap();
                } else {
                    let gray = color::AnsiValue::rgb(3, 3, 3);
                    write!(
                        lock,
                        "{}{}{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(gray),
                        shortened_name,
                        Fg(color::Reset)
                    ).unwrap();
                }
            }
        }

        // Draw the message input area
        self.current_channel_mut().message_buffer_formatted = ::textwrap::fill(
            &self.current_channel().message_buffer,
            (width - CHAN_WIDTH - 1) as usize,
        );

        let message_area_height = self.message_area_height();

        for (l, line) in self
            .current_channel()
            .message_buffer_formatted
            .lines()
            .enumerate()
        {
            write!(
                lock,
                "{}{}",
                Goto(CHAN_WIDTH + 1, message_area_height + l as u16),
                line
            ).unwrap();
        }

        write!(
            lock,
            "{}",
            Goto(CHAN_WIDTH + 1 + self.cursor_pos as u16, height)
        ).unwrap();

        {
            use std::io::Write;
            let out = ::std::io::stdout();
            let mut l = out.lock();
            l.write_all(lock.as_bytes()).unwrap();
            l.flush().unwrap();
        }
    }

    fn handle_input(&mut self, event: &termion::event::Event) {
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
                    let remove_pos = self.cursor_pos - 1;
                    self.current_channel_mut().message_buffer.remove(remove_pos);
                    self.cursor_pos -= 1;
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
            Key(Ctrl('q')) => {
                let server = &mut self.servers[self.current_server];
                server.channels[server.current_channel].message_scroll_offset += 1;
            }
            Key(Ctrl('e')) => {
                let server = &mut self.servers[self.current_server];
                let chan = &mut server.channels[server.current_channel];
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
                        self.servers[self.current_server]
                            .connection
                            .autocomplete(last_word)
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
                        .extend(chosen_completion.chars());
                    self.autocomplete_index += 1;
                }
            }
            Key(Char(c)) => {
                self.autocompletions.clear();
                self.autocomplete_index = 0;

                let current_pos = self.cursor_pos;
                self.current_channel_mut()
                    .message_buffer
                    .insert(current_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Input(event) => {
                self.handle_input(&event);
            }
            Event::Message(message) => {
                if let Err(e) = self.add_message(&message, true) {
                    self.add_client_message(&e.to_string());
                }
            }
            Event::HistoryMessage(message) => {
                // Attempt to add message, otherwise requeue it
                if self.add_message(&message, false).is_err() {
                    self.sender.send(Event::HistoryMessage(message)).unwrap();
                }
            }
            Event::Error(message) => {
                self.add_client_message(&message);
            }
            Event::HistoryLoaded {
                server,
                channel,
                unread_count,
            } => match self
                .servers
                .iter_mut()
                .find(|s| s.name == server)
                .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
            {
                Some(c) => c.num_unreads = unread_count,
                None => self
                    .sender
                    .send(Event::HistoryLoaded {
                        server,
                        channel,
                        unread_count,
                    })
                    .unwrap(),
            },
            Event::Connected(conn) => {
                self.add_server(conn);
            }
        }
    }

    pub fn run(mut self) {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};
        self.draw();
        loop {
            let event = match self.events.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            };
            self.handle_event(event);

            // Now we have another 16 miliseconds to handle other events before anyone notices
            let start_instant = Instant::now();
            while let Some(remaining_time) =
                Duration::from_millis(16).checked_sub(start_instant.elapsed())
            {
                let event = match self.events.recv_timeout(remaining_time) {
                    Ok(ev) => ev,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(_) => {
                        self.shutdown = true;
                        break;
                    }
                };

                self.handle_event(event);
            }

            self.draw();

            if self.shutdown {
                break;
            }
        }
    }
}

use failure::Error;
pub struct ClientConn {
    name: String,
    channel_names: Vec<String>,
    sender: Sender<Event>,
}

impl ClientConn {
    pub fn new(sender: Sender<Event>) -> Box<Conn> {
        Box::new(ClientConn {
            name: "Client".to_string(),
            channel_names: vec!["Errors".to_owned(), "Mentions".to_owned()],
            sender: sender,
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
                server: "Client".to_string(),
                channel: channel.to_string(),
                contents: contents.to_string(),
                sender: String::new(),
                is_mention: false,
                timestamp: 0.0.to_string(),
            }))
            .expect("Sender died");
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }
}
