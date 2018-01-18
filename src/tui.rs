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
    connection: Option<Box<Conn>>,
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

        Self {
            servers: vec![
                Server {
                    name: String::from("Client"),
                    channels: vec![
                        Channel {
                            name: String::from("warnings"),
                            messages: Vec::new(),
                            has_unreads: false,
                        },
                    ],
                    current_channel: 0,
                    connection: None,
                },
            ],
            current_server: 0,
            message_buffer: String::new(),
            shutdown: false,
            events: Some(reciever),
            sender: sender,
        }
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
            connection: Some(connection),
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
        match server.connection {
            Some(ref mut conn) => {
                let current_channel_name = &server.channels[server.current_channel].name;
                conn.send_channel_message(&current_channel_name, &self.message_buffer);
            }
            None => {}
        }
        self.message_buffer.clear();
    }

    pub fn draw(&mut self) {
        use termion::cursor::Goto;
        use termion::style::{Bold, Reset};
        let chan_width = 20;
        let (_width, height) =
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
                print!("{}{}+", Goto(1, c as u16 + 1), channel.name);
            } else {
                print!("{}{}", Goto(1, c as u16 + 1), channel.name);
            }
        }

        for (m, msg) in server.channels[server.current_channel]
            .messages
            .iter()
            .rev()
            .take(height as usize - 2)
            .enumerate()
        {
            print!("{}{}", Goto(chan_width + 1, height - 1 - m as u16), msg);
        }

        // Print the message buffer
        print!("{}{}", Goto(chan_width + 1, height), self.message_buffer);

        std::io::stdout().flush().expect("TUI drawing flush failed");
    }

    fn handle_input(&mut self, event: termion::event::Event) {
        match event {
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
                Event::Input(ev) => {
                    self.handle_input(ev);
                }
                Event::Message(message) => {
                    self.add_message(&message);
                }
            }
            self.draw();
            if self.shutdown {
                break;
            }
        }
    }
}
