use termion;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::io::stdin;
use conn::{Conn, Event, Message};

use termion::input::TermRead;
use termion::event::Event::*;
use termion::event::Key::*;
use termion::color::{AnsiValue, Fg};
use termion::cursor::Goto;
use termion::{color, style};

use std::io::Write;
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

fn djb2(input: &str) -> u64 {
    let mut hash: u64 = 5381;

    for c in input.bytes() {
        hash = (hash << 5).wrapping_add(hash).wrapping_add(c as u64);
    }
    return hash;
}

#[derive(Debug, Fail)]
enum TuiError {
    #[fail(display = "Got a message from an unknown channel")] UnknownChannel,
    #[fail(display = "Got a message from an unknown server")] UnknownServer,
}

pub struct TUI {
    servers: Vec<Server>,
    current_server: usize,
    message_buffer: String,
    message_area_formatted: String,
    longest_channel_name: u16,
    shutdown: bool,
    events: Receiver<Event>,
    sender: Sender<Event>,
    server_scroll_offset: usize,
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
}

struct ChanMessage {
    formatted_width: Option<usize>,
    raw: String,
    pub contents: String,
    pub sender: String,
}

impl ChanMessage {
    fn new(sender: String, contents: String) -> Self {
        ChanMessage {
            formatted_width: None,
            raw: contents,
            contents: String::new(),
            sender: sender,
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
            message_buffer: String::new(),
            message_area_formatted: String::new(),
            longest_channel_name: 0,
            shutdown: false,
            events: reciever,
            sender: sender,
            server_scroll_offset: 0,
        };
        let sender = tui.sender();
        tui.add_server(ClientConn::new(sender));
        tui
    }

    pub fn sender(&self) -> Sender<Event> {
        self.sender.clone()
    }

    fn reset_current_unreads(&mut self) {
        let server = &mut self.servers[self.current_server];
        server.channels[server.current_channel].num_unreads = 0;
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
        self.reset_current_unreads();
        let server = &mut self.servers[self.current_server];
        for i in 0..server.channels.len() {
            let check_index = (server.current_channel + i) % server.channels.len();
            if server.channels[check_index].num_unreads > 0 {
                server.current_channel = check_index;
                break;
            }
        }
    }

    fn previous_channel_unread(&mut self) {
        self.reset_current_unreads();
        let server = &mut self.servers[self.current_server];
        for i in 0..server.channels.len() {
            let check_index =
                (server.current_channel + server.channels.len() - i) % server.channels.len();
            if server.channels[check_index].num_unreads > 0 {
                server.current_channel = check_index;
                break;
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
        self.servers[0].channels[0]
            .messages
            .push(ChanMessage::new(String::from("Client"), message.to_owned()));
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
    }

    fn add_message(&mut self, message: &Message, set_unread: bool) -> Result<(), Error> {
        use tui::TuiError::*;

        let server = self.servers
            .iter_mut()
            .find(|s| s.name == message.server)
            .ok_or(UnknownServer)?;
        let channel = server
            .channels
            .iter_mut()
            .find(|c| c.name == message.channel)
            .ok_or(UnknownChannel)?;

        if set_unread {
            channel.num_unreads += 1;
        }

        channel.messages.push(ChanMessage::new(
            message.sender.clone(),
            message.contents.clone(),
        ));
        Ok(())
    }

    fn send_message(&mut self) {
        let server = &mut self.servers[self.current_server];
        let current_channel_name = &server.channels[server.current_channel].name;
        server
            .connection
            .send_channel_message(current_channel_name, &self.message_buffer);
        self.message_buffer.clear();
        self.message_area_formatted.clear();
    }

    fn message_area_height(&self) -> u16 {
        let (_, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        let message_area_lines = self.message_area_formatted.lines().count() as u16;
        if message_area_lines > 1 {
            height - message_area_lines + 1
        } else {
            height
        }
    }

    #[allow(unused_must_use)]
    fn draw(&mut self) {
        let chan_width = self.longest_channel_name + 1;

        let (width, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        print!("{}", termion::clear::All);

        for i in 1..height + 1 {
            print!("{}|", Goto(chan_width, i));
        }

        // Reformat all the messages, inside their own block because NLLs
        {
            let remaining_width = (width - chan_width) as usize;
            let server = &mut self.servers[self.current_server];
            for message in server.channels[server.current_channel].messages.iter_mut() {
                message.format(remaining_width);
            }
        }

        // NLL HACK
        {
            let message_area_height = self.message_area_height();
            let out = ::std::io::stdout();
            let mut lock = out.lock();
            let server = &self.servers[self.current_server];
            // Draw all the messages by looping over them in reverse
            let mut row = message_area_height - 1;
            'outer: for (m, message) in server.channels[server.current_channel]
                .messages
                .iter()
                .rev()
                .enumerate()
            {
                // Unread marker
                let num_unreads = server.channels[server.current_channel].num_unreads;
                if (num_unreads > 0) && (m == num_unreads) {
                    write!(lock, "{}", Goto(chan_width + 1, row));
                    write!(
                        lock,
                        "{}{}{}",
                        Fg(color::Red),
                        ::std::iter::repeat('-')
                            .take((width - chan_width) as usize)
                            .collect::<String>(),
                        Fg(color::Reset)
                    );
                    row -= 1;
                }

                for (l, line) in message.contents.lines().rev().enumerate() {
                    let num_lines = message.contents.lines().count();
                    write!(lock, "{}", Goto(chan_width + 1, row));
                    row -= 1;
                    if l == num_lines - 1 {
                        write!(
                            lock,
                            "{}{}{}: {}",
                            Fg(COLORS[djb2(&message.sender) as usize % COLORS.len()]),
                            message.sender,
                            Fg(color::Reset),
                            line
                        );
                    } else {
                        write!(lock, "{}", line);
                    }
                    if row == 1 {
                        break 'outer;
                    }
                }
            }
        }

        self.draw_server_names();
        self.draw_channel_names();
        self.draw_message_area();
    }

    #[allow(unused_must_use)]
    fn draw_server_names(&mut self) {
        let out = ::std::io::stdout();
        let mut lock = out.lock();
        let chan_width = self.longest_channel_name + 1;
        let (width, _) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        let width = width - chan_width;
        // If the end of the current server name does not appear on the screen, increase until it
        // does
        loop {
            let chars_to_name_end = self.servers
                .iter()
                .skip(self.server_scroll_offset)
                .map(|s| s.name.chars().count())
                .take(self.current_server + 1)
                .sum::<usize>()
                + 3 * (self.current_server - self.server_scroll_offset);
            if chars_to_name_end < width as usize {
                break;
            } else {
                self.server_scroll_offset += 1;
            }
        }
        // If the beginning of the current server name does not appear on the screen, set scroll
        // index to its
        if self.current_server < self.server_scroll_offset {
            self.server_scroll_offset = self.current_server
        }
        /*
        let chars_to_name_start = self.servers
            .iter()
            .skip(self.server_scroll_offset)
            .map(|s| s.name.chars().count())
            .take(self.current_server)
            .sum::<usize>()
            + 3 * self.current_server;

        if chars_to_name_start > width as usize {
            self.server_scroll_offset = self.current_server;
        }
        */

        // Draw all the server names across the top
        write!(lock, "{}", Goto(chan_width + 1, 1)); // Move to the top-right corner
        let num_servers = self.servers.len();
        for (s, server) in self.servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            let delim = if s == num_servers - 1 { "" } else { " • " };
            if s == self.current_server {
                write!(lock, "{}{}{}", style::Bold, server.name, style::Reset,);
            } else if server.has_unreads() {
                write!(
                    lock,
                    "{}{}{}",
                    Fg(color::Red),
                    server.name,
                    Fg(color::Reset),
                );
            } else {
                write!(
                    lock,
                    "{}{}{}",
                    Fg(color::AnsiValue::rgb(3, 3, 3)),
                    server.name,
                    Fg(color::Reset),
                );
            }
            write!(lock, "{}", delim);
        }
        lock.flush().unwrap();
    }

    #[allow(unused_must_use)]
    fn draw_message_area(&self) {
        let out = ::std::io::stdout();
        let mut lock = out.lock();
        let chan_width = self.longest_channel_name + 1;

        let message_area_height = self.message_area_height();

        for (l, line) in self.message_area_formatted.lines().enumerate() {
            write!(
                lock,
                "{}{}",
                Goto(chan_width + 1, message_area_height + l as u16),
                line
            );
        }
        if self.message_buffer.is_empty() {
            let (_, height) =
                termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");
            write!(lock, "{}", Goto(chan_width + 1, height));
        }

        lock.flush().unwrap();
    }

    #[allow(unused_must_use)]
    fn draw_channel_names(&mut self) {
        let out = ::std::io::stdout();
        let mut lock = out.lock();
        let (_, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");
        let chan_width = self.longest_channel_name + 1;

        // Draw all the channels for the current server down the left side
        let server = &mut self.servers[self.current_server];
        {
            let height = height as usize;
            if server.current_channel > height + server.channel_scroll_offset {
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
            let shortened_name =
                String::from_iter(channel.name.chars().take((chan_width - 1) as usize));
            if c == server.current_channel {
                write!(
                    lock,
                    "{}{}{}{}",
                    Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                    style::Bold,
                    shortened_name,
                    style::Reset
                );
            } else if channel.num_unreads > 0 {
                write!(
                    lock,
                    "{}{}{}{}",
                    Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                    Fg(color::Red),
                    shortened_name,
                    Fg(color::Reset)
                );
            } else {
                let gray = color::AnsiValue::rgb(3, 3, 3);
                write!(
                    lock,
                    "{}{}{}{}",
                    Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                    Fg(gray),
                    shortened_name,
                    Fg(color::Reset)
                );
            }
        }
        lock.flush().unwrap();
    }

    fn handle_input(&mut self, event: &termion::event::Event) {
        match *event {
            Key(Char('\n')) => {
                if !self.message_buffer.is_empty() {
                    self.send_message();
                    self.draw();
                }
            }
            Key(Backspace) => {
                //TODO: It would be great to apply the same anti-flicker optimization,
                //but properly clearing the message area is tricky
                self.message_buffer.pop();
                let (width, _) =
                    termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");
                let chan_width = self.longest_channel_name + 1;
                self.message_area_formatted =
                    ::textwrap::fill(&self.message_buffer, (width - chan_width) as usize);
                self.draw();
            }
            Key(Ctrl('c')) => self.shutdown = true,
            Key(Up) => {
                self.previous_channel();
                self.draw();
            }
            Key(Down) => {
                self.next_channel();
                self.draw();
            }
            Key(Right) => {
                self.next_server();
                self.draw();
            }
            Key(Left) => {
                self.previous_server();
                self.draw();
            }
            Key(Char(c)) => {
                let previous_num_lines = self.message_area_formatted.lines().count();

                self.message_buffer.push(c);
                let (width, _) =
                    termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");
                let chan_width = self.longest_channel_name;
                self.message_area_formatted =
                    ::textwrap::fill(&self.message_buffer, (width - chan_width - 1) as usize);

                if previous_num_lines != self.message_area_formatted.lines().count() {
                    self.draw();
                } else {
                    self.draw_message_area();
                }
            }
            Key(PageDown) => {
                self.next_channel_unread();
                self.draw();
            }
            Key(PageUp) => {
                self.previous_channel_unread();
                self.draw();
            }
            _ => {}
        }
    }

    pub fn run(mut self) {
        self.draw();
        loop {
            let event = match self.events.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            };

            let (server_name, channel_name) = {
                let server = &self.servers[self.current_server];
                let server_name = server.name.clone();
                let channel_name = server.channels[server.current_channel].name.clone();
                (server_name, channel_name)
            };

            match event {
                Event::Input(event) => {
                    self.handle_input(&event);
                }
                // These optimizations could be substantially improved
                // Technically we only need to redraw the one server recieving the event
                // I'm a bit uncomfortable doing that though because it spreads out the logic
                // a lot and may make refactoring/recoloring this too difficult
                Event::Message(message) => {
                    if let Err(e) = self.add_message(&message, true) {
                        self.add_client_message(&e.to_string());
                    }
                    if message.server == server_name && message.channel == channel_name {
                        self.draw();
                    } else if message.server == server_name {
                        self.draw_channel_names();
                        self.draw_message_area();
                    } else {
                        self.draw_server_names();
                        self.draw_message_area();
                    }
                }
                Event::HistoryMessage(message) => {
                    if let Err(e) = self.add_message(&message, false) {
                        self.add_client_message(&e.to_string());
                    }
                }
                Event::Error(message) => {
                    self.add_client_message(&message);
                    if &server_name == "Client" && &channel_name == "Errors" {
                        self.draw();
                    }
                    if &server_name == "Client" {
                        self.draw_channel_names();
                        self.draw_message_area();
                    }
                }
                Event::HistoryLoaded { server, channel } => {
                    if (server == server_name) && (channel == channel_name) {
                        self.draw();
                    }
                }
            }
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
            }))
            .expect("Sender died");
    }

    fn channels<'a>(&'a self) -> Box<Iterator<Item = &'a str> + 'a> {
        Box::new(self.channel_names.iter().map(|s| s.as_str()))
    }
}
