use conn::{Conn, Event, Message};
use std::sync::mpsc::{channel, Receiver, Sender};
use termion;

use termion::color::{AnsiValue, Fg};
use termion::cursor::Goto;
use termion::event::Event::*;
use termion::event::Key::*;
use termion::input::TermRead;
use termion::{color, style};

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
}

struct ChanMessage {
    formatted_width: Option<usize>,
    raw: String,
    pub formatted: String,
    pub sender: String,
    timestamp: String,
}

impl ChanMessage {
    fn new(sender: String, contents: String, timestamp: String) -> Self {
        ChanMessage {
            formatted_width: None,
            raw: contents,
            formatted: String::new(),
            sender,
            timestamp,
        }
    }

    fn format(&mut self, width: usize) {
        if Some(width) == self.formatted_width {
            return;
        }

        self.formatted_width = Some(width);
        self.formatted.clear();
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
                self.formatted.push('\n');
            }

            if l == 0 {
                for (l, wrapped_line) in first_line_wrapper.wrap_iter(line.trim_left()).enumerate()
                {
                    if l == 0 {
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
        // Clean trailing whitespace from messages
        while self.formatted.ends_with(|p: char| p.is_whitespace()) {
            self.formatted.pop();
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
                })
                .collect(),
            name: connection.name().to_string(),
            connection: connection,
            current_channel: 0,
            channel_scroll_offset: 0,
        });

        self.longest_channel_name = self.servers
            .iter()
            .flat_map(|s| s.channels.iter().map(|c| c.name.len()))
            .max()
            .unwrap_or(0) as u16 + 1;

        let previous_server_name = self.servers[self.current_server].name.clone();
        self.servers.sort_by_key(|s| s.name.clone());
        self.current_server = self.servers
            .iter()
            .position(|s| s.name == previous_server_name)
            .unwrap();
    }

    fn add_message(&mut self, message: Message, set_unread: bool) -> Result<(), Message> {
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

        if set_unread {
            channel.num_unreads += 1;
        }

        channel.messages.push(ChanMessage::new(
            message.sender,
            message.contents,
            message.timestamp,
        ));

        Ok(())
    }

    fn send_message(&mut self) {
        let contents = self.current_channel().message_buffer.clone();
        let server = &mut self.servers[self.current_server];
        let current_channel_name = &server.channels[server.current_channel].name;
        server
            .connection
            .send_channel_message(current_channel_name, &contents);
    }

    fn draw(&mut self, render_buffer: &mut String) {
        use std::fmt::Write;

        let draw_start = ::std::time::Instant::now();
        let (width, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        write!(render_buffer, "{}", termion::clear::All).unwrap();

        for i in 1..height + 1 {
            write!(render_buffer, "{}|", Goto(CHAN_WIDTH, i)).unwrap();
        }

        let remaining_width = (width - CHAN_WIDTH) as usize;
        // Reformat all the messages, inside their own block because NLLs
        {
            let server = &mut self.servers[self.current_server];
            for message in server.channels[server.current_channel].messages.iter_mut() {
                message.format(remaining_width);
            }
        }

        // Draw the message input area
        // We need this message area height to render the channel messages
        // More NLL hacking
        let message_area_height = {
            let wrapped_lines = ::textwrap::Wrapper::new((width - CHAN_WIDTH - 1) as usize)
                .wrap(&self.current_channel().message_buffer);

            let message_area_height = if wrapped_lines.len() > 1 {
                height - wrapped_lines.len() as u16 + 1
            } else {
                height
            };

            for (l, line) in wrapped_lines.iter().enumerate() {
                write!(
                    render_buffer,
                    "{}{}",
                    Goto(CHAN_WIDTH + 1, message_area_height + l as u16),
                    line
                ).unwrap();
            }

            message_area_height
        };

        // NLL HACK
        {
            // Draw all the messages by looping over them in reverse
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
                    write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, row)).unwrap();
                    write!(render_buffer, "{}", Fg(color::Red)).unwrap();
                    render_buffer.extend(::std::iter::repeat('-').take(remaining_width));
                    write!(render_buffer, "{}", Fg(color::Reset)).unwrap();
                    row -= 1;
                    draw_unread_marker = false;
                    if row == 1 {
                        break 'outer;
                    }
                }

                for (l, line) in message.formatted.lines().rev().enumerate() {
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    let num_lines = message.formatted.lines().count();
                    write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, row)).unwrap();
                    row -= 1;
                    if l == num_lines - 1 {
                        write!(
                            render_buffer,
                            "{}{}{}: {}",
                            Fg(COLORS[djb2(&message.sender) as usize % COLORS.len()]),
                            message.sender,
                            Fg(color::Reset),
                            line
                        ).unwrap();
                    } else {
                        write!(render_buffer, "{}", line).unwrap();
                    }
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
        }

        // Draw all the server names across the top
        write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, 1)).unwrap(); // Move to the top-right corner
        let num_servers = self.servers.len();
        for (s, server) in self.servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            if s == self.current_server {
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
            let server = &mut self.servers[self.current_server];
            {
                let height = height as usize;
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
                .take(height as usize)
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
                } else if channel.num_unreads > 0 {
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

        //self.add_client_message(&format!("{:?}", cursor_position));
        write!(render_buffer, "{}", Goto(self.cursor_pos as u16, height)).unwrap();

        {
            use std::io::Write;
            let out = ::std::io::stdout();
            let mut lock = out.lock();
            lock.write_all(render_buffer.as_bytes()).unwrap();
            lock.flush().unwrap();
            render_buffer.clear();
        }
        self.add_client_message(&format!("{:?}", draw_start.elapsed()));
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
                    self.autocompletions = if let Some(last_word) = self.current_channel()
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
                if let Err(msg) = self.add_message(message, true) {
                    self.add_client_message(&format!(
                        "Failed to add message from {}, {}",
                        msg.channel, msg.server
                    ));
                }
            }
            Event::HistoryMessage(message) => {
                // Attempt to add message, otherwise requeue it
                if let Err(message) = self.add_message(message, false) {
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
            } => match self.servers
                .iter_mut()
                .find(|s| s.name == server)
                .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
            {
                Some(c) => c.num_unreads = unread_count,
                None => self.sender
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

    // This is basically a game loop, we could use a temporary storage allocator
    // If that were possible
    pub fn run(mut self) {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};
        let mut render_buffer = String::new();
        self.draw(&mut render_buffer);
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
