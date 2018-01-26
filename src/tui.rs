use termion;
use std;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::io::{stdin, Write};
use conn::{Conn, Event, Message};

use termion::input::TermRead;
use termion::event::Event::*;
use termion::event::Key::*;

pub struct TUI {
    servers: Vec<Server>,
    current_server: usize,
    pub message_buffer: String,
    shutdown: bool,
    events: Option<Receiver<Event>>,
    sender: Sender<Event>,
}

pub struct Server {
    channels: Vec<Channel>,
    connection: Box<Conn>,
    name: String,
    current_channel: usize,
}

pub struct Channel {
    messages: Vec<String>,
    name: String,
    has_unreads: bool,
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

        let client_conn = ClientConn::new(ServerConfig::Client, sender.clone()).unwrap();

        let mut tui = Self {
            servers: Vec::new(),
            current_server: 0,
            message_buffer: String::new(),
            shutdown: false,
            events: Some(reciever),
            sender: sender,
        };
        tui.add_server(client_conn);
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
            .push(message.to_owned())
    }

    pub fn add_server(&mut self, connection: Box<Conn>) {
        self.servers.push(Server {
            channels: connection
                .channels()
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
        });
    }

    pub fn add_message(&mut self, message: &Message) {
        let server = self.servers
            .iter_mut()
            .find(|s| s.name == message.server)
            .expect(&format!("Unknown server {}", message.server));
        let channel = server
            .channels
            .iter_mut()
            .find(|c| c.name == message.channel)
            .expect(&format!("Unknown channel {}", message.channel));
        channel
            .messages
            .push(format!("{}: {}", message.sender, message.contents));
        channel.has_unreads = true;
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
        use termion::cursor::Goto;
        use termion::style::{Bold, Reset};
        let chan_width = 20;
        let (width, height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        print!("{}", termion::clear::All);

        for i in 1..height + 1 {
            print!("{}|", Goto(chan_width, i));
        }

        // Draw all the server names across the top
        let mut server_bar = format!("{}", Goto(21, 1)); // Move to the top-right corner
        for (s, server) in self.servers.iter().enumerate() {
            if s == self.current_server {
                server_bar.extend(format!("{}{}{} ", Bold, server.name, Reset).chars());
            } else {
                server_bar.extend(format!("{} ", server.name).chars());
            }
        }
        print!("{}", server_bar);

        // Draw all the channels for the current server down the left side
        let server = &mut self.servers[self.current_server];
        for (c, channel) in server.channels.iter_mut().enumerate() {
            if c == server.current_channel {
                print!("{}{}{}{}", Goto(1, c as u16 + 1), Bold, channel.name, Reset);
                // Remove unreads marker from current channel
                channel.has_unreads = false;
            } else if channel.has_unreads {
                print!("{}+{}", Goto(1, c as u16 + 1), channel.name);
            } else {
                print!("{}{}", Goto(1, c as u16 + 1), channel.name);
            }
        }

        let remaining_width = width - chan_width;
        let mut msg_row = height - 1;
        let mut buf = Vec::new();
        let mut line = String::new();
        'outer: for msg in server.channels[server.current_channel]
            .messages
            .iter()
            .rev()
        {
            for c in msg.chars().rev() {
                if c != '\n' {
                    buf.push(c);
                }

                if c == '\n' || buf.len() == remaining_width as usize {
                    line.clear();
                    line.extend(buf.iter().rev());
                    buf.clear();
                    print!(
                        "{}{}",
                        Goto(chan_width + 1, msg_row as u16),
                        line
                    );
                    msg_row -= 1;
                    if msg_row == 1 {
                        break 'outer;
                    }
                }
            }
            // If we have chars left to print, do that too
            if !buf.is_empty() {
                line.clear();
                line.extend(buf.iter().rev());
                buf.clear();
                print!(
                    "{}{}",
                    Goto(chan_width + 1, msg_row as u16),
                    line
                );
                msg_row -= 1;
                if msg_row == 1 {
                    break 'outer;
                }
            }
        }
        // Print the message buffer
        print!("{}{}", Goto(chan_width + 1, height), self.message_buffer);

        std::io::stdout().flush().expect("TUI drawing flush failed");
    }

    fn handle_input(&mut self, event: &termion::event::Event) {
        match *event {
            Key(Char('\n')) => self.send_message(),
            Key(Backspace) => {
                self.message_buffer.pop();
            }
            Key(Ctrl('c')) => self.shutdown = true,
            Key(Up) => self.previous_channel(),
            Key(Down) => self.next_channel(),
            Key(Right) => self.next_server(),
            Key(Left) => self.previous_server(),
            Key(Char(c)) => self.message_buffer.push(c),
            _ => {}
        }
    }

    pub fn run(mut self) {
        let events = self.events.take().unwrap();
        for event in events {
            match event {
                Event::Input(event) => self.handle_input(&event),
                Event::Message(message) => self.add_message(&message),
                Event::Error(message) => self.add_client_message(&message),
            }
            self.draw();
            if self.shutdown {
                break;
            }
        }
    }
}

use conn::ServerConfig;
use failure::Error;
struct ClientConn {
    name: String,
    channel_names: Vec<String>,
    sender: Sender<Event>,
}

impl Conn for ClientConn {
    fn new(_config: ServerConfig, sender: Sender<Event>) -> Result<Box<Conn>, Error> {
        Ok(Box::new(ClientConn {
            name: "Client".to_string(),
            channel_names: vec!["Warnings".to_owned(), "Mentions".to_owned()],
            sender: sender,
        }))
    }

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
