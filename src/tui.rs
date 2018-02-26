use termion;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::io::stdin;
use conn::{Conn, Event, Message, ServerConfig};

use termion::input::TermRead;
use termion::event::Event::*;
use termion::event::Key::*;

use pancurses::{endwin, initscr};

fn djb2(input: &str) -> u64 {
    let mut hash: u64 = 5381;

    for c in input.bytes() {
        hash = (hash << 5).wrapping_add(hash).wrapping_add(c as u64);
    }
    return hash;
}

#[derive(Debug, Fail)]
pub enum TuiError {
    #[fail(display = "Got a message from an unknown channel")] UnknownChannel,
    #[fail(display = "Got a message from an unknown server")] UnknownServer,
}

pub struct TUI {
    servers: Vec<Server>,
    current_server: usize,
    pub message_buffer: String,
    shutdown: bool,
    events: Receiver<Event>,
    sender: Sender<Event>,
    previous_width: i32,
    win: ::pancurses::Window,
}

pub struct Server {
    channels: Vec<Channel>,
    connection: Box<Conn>,
    name: String,
    current_channel: usize,
    has_unreads: bool,
}

pub struct Channel {
    messages: Vec<ChanMessage>,
    name: String,
    pub has_unreads: bool,
}

struct ChanMessage {
    raw: String,
    pub contents: String,
    pub sender: String,
}

impl ChanMessage {
    pub fn new(sender: String, contents: String) -> Self {
        ChanMessage {
            raw: contents,
            contents: String::new(),
            sender: sender,
        }
    }

    pub fn format(&mut self, width: usize) {
        let mut formatted = String::with_capacity(self.raw.len());
        // The first line is special because we need to leave space for the sender, plus a ": "
        let mut current_length = self.sender.chars().count() + 2;
        for line in self.raw.lines() {
            for next_word in line.split(' ').filter(|word| word.len() > 0) {
                let next_word_len = next_word.chars().count();
                // If the word runs over the end of the current line, start a new one
                if current_length + next_word_len > width {
                    if let Some(' ') = formatted.chars().last() {
                        formatted.pop();
                    }
                    formatted.push_str("\n    ");
                    current_length = 4;
                }
                // If this word needs to be split (it's a url or something)
                if next_word_len > (width - 4) {
                    for c in next_word.chars() {
                        formatted.push(c);
                        current_length += 1;
                        if current_length == width {
                            formatted.push_str("\n    ");
                            current_length = 4;
                        }
                    }
                } else {
                    // Everything is fine
                    formatted.extend(next_word.chars());
                    formatted.push(' ');
                    current_length += next_word_len + 1;
                }
            }
            if let Some(' ') = formatted.chars().last() {
                formatted.pop();
            }
            formatted.push_str("\n    ");
            current_length = 4;
        }
        self.contents = formatted.trim_right().to_owned();
    }
}

impl Drop for TUI {
    fn drop(&mut self) {
        endwin();
    }
}

fn format_message_area(raw: &str, width: usize) -> String {
    let mut formatted = String::with_capacity(raw.len());
    let mut current_length = 0;
    let ending_spaces: String = raw.chars()
        .rev()
        .take_while(|c| c.is_whitespace())
        .collect();
    for line in raw.lines() {
        for next_word in line.split(' ').filter(|word| word.len() > 0) {
            let next_word_len = next_word.chars().count();
            // If the word runs over the end of the current line, start a new one
            if current_length + next_word_len > width {
                if let Some(' ') = formatted.chars().last() {
                    formatted.pop();
                }
                formatted.push_str("\n");
                current_length = 0;
            }
            // If this word needs to be split (it's a url or something)
            if next_word_len > (width - 4) {
                for c in next_word.chars() {
                    formatted.push(c);
                    current_length += 1;
                    if current_length == width {
                        formatted.push_str("\n");
                        current_length = 0;
                    }
                }
            } else {
                // Everything is fine
                formatted.extend(next_word.chars());
                formatted.push(' ');
                current_length += next_word_len + 1;
            }
        }
        if let Some(' ') = formatted.chars().last() {
            formatted.pop();
        }
        formatted.push_str("\n");
        current_length = 0;
    }
    if let Some('\n') = formatted.chars().last() {
        formatted.pop();
    }
    formatted.extend(ending_spaces.chars().rev());
    formatted
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
            shutdown: false,
            events: reciever,
            sender: sender,
            previous_width: 0,
            win: initscr(),
        };
        ::pancurses::start_color();
        // pancurses black and white aren't actually black and white
        ::pancurses::init_color(::pancurses::COLOR_BLACK, 0, 0, 0);
        ::pancurses::init_color(::pancurses::COLOR_WHITE, 1000, 1000, 1000);

        // set up corresponding pairs for all colors
        for i in 0..256 {
            ::pancurses::init_pair(i as i16, i as i16, ::pancurses::COLOR_BLACK);
        }

        ::pancurses::init_pair(0, ::pancurses::COLOR_WHITE, ::pancurses::COLOR_BLACK);
        ::pancurses::init_pair(1, ::pancurses::COLOR_RED, ::pancurses::COLOR_BLACK);
        tui.win.attrset(::pancurses::ColorPair(0));

        tui.add_server(ServerConfig::Client);
        tui.add_client_message(&format!("num colors- {}", ::pancurses::COLORS()));
        tui
    }

    pub fn sender(&self) -> Sender<Event> {
        self.sender.clone()
    }

    pub fn next_server(&mut self) {
        self.current_server += 1;
        if self.current_server >= self.servers.len() {
            self.current_server = 0;
        }
    }

    pub fn previous_server(&mut self) {
        if self.current_server > 0 {
            self.current_server -= 1;
        } else {
            self.current_server = self.servers.len() - 1;
        }
    }

    pub fn next_channel_unread(&mut self) {
        let server = &mut self.servers[self.current_server];
        for i in 0..server.channels.len() {
            let check_index = (server.current_channel + i) % server.channels.len();
            if server.channels[check_index].has_unreads {
                server.current_channel = check_index;
                break;
            }
        }
    }

    pub fn previous_channel_unread(&mut self) {
        let server = &mut self.servers[self.current_server];
        for i in 0..server.channels.len() {
            let check_index =
                (server.current_channel + server.channels.len() - i) % server.channels.len();
            if server.channels[check_index].has_unreads {
                server.current_channel = check_index;
                break;
            }
        }
    }

    pub fn next_channel(&mut self) {
        let server = &mut self.servers[self.current_server];
        server.current_channel += 1;
        if server.current_channel >= server.channels.len() {
            server.current_channel = 0;
        }
    }

    pub fn previous_channel(&mut self) {
        let server = &mut self.servers[self.current_server];
        if server.current_channel > 0 {
            server.current_channel -= 1;
        } else {
            server.current_channel = server.channels.len() - 1;
        }
    }

    pub fn add_client_message(&mut self, message: &str) {
        self.servers[0].channels[0]
            .messages
            .push(ChanMessage::new(String::from("Client"), message.to_owned()));
    }

    pub fn add_server(&mut self, config: ServerConfig) {
        use slack_conn::SlackConn;
        use discord_conn::DiscordConn;
        let connection = match config {
            ServerConfig::Slack { token } => SlackConn::new(token, self.sender()).unwrap(),
            ServerConfig::Discord { token, name } => {
                DiscordConn::new(token, name, self.sender()).unwrap()
            }
            ServerConfig::Client => ClientConn::new(self.sender()).unwrap(),
        };

        let mut channels: Vec<String> = connection.channels().into_iter().cloned().collect();
        channels.sort();
        self.servers.push(Server {
            channels: channels
                .iter()
                .map(|name| Channel {
                    messages: Vec::new(),
                    name: name.to_string(),
                    has_unreads: false,
                })
                .collect(),
            name: connection.name().to_string(),
            connection: connection,
            current_channel: 0,
            has_unreads: false,
        });
    }

    pub fn add_message(&mut self, message: Message, set_unread: bool) -> Result<(), Error> {
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

        channel
            .messages
            .push(ChanMessage::new(message.sender, message.contents));
        if set_unread {
            channel.has_unreads = true;
            server.has_unreads = true;
        }
        Ok(())
    }

    pub fn send_message(&mut self) {
        let server = &mut self.servers[self.current_server];
        let current_channel_name = &server.channels[server.current_channel].name;
        server
            .connection
            .send_channel_message(current_channel_name, &self.message_buffer);
        self.message_buffer.clear();
    }

    pub fn draw(&mut self) {
        // Format the message area text first.
        let chan_width = self.servers
            .iter()
            .flat_map(|s| s.channels.iter().map(|c| c.name.len()))
            .max()
            .unwrap() as i32;

        let width = self.win.get_max_x();
        let height = self.win.get_max_y();

        let message_area_formatted =
            format_message_area(&self.message_buffer, width as usize - chan_width as usize);

        self.win.clear();

        for i in 0..height {
            self.win.mvaddch(i as i32, chan_width, '|');
        }

        // Draw all the server names across the top
        self.win.mv(0, chan_width + 1);
        let num_servers = self.servers.len();
        for (s, server) in self.servers.iter_mut().enumerate() {
            let delim = if s == num_servers - 1 { "" } else { " • " };
            if s == self.current_server {
                self.win.attron(::pancurses::Attribute::Bold);
                self.win.addstr(&format!("{}{}", server.name, delim));
                self.win.attroff(::pancurses::Attribute::Bold);
                server.has_unreads = false;
            } else if server.has_unreads {
                self.win.color_set(1);
                self.win.addstr(&format!("{}{}", server.name, delim));
                self.win.color_set(0);
            } else {
                self.win.addstr(&format!("{}{}", server.name, delim));
            }
        }

        use std::iter::FromIterator;
        // Draw all the channels for the current server down the left side
        let server = &mut self.servers[self.current_server];
        for (c, channel) in server.channels.iter_mut().enumerate() {
            let shortened_name =
                String::from_iter(channel.name.chars().take((chan_width - 1) as usize));
            if c == server.current_channel {
                self.win.attron(::pancurses::Attribute::Bold);
                self.win.mvaddstr(c as i32, 0, &shortened_name);
                self.win.attroff(::pancurses::Attribute::Bold);
                // Remove unreads marker from current channel
                channel.has_unreads = false;
            } else if channel.has_unreads {
                self.win.color_set(1);
                self.win.mvaddstr(c as i32, 0, &shortened_name);
                self.win.color_set(0);
            } else {
                self.win.mvaddstr(c as i32, 0, &shortened_name);
            }
        }

        let message_area_lines = message_area_formatted.lines().count() as i32;
        let message_area_height = if message_area_lines > 1 {
            height - message_area_lines + 1
        } else {
            height
        };

        // Draw all the messages by looping over them in reverse
        let remaining_width = (width - chan_width) as usize;
        let mut row = message_area_height - 2;
        'outer: for message in server.channels[server.current_channel]
            .messages
            .iter_mut()
            .rev()
        {
            if self.previous_width != width {
                message.format(remaining_width - 1);
            }
            for (l, line) in message.contents.lines().rev().enumerate() {
                let num_lines = message.contents.lines().count();
                self.win.mv(row, chan_width + 1);
                row -= 1;
                if l == num_lines - 1 {
                    self.win.color_set((djb2(&message.sender) % 256) as i16);
                    self.win.addstr(&format!("{}", message.sender));
                    self.win.color_set(0);
                    self.win.addstr(": ");
                }
                self.win.addstr(line);
                if row == 1 {
                    break 'outer;
                }
            }
        }
        for (l, line) in message_area_formatted.lines().enumerate() {
            self.win
                .mvaddstr(message_area_height + l as i32 - 1, chan_width + 1, line);
        }

        if self.message_buffer.is_empty() {
            self.win.mv(height - 1, chan_width + 1);
        }

        self.win.refresh();
    }

    fn handle_input(&mut self, event: &termion::event::Event) {
        match *event {
            Key(Char('\n')) => {
                self.send_message();
            }
            Key(Backspace) => {
                self.message_buffer.pop();
            }
            Key(Ctrl('c')) => self.shutdown = true,
            Key(Up) => {
                self.previous_channel();
            }
            Key(Down) => {
                self.next_channel();
            }
            Key(Right) => {
                self.next_server();
            }
            Key(Left) => {
                self.previous_server();
            }
            Key(Char(c)) => {
                self.message_buffer.push(c);
            }
            Key(PageDown) => {
                self.next_channel_unread();
            }
            Key(PageUp) => {
                self.previous_channel_unread();
            }
            _ => {}
        }
        self.draw();
    }

    pub fn run(mut self) {
        loop {
            let event = match self.events.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            };

            match event {
                Event::Input(event) => {
                    self.handle_input(&event);
                }
                Event::Message(message) => {
                    if let Err(e) = self.add_message(message, true) {
                        self.add_client_message(&e.to_string());
                    }
                    self.draw();
                }
                Event::HistoryMessage(message) => {
                    if let Err(e) = self.add_message(message, false) {
                        self.add_client_message(&e.to_string());
                        self.draw();
                    }
                }
                Event::Error(message) => {
                    self.add_client_message(&message);
                    self.draw();
                }
                Event::Mention(message) => {
                    if let Err(e) = self.add_message(message.clone(), true) {
                        self.add_client_message(&e.to_string());
                    } else {
                        self.add_message(
                            Message {
                                sender: message.sender,
                                contents: message.contents,
                                server: "Client".to_owned(),
                                channel: "Mentions".to_owned(),
                            },
                            true,
                        ).unwrap_or_else(|e| self.add_client_message(&e.to_string()));
                    }
                    self.draw();
                }
                // TODO: Optimize
                Event::HistoryLoaded { .. } => {
                    self.draw();
                }
            }
            if self.shutdown {
                print!("\n\r");
                break;
            }
        }
    }
}

use failure::Error;
struct ClientConn {
    name: String,
    channel_names: Vec<String>,
    sender: Sender<Event>,
}

impl ClientConn {
    pub fn new(sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        Ok(Box::new(ClientConn {
            name: "Client".to_string(),
            channel_names: vec!["Errors".to_owned(), "Mentions".to_owned()],
            sender: sender,
        }))
    }
}

impl Conn for ClientConn {
    fn name(&self) -> &String {
        &self.name
    }

    fn handle_cmd(&mut self, _cmd: String, _args: Vec<String>) {}

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        self.sender
            .send(Event::Message(Message {
                server: "Client".to_string(),
                channel: channel.to_string(),
                contents: contents.to_string(),
                sender: String::new(),
            }))
            .expect("Sender died");
    }

    fn channels(&self) -> Vec<&String> {
        self.channel_names.iter().collect()
    }

    fn autocomplete(&self, _word: &str) -> Option<String> {
        None
    }
}
