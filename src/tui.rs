use crate::chan_message::ChanMessage;
use crate::conn::{ChannelType, Completer, ConnEvent, DateTime, Message, TuiEvent};
use crate::cursor_vec::CursorVec;
use crate::DFAExtension;

use std::cmp::{max, min};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::error;
use regex_automata::DenseDFA;

lazy_static::lazy_static! {
    static ref URL_REGEX_DATA: Vec<u16> = {
        let raw = include_bytes!("../url_regex");
        raw.chunks_exact(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect()
    };
    static ref URL_REGEX: DenseDFA<&'static [u16], u16> = unsafe {
        DenseDFA::from_bytes(std::slice::from_raw_parts(
            URL_REGEX_DATA.as_ptr() as *const u8, URL_REGEX_DATA.len() * 2)
        )
    };
}

const CHAN_WIDTH: u16 = 20;

pub struct Tui {
    servers: CursorVec<Server>,
    longest_channel_name: u16,
    shutdown: bool,
    events: UnboundedReceiver<ConnEvent>,
    sender: UnboundedSender<ConnEvent>, // Tui can send events to itself, this is also cloned and sent to backend connections
    server_scroll_offset: usize,
    autocompletions: Vec<String>,
    autocomplete_index: usize,
    cursor_pos: usize,
    _guards: (
        termion::screen::AlternateScreen<::std::io::Stdout>,
        termion::raw::RawTerminal<::std::io::Stdout>,
    ),
    previous_terminal_height: u16,
    truncate_buffer_to: usize,
}

pub struct Server {
    pub channels: Vec<Channel>,
    pub completer: Option<Box<dyn Completer>>,
    pub name: String,
    pub current_channel: usize,
    pub channel_scroll_offset: usize,
    pub sender: UnboundedSender<TuiEvent>,
}

impl Server {
    fn has_unreads(&self) -> bool {
        self.channels.iter().any(Channel::is_unread)
    }
}

pub struct Channel {
    pub messages: Vec<ChanMessage>,
    pub name: String,
    pub read_at: DateTime,
    pub latest: DateTime,
    pub has_history: bool,
    pub message_scroll_offset: usize,
    pub message_buffer: String,
    pub channel_type: ChannelType,
}

impl Channel {
    fn is_unread(&self) -> bool {
        self.latest > self.read_at
    }

    fn num_unreads(&self) -> usize {
        self.messages
            .iter()
            .rev()
            .take_while(|m| *m.timestamp() > self.read_at)
            .count()
    }
}

impl Tui {
    pub fn new() -> Self {
        use termion::input::TermRead;
        use termion::raw::IntoRawMode;

        let screenguard = termion::screen::AlternateScreen::from(::std::io::stdout());
        let rawguard = std::io::stdout()
            .into_raw_mode()
            .expect("Couldn't put the terminal in raw mode");

        let (sender, reciever) = futures::channel::mpsc::unbounded();

        // Set up a signal handler so we get notified when the terminal is resized
        // Must be called before any threads are launched
        let winch_send = sender.clone();
        let signals = signal_hook::iterator::Signals::new(&[::libc::SIGWINCH])
            .expect("Couldn't register resize signal handler");
        tokio::task::spawn_blocking(move || {
            for _ in &signals {
                let mut winch_send = winch_send.clone();
                tokio::spawn(async move { winch_send.send(ConnEvent::Resize).await });
            }
        });

        // Launch a background thread to feed input from stdin
        // Note this isn't raw keyboard events, it's termion's opinion of an event
        let send = sender.clone();
        tokio::task::spawn_blocking(move || {
            for event in std::io::stdin().events() {
                if let Ok(ev) = event {
                    let mut send = send.clone();
                    tokio::spawn(async move { send.send(ConnEvent::Input(ev)).await });
                }
            }
        });

        // Launch a task to ping back attempts to type in the errors tab
        // This is pretty stupid, but otherwise we have to special-case the Client
        // tab in other parts of the code
        let mut to_tui = sender.clone();
        let (to_client, mut from_client) = futures::channel::mpsc::unbounded();
        tokio::spawn(async move {
            while let Some(ev) = from_client.next().await {
                if let TuiEvent::SendMessage {
                    contents, channel, ..
                } = ev
                {
                    to_tui
                        .send(ConnEvent::Message(Message {
                            server: "Client".into(),
                            channel,
                            sender: "You".into(),
                            contents,
                            timestamp: DateTime::now(),
                            reactions: Vec::new(),
                        }))
                        .await
                        .unwrap();
                }
            }
        });

        // Initialize with the Client's server which displays an error log
        let now = DateTime::now();
        let client = Server {
            channels: vec![Channel {
                messages: Vec::new(),
                name: "Errors".into(),
                read_at: now,
                latest: now,
                has_history: false,
                message_scroll_offset: 0,
                message_buffer: String::new(),
                channel_type: ChannelType::Normal,
            }],
            completer: None,
            channel_scroll_offset: 0,
            current_channel: 0,
            name: "Client".into(),
            sender: to_client,
        };

        Self {
            servers: CursorVec::new(client),
            longest_channel_name: 6, // "Client"
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

    pub fn sender(&self) -> UnboundedSender<ConnEvent> {
        self.sender.clone()
    }

    fn update_history(&mut self) {
        if !self.current_channel().has_history {
            let channel_to_update = self.current_channel().name.clone();
            let mut sender = self.servers.get().sender.clone();
            tokio::spawn(async move {
                sender
                    .send(TuiEvent::GetHistory {
                        channel: channel_to_update,
                    })
                    .await
                    .unwrap()
            });
        }
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
        server.channels[server.current_channel].read_at = chrono::Utc::now().into();
        let current_channel = &server.channels[server.current_channel];

        let mut sender = server.sender.clone();
        let event = TuiEvent::MarkRead {
            server: server.name.clone(),
            channel: current_channel.name.clone(),
        };
        tokio::spawn(async move { sender.send(event).await.unwrap() });
    }

    fn next_server(&mut self) {
        self.reset_current_unreads();
        self.servers.next();
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    fn previous_server(&mut self) {
        self.reset_current_unreads();
        self.servers.prev();
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    fn next_channel_unread(&mut self) {
        let server = self.servers.get_mut();
        if let Some(index) = (0..server.channels.len())
            .map(|i| (server.current_channel + i) % server.channels.len())
            .find(|i| server.channels[*i].is_unread() && *i != server.current_channel)
        {
            self.reset_current_unreads();
            self.servers.get_mut().current_channel = index;
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    fn previous_channel_unread(&mut self) {
        //NLL HACK
        let index = {
            let server = self.servers.get_mut();
            (0..server.channels.len())
                .map(|i| {
                    (server.current_channel + server.channels.len() - i) % server.channels.len()
                })
                .find(|i| server.channels[*i].is_unread() && *i != server.current_channel)
        };
        match index {
            None => {}
            Some(index) => {
                self.reset_current_unreads();
                self.servers.get_mut().current_channel = index;
            }
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    fn next_channel(&mut self) {
        self.reset_current_unreads();
        let server = self.servers.get_mut();
        server.current_channel += 1;
        if server.current_channel >= server.channels.len() {
            server.current_channel = 0;
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    fn previous_channel(&mut self) {
        self.reset_current_unreads();
        let server = self.servers.get_mut();
        if server.current_channel > 0 {
            server.current_channel -= 1;
        } else {
            server.current_channel = server.channels.len() - 1;
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
        self.update_history();
    }

    // Take by value because we need to own the allocation
    fn add_client_message(&mut self, message: String) {
        self.servers.get_first_mut().channels[0]
            .messages
            .push(ChanMessage::from(Message {
                server: "Client".into(),
                channel: "Errors".into(),
                contents: message,
                timestamp: chrono::Utc::now().into(),
                sender: "Client".into(),
                reactions: Vec::new(),
            }));
    }

    pub fn add_server(&mut self, mut server: Server) {
        server.channels.sort_by(|c1, c2| c1.name.cmp(&c2.name));
        server
            .channels
            .sort_by_key(|c| c.channel_type == ChannelType::DirectMessage);

        server.current_channel = server
            .channels
            .iter()
            .position(|c| c.name == "general")
            .unwrap_or(0);

        self.servers.push(server);

        self.longest_channel_name = self
            .servers
            .iter()
            .flat_map(|s| s.channels.iter().map(|c| c.name.len()))
            .max()
            .unwrap_or(0) as u16
            + 1;

        let previous_server_name = self.servers.get().name.clone();
        self.servers.sort_by_key(|s| s.name.clone());
        // TODO properly pin the client tab to the far-left position
        // This is a temporary hack, and may suggest that instead of a CursorVec I should have
        // functions like current_server() as I have current_channel()
        self.servers.sort_by_key(|s| &s.name != "Client");
        while self.servers.get().name != previous_server_name {
            self.servers.next();
        }
    }

    fn add_message(&mut self, message: Message) {
        let channel = match self
            .servers
            .iter_mut()
            .find(|s| s.name == message.server)
            .or_else(|| {
                error!("Unable to add message, no server named {}", message.server);
                None
            })
            .and_then(|server| {
                server
                    .channels
                    .iter_mut()
                    .find(|c| c.name == message.channel)
            }) {
            Some(c) => c,
            None => {
                error!(
                    "Unable to add message, no channel named {} in server {}",
                    message.channel, message.server
                );
                return;
            }
        };

        let needs_sort = channel
            .messages
            .last()
            .map(|m| *m.timestamp())
            .unwrap_or(message.timestamp)
            > message.timestamp;

        channel.messages.push(message.into());

        if needs_sort {
            channel
                .messages
                .sort_unstable_by(|m1, m2| m1.timestamp().cmp(&m2.timestamp()));
        }
        channel.latest = channel
            .messages
            .last()
            .map(|m| *m.timestamp())
            .unwrap_or(channel.latest);
    }

    async fn send_message(&mut self) {
        let contents = self.current_channel().message_buffer.clone();
        self.current_channel_mut().message_buffer.clear();
        if self.servers.tell() == 0 && !contents.starts_with('/') {
            self.add_client_message(contents);
            return;
        }
        let current_server_name = self.servers.get().name.clone();
        let current_channel_name = self.current_channel().name.clone();
        if contents.starts_with("+:") {
            if let Some(ts) = self
                .current_channel()
                .messages
                .last()
                .map(|m| *m.timestamp())
            {
                let reaction = &contents[2..contents.len() - 1];
                self.servers
                    .get_mut()
                    .sender
                    .send(TuiEvent::AddReaction {
                        reaction: reaction.into(),
                        server: current_server_name,
                        channel: current_channel_name,
                        timestamp: ts,
                    })
                    .await
                    .unwrap()
            } else {
                self.add_client_message(
                    "Can't react to most recent message if there are no messages in this channel!"
                        .to_string(),
                );
            }
        } else if contents == "/mark" || contents == "/m" {
            // Mark current channel as read
            self.reset_current_unreads();
        } else if contents.starts_with("/c") {
            // Find and switch to the specified channel
            if let Some(requested_channel) = contents.splitn(2, ' ').nth(1) {
                if let Some(index) = self
                    .servers
                    .get()
                    .channels
                    .iter()
                    .position(|c| c.name == requested_channel)
                {
                    self.reset_current_unreads();
                    self.servers.get_mut().current_channel = index;
                    self.cursor_pos =
                        min(self.cursor_pos, self.current_channel().message_buffer.len());
                    self.update_history();
                } else {
                    error!("unknown channel {}", requested_channel);
                }
            }
        } else if contents.starts_with("/s") {
            // Find and switch to a server
            if let Some(requested_server) = contents.splitn(2, ' ').nth(1) {
                let index = self.servers.iter().position(|s| s.name == requested_server);
                if let Some(index) = index {
                    self.reset_current_unreads();
                    while self.servers.tell() != index {
                        self.servers.next();
                    }
                    self.cursor_pos =
                        min(self.cursor_pos, self.current_channel().message_buffer.len());
                    self.update_history();
                } else {
                    error!("unknown server {}", requested_server);
                }
            }
        } else if contents == "/url" {
            // The /url command searches for a URL mentioned in the current channel and
            // copies it to the clipboard if one is found
            use std::io::Write;
            use std::process::{Command, Stdio};
            if let Some(mut url) = self
                .current_channel()
                .messages
                .iter()
                .rev()
                .filter_map(|message| URL_REGEX.get_first(&message.raw.as_bytes()))
                .next()
            {
                if url.ends_with(&[b'>']) {
                    url = &url[..url.len() - 1];
                }
                let _ = Command::new("xclip")
                    .arg("-selection")
                    .arg("clipboard")
                    .stdin(Stdio::piped())
                    .spawn()
                    .and_then(|mut child| child.stdin.as_mut().unwrap().write_all(url))
                    .map_err(|e| error!("{:#?}", e));
            }
        } else if contents.starts_with('/') {
            self.servers
                .get_mut()
                .sender
                .send(TuiEvent::Command {
                    server: current_server_name,
                    channel: current_channel_name,
                    command: String::from(&contents[1..]),
                })
                .await
                .unwrap();
        } else {
            self.servers
                .get_mut()
                .sender
                .send(TuiEvent::SendMessage {
                    server: current_server_name,
                    channel: current_channel_name,
                    contents,
                })
                .await
                .unwrap();
        }
    }

    fn draw(&mut self, render_buffer: &mut String) {
        use std::fmt::Write;
        use termion::color::Fg;
        use termion::cursor::Goto;
        use termion::{color, style};

        let (terminal_width, terminal_height) =
            termion::terminal_size().expect("TUI draw couldn't get terminal dimensions");

        if terminal_height != self.previous_terminal_height {
            render_buffer.clear();
            let _ = write!(render_buffer, "{}", termion::clear::All);

            for i in 1..=terminal_height {
                let _ = write!(render_buffer, "{}|", Goto(CHAN_WIDTH, i));
            }
            self.truncate_buffer_to = render_buffer.len();
            self.previous_terminal_height = terminal_height;
        } else {
            render_buffer.truncate(self.truncate_buffer_to);
        }

        let remaining_width = (terminal_width - CHAN_WIDTH) as usize;

        // Draw the message input area
        // We need this message area height to render the channel messages
        // More NLL hacking
        let total_chars = self.current_channel().message_buffer.chars().count();
        let rows = (total_chars / remaining_width) + 1;
        for row in (0..rows).rev() {
            let _ = write!(
                render_buffer,
                "{}",
                Goto(CHAN_WIDTH + 1, terminal_height - (rows - row - 1) as u16)
            );
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
        let mut draw_unread_marker = self.current_channel().is_unread();

        let offset = self.current_channel().message_scroll_offset;

        let mut row = message_area_height - 1;
        let mut skipped = 0;
        'outer: for (m, message) in self
            .current_channel_mut()
            .messages
            .iter_mut()
            .rev()
            .enumerate()
        {
            // Unread marker
            if (draw_unread_marker) && (m == num_unreads) {
                let _ = write!(
                    render_buffer,
                    "{}{}",
                    Goto(CHAN_WIDTH + 1, row),
                    Fg(color::Red)
                );
                render_buffer.extend(::std::iter::repeat('-').take(remaining_width));
                let _ = write!(render_buffer, "{}", Fg(color::Reset));
                row -= 1;
                draw_unread_marker = false;
                if row == 1 {
                    break 'outer;
                }
            }

            for line in message.formatted_to(remaining_width).lines().rev() {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                let _ = write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, row));
                render_buffer.push_str(line);
                row -= 1;
                if row == 1 {
                    break 'outer;
                }
            }
        }

        // If we didn't draw the unread marker, put it at the top of the screen
        if draw_unread_marker {
            let _ = write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, max(2, row)));
            let _ = write!(render_buffer, "{}", Fg(color::Red));
            render_buffer.extend(::std::iter::repeat('-').take(remaining_width));
            let _ = write!(render_buffer, "{}", Fg(color::Reset));
        }

        // Draw all the server names across the top
        let _ = write!(render_buffer, "{}", Goto(CHAN_WIDTH + 1, 1)); // Move to the top-right corner
        let num_servers = self.servers.len();
        for (s, server) in self
            .servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            if s == self.servers.tell() {
                let _ = write!(
                    render_buffer,
                    "{}{}{}",
                    style::Bold,
                    server.name,
                    style::Reset
                );
            } else if server.has_unreads() {
                let _ = write!(
                    render_buffer,
                    "{}{}{}",
                    Fg(color::Red),
                    server.name,
                    Fg(color::Reset),
                );
            } else {
                let _ = write!(
                    render_buffer,
                    "{}{}{}",
                    Fg(color::AnsiValue::rgb(3, 3, 3)),
                    server.name,
                    Fg(color::Reset),
                );
            }
            let _ = write!(
                render_buffer,
                "{}",
                if s == num_servers - 1 { "" } else { " • " }
            );
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
                    let _ = write!(f, "{}", name);
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
                let draw_at = (c - server.channel_scroll_offset) as u16
                    + 1
                    + ((channel.channel_type == ChannelType::DirectMessage) as u16);
                // Skip a row if we're transitioning from the normal to DM channels
                if c == server.current_channel {
                    let _ = write!(render_buffer, "{}{}", Goto(1, draw_at), style::Bold);
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    let _ = write!(render_buffer, "{}", style::Reset);
                } else if channel.is_unread() {
                    let _ = write!(render_buffer, "{}{}", Goto(1, draw_at), Fg(color::Red));
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    let _ = write!(render_buffer, "{}", style::Reset);
                } else {
                    let gray = color::AnsiValue::rgb(3, 3, 3);
                    let _ = write!(render_buffer, "{}{}", Goto(1, draw_at), Fg(gray));
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    let _ = write!(render_buffer, "{}", style::Reset);
                }
            }
        }

        let _ = write!(
            render_buffer,
            "{}",
            Goto(
                CHAN_WIDTH + 1 + (self.cursor_pos % remaining_width) as u16,
                terminal_height - (total_chars / remaining_width) as u16
                    + (self.cursor_pos / remaining_width) as u16
            )
        );
        {
            use std::io::Write;
            let out = std::io::stdout();
            let mut lock = out.lock();
            lock.write_all(render_buffer.as_bytes())
                .expect("Unable to write to stdout");
            lock.flush().unwrap();
        }
    }

    async fn handle_input(&mut self, event: &::termion::event::Event) {
        use termion::event::Event::*;
        use termion::event::Key::*;
        use termion::event::{MouseButton, MouseEvent};

        match *event {
            Key(Char('\n')) => {
                if !self.current_channel().message_buffer.is_empty() {
                    self.send_message().await;
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
            Key(Ctrl('c')) => {
                error!("got shutdown request");
                self.shutdown = true;
            }
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
                    // Pick a source to autocomplete from
                    let search_name_fragment = self
                        .current_channel()
                        .message_buffer
                        .splitn(2, ' ')
                        .nth(1)
                        .unwrap_or_default();

                    if self.current_channel().message_buffer.starts_with("/c ") {
                        // Autocomplete from current server's channel names
                        self.autocompletions = self
                            .servers
                            .get()
                            .channels
                            .iter()
                            .map(|c| c.name.to_string())
                            .filter(|name| name.starts_with(search_name_fragment))
                            .collect();
                    } else if self.current_channel().message_buffer.starts_with("/s ") {
                        // Autocomplete from available server names
                        self.autocompletions = self
                            .servers
                            .iter()
                            .map(|s| s.name.to_string())
                            .filter(|name| name.starts_with(search_name_fragment))
                            .collect();
                    } else if self
                        .current_channel()
                        .message_buffer
                        .starts_with("/upload ")
                    {
                        fn complete_from(argument: &str) -> Option<Vec<String>> {
                            use std::path::Path;

                            let current_dir = std::env::current_dir().unwrap();
                            let full_path = current_dir.join(Path::new(&argument));
                            let start_of_entry = full_path.file_name()?.to_str()?;
                            let dir_part = full_path.parent()?;

                            let mut output = Vec::new();
                            // Autocomplete from the path provided
                            for entry in std::fs::read_dir(dir_part).unwrap() {
                                let entry = entry.unwrap();
                                let path = entry.path();
                                if path.file_name()?.to_str()?.starts_with(start_of_entry) {
                                    let mut suggestion = path
                                        .strip_prefix(&current_dir)
                                        .unwrap()
                                        .to_str()?
                                        .to_string();
                                    if path.is_dir() {
                                        suggestion.push(std::path::MAIN_SEPARATOR);
                                    }
                                    output.push(suggestion);
                                }
                            }
                            Some(output)
                        }

                        let argument = self
                            .current_channel()
                            .message_buffer
                            .splitn(2, ' ')
                            .nth(1)
                            .unwrap_or(".");

                        self.autocompletions = complete_from(argument).unwrap_or_default();
                        self.autocompletions.sort();
                    } else {
                        self.autocompletions = if let Some(last_word) = self
                            .current_channel()
                            .message_buffer
                            .split_whitespace()
                            .last()
                        {
                            self.servers
                                .get()
                                .completer
                                .as_ref()
                                .map(|c| c.autocomplete(last_word))
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        }
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
                let current_server_name = self.servers.get().name.clone();
                let current_channel_name = self.current_channel().name.clone();
                self.servers
                    .get_mut()
                    .sender
                    .send(TuiEvent::SendTyping {
                        server: current_server_name,
                        channel: current_channel_name,
                    })
                    .await
                    .unwrap();
                self.autocompletions.clear();
                self.autocomplete_index = 0;
                let current_pos = self.cursor_pos as usize;
                self.current_channel_mut()
                    .message_buffer
                    .insert(current_pos, c);
                self.cursor_pos += 1;
            }
            Unsupported(ref bytes) => match bytes.as_slice() {
                [27, 79, 65] => {
                    self.sender
                        .send(ConnEvent::Input(Mouse(MouseEvent::Press(
                            MouseButton::WheelUp,
                            1,
                            1,
                        ))))
                        .await
                        .unwrap();
                }
                [27, 79, 66] => {
                    self.sender
                        .send(ConnEvent::Input(Mouse(MouseEvent::Press(
                            MouseButton::WheelDown,
                            1,
                            1,
                        ))))
                        .await
                        .unwrap();
                }

                _ => {}
            },
            _ => {}
        }
    }

    async fn handle_event(&mut self, event: ConnEvent) {
        match event {
            ConnEvent::Resize => {} // Will be redrawn because we got an event
            ConnEvent::Input(event) => {
                self.handle_input(&event).await;
            }
            ConnEvent::Message(message) => {
                self.add_message(message);
            }
            ConnEvent::MessageEdited {
                server,
                channel,
                contents,
                timestamp,
            } => {
                if let Some(msg) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .or_else(|| {
                        error!("Couldn't process edit request: No server named {}", server);
                        None
                    }).and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                    .or_else(|| {
                        error!(
                            "Couldn't process edit request: No channel named {} in server {}",
                            channel, server
                        );
                        None
                    }).and_then(|c| {
                        c.messages
                            .iter_mut()
                            .rev()
                            .find(|m| m.timestamp() == &timestamp)
                    }).or_else(|| {
                        error!(
                            "Couldn't process edit request: No message with timestamp {} in server: {}, channel: {}",
                            timestamp, server, channel,
                        );
                        None
                    }) {
                    msg.edit_to(contents);
                    }
            }
            ConnEvent::ReactionAdded {
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
                            .find(|m| m.timestamp() == &timestamp)
                    })
                {
                    msg.add_reaction(&reaction);
                } else {
                    error!(
                        "Couldn't add reaction {} to message: server: {}, channel: {}, timestamp: {}",
                        reaction, server, channel, timestamp
                    );
                }
            }
            ConnEvent::ReactionRemoved {
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
                            .find(|m| m.timestamp() == &timestamp)
                    })
                {
                    msg.remove_reaction(&reaction);
                } else {
                    error!(
                        "Couldn't remove reaction {} from message server: {}, channel: {}, timestamp: {}",
                        reaction, server, channel, timestamp
                    );
                }
            }
            ConnEvent::Error(message) => {
                self.add_client_message(message);
            }
            ConnEvent::HistoryLoaded {
                messages,
                server,
                channel,
            } => {
                if let Some(c) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                {
                    // TODO: This duplicate check is quadratic and maybe not what we even want;
                    // duplicate timestamps might be okay. Should we compare everything instead?
                    for new_message in messages {
                        if !c.messages.iter().any(|m| *m.timestamp() == new_message.timestamp) {
                            c.messages.push(new_message.into());
                        }
                    }
                    c.messages
                        .sort_unstable_by(|m1, m2| m1.timestamp().cmp(&m2.timestamp()));
                    c.has_history = true;
                } else {
                    error!(
                        "Got history for an unknown channel {} in server {}",
                        channel, server
                    );
                }
            }
            ConnEvent::ServerConnected(server) => {
                self.add_server(server);
            }
            ConnEvent::MarkChannelRead {
                server,
                channel,
                read_at,
                latest,
            } => {
                let current_channel_name = self.current_channel().name.clone();
                if let Some(c) = self
                    .servers
                    .iter_mut()
                    .find(|s| s.name == server)
                    .and_then(|server| server.channels.iter_mut().find(|c| c.name == channel))
                {
                    if current_channel_name != c.name {
                        read_at.map(|t| c.read_at = t);
                        latest.map(|t| c.latest = t);
                    }
                }
            }
        }
    }

    pub async fn run(mut self) {
        let mut render_buffer = String::new();
        self.draw(&mut render_buffer);
        while let Some(event) = self.events.next().await {
            self.handle_event(event).await;

            self.draw(&mut render_buffer);

            if self.shutdown {
                return;
            }
        }
    }
}
