use termion;

use std;
use std::io::Write;

pub struct TUI {
    channels: Vec<Channel>,
    current_channel: usize,
    pub message_buffer: String,
}

pub struct Channel {
    messages: Vec<String>,
    server_name: String,
    channel_name: String,
}

impl TUI {
    pub fn new() -> Self {
        Self {
            channels: vec![
                Channel {
                    messages: Vec::new(),
                    server_name: String::from("Client"),
                    channel_name: String::from("warnings"),
                },
            ],
            current_channel: 0,
            message_buffer: String::new(),
        }
    }

    pub fn next_channel(&mut self) {
        self.current_channel += 1;
        if self.current_channel >= self.channels.len() {
            self.current_channel = 0;
        }
    }

    pub fn previous_channel(&mut self) {
        if self.current_channel > 0 {
            self.current_channel -= 1;
        } else {
            self.current_channel = self.channels.len() - 1;
        }
    }

    pub fn add_client_message(&mut self, message: &str) {
        self.channels[0].messages.push(message.to_owned())
    }

    pub fn add_channel(
        &mut self,
        server_name: &str,
        channel_name: &str,
    ) {
        // Change the currently selected channel
        self.channels.push(Channel {
            server_name: server_name.to_owned(),
            channel_name: channel_name.to_owned(),
            messages: Vec::new(),
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
            .expect(&format!(
                "Unknown channel: {} server: {} combination",
                channel_name, server_name
            ))
            .messages
            .push(format!("{}: {}", user, message));
    }

    pub fn send_message(&mut self) {
        self.channels[self.current_channel]
            .messages
            .push(self.message_buffer.clone());

        self.message_buffer.clear();
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
        let mut last_server = String::new();
        for channel in self.channels.iter() {
            if channel.server_name != last_server {
                print!("{}{}", Goto(1, row + 1 as u16), channel.server_name);
                row += 1;
                last_server = channel.server_name.clone();
            }

            print!("{}{}", Goto(3, row + 1 as u16), channel.channel_name);
            row += 1;
        }

        for (m, msg) in self.channels[self.current_channel]
            .messages
            .iter()
            .rev()
            .take(height as usize - 1)
            .enumerate()
        {
            print!("{}{}", Goto(chan_width + 1, height - 1 - m as u16), msg);
        }

        // Print the message buffer
        print!("{}{}", Goto(chan_width + 1, height), self.message_buffer);

        std::io::stdout().flush();
        //self.output_handle.flush()?;

        Ok(())
    }
}
