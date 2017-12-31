use termion;

use std;
use std::sync::{Arc, Mutex};
use std::io::Write;

use conn::Conn;

pub struct TUI {
    channels: Vec<Channel>,
    current_channel: Option<usize>,
    client_messages: Vec<String>,
    pub message_buffer: String,
}

pub struct Channel {
    messages: Vec<String>,
    connection: Arc<Mutex<Conn>>,
    server_name: String,
    channel_name: String,
}

impl TUI {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            current_channel: None,
            client_messages: Vec::new(),
            message_buffer: String::new(),
        }
    }

    pub fn next_channel(&mut self) {
        if let current_channel = Some(self.current_channel) {
            current_channel += 1;
            if current_channel >= self.channels.len() {
                current_channel = 0;
            }
        }
    }

    pub fn prev_channel(&mut self) {
        if let current_channel = Some(self.current_channel) {
            if self.current_channel > 0 {
                self.current_channel -= 1;
            } else {
                self.current_channel = self.channels.len() - 1;
            }
        }
    }

    pub fn add_client_message(&mut self, message: &str) {
        self.client_messages.push(message.to_owned())
    }

    pub fn add_channel(
        &mut self,
        server_name: &str,
        channel_name: &str,
        connection: Arc<Mutex<Conn>>,
    ) {
        // Change the currently selected channel
        self.channels.push(Channel {
            server_name: server_name.to_owned(),
            channel_name: channel_name.to_owned(),
            messages: Vec::new(),
            connection: connection,
        })
    }

    pub fn add_message(
        &mut self,
        server_name: &str,
        channel_name: &str,
        user: &str,
        message: &str,
    ) {
        self.channels
            .iter_mut()
            .find(|c| c.server_name == server_name && c.channel_name == channel_name)
            .unwrap()
            .messages
            .push(format!("{}: {}", user, message));
    }

    pub fn send_message(&mut self) {
        if let current_channel = Some(self.current_channel) {
        self.channels[current_channel]
            .messages
            .push(self.message_buffer.clone());

        self.channels[current_channel]
            .connection
            .lock()
            .unwrap()
            .send_channel_message(self.message_buffer);

        self.message_buffer.clear();
        }
    }

    pub fn draw(&mut self) -> Result<(), std::io::Error> {
        use termion::cursor::Goto;
        //use termion::style::{Bold, NoBold};
        let chan_width = 15;
        let (_width, height) = termion::terminal_size()?;

        print!("{}", termion::clear::All);

        for i in 1..height + 1 {
            print!("{}|", Goto(15, i));
        }

        // Draw all the server names
        let mut row = 0;
        for server in self.servers.iter() {
            print!("{}{}", Goto(1, row + 1 as u16), server.name);
            row += 1;
            for channel in server.channels.iter() {
                print!("{}{}", Goto(3, row + 1 as u16), channel.name);
                row += 1;
            }
        }

        if self.servers.len() > 0 {
            for (m, msg) in self.current_channel()
                .messages
                .iter()
                .rev()
                .take(height as usize - 1)
                .enumerate()
            {
                print!("{}{}", Goto(chan_width + 1, height - 1 - m as u16), msg);
            }
        }

        // Print the message buffer
        print!("{}{}", Goto(chan_width + 1, height), self.message_buffer);

        std::io::stdout().flush();
        //self.output_handle.flush()?;

        Ok(())
    }
}
