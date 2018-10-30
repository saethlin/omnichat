use chan_message::ChanMessage;
use conn::{Conn, DateTime, Event, IString, Message};
use cursor_vec::CursorVec;
use std::cmp::{max, min};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};

const CHAN_WIDTH: u16 = 20;

pub struct Tui {
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
    name: IString,
    read_at: DateTime,
    message_scroll_offset: usize,
    message_buffer: String,
}

impl Channel {
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
        use std::thread;
        use termion::input::TermRead;
        use termion::raw::IntoRawMode;

        let screenguard = ::termion::screen::AlternateScreen::from(::std::io::stdout());
        let rawguard = ::std::io::stdout()
            .into_raw_mode()
            .expect("Couldn't put the terminal in raw mode");

        let (sender, reciever) = sync_channel(100);

        // Must be called before any threads are launched
        let winch_send = sender.clone();
        let signals = ::signal_hook::iterator::Signals::new(&[::libc::SIGWINCH])
            .expect("Couldn't register resize signal handler");
        thread::spawn(move || {
            for _ in &signals {
                let _ = winch_send.send(Event::Resize);
            }
        });

        let send = sender.clone();
        thread::spawn(move || {
            for event in ::std::io::stdin().events() {
                if let Ok(ev) = event {
                    let _ = send.send(Event::Input(ev));
                }
            }
        });

        Self {
            servers: CursorVec::new(Server {
                channels: vec!["Errors"]
                    .iter()
                    .map(|name| Channel {
                        messages: Vec::new(),
                        name: (*name).into(),
                        read_at: ::chrono::Utc::now().into(),
                        message_scroll_offset: 0,
                        message_buffer: String::new(),
                    }).collect(),
                connection: ClientConn::create_on(sender.clone()),
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
            server.channels[server.current_channel].read_at = ::chrono::Utc::now().into();
            let current_channel = &server.channels[server.current_channel];

            server.connection.mark_read(&current_channel.name);
        }
    }

    fn next_server(&mut self) {
        self.reset_current_unreads();
        self.servers.next();
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
    }

    fn previous_server(&mut self) {
        self.reset_current_unreads();
        self.servers.prev();
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
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
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
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
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
    }

    fn next_channel(&mut self) {
        self.reset_current_unreads();
        // NLL HACK
        {
            let server = self.servers.get_mut();
            server.current_channel += 1;
            if server.current_channel >= server.channels.len() {
                server.current_channel = 0;
            }
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
    }

    fn previous_channel(&mut self) {
        self.reset_current_unreads();
        // NLL HACK
        {
            let server = &mut self.servers.get_mut();
            if server.current_channel > 0 {
                server.current_channel -= 1;
            } else {
                server.current_channel = server.channels.len() - 1;
            }
        }
        self.cursor_pos = min(self.cursor_pos, self.current_channel().message_buffer.len());
    }

    // Take by value because we need to own the allocation
    fn add_client_message(&mut self, message: String) {
        self.servers.get_first_mut().channels[0]
            .messages
            .push(ChanMessage::from(::conn::Message {
                server: "Client".into(),
                channel: "Errors".into(),
                contents: message,
                timestamp: ::chrono::Utc::now().into(),
                sender: "Client".into(),
                reactions: Vec::new(),
            }));
    }

    pub fn add_server(&mut self, connection: Box<Conn>) {
        let mut channels = connection.channels().to_vec();
        channels.sort();

        self.servers.push(Server {
            channels: channels
                .into_iter()
                .map(|name| Channel {
                    messages: Vec::new(),
                    name,
                    read_at: ::chrono::Utc::now().into(), // This is a Bad Idea; we've marked everything as read by default, when we have no right to but I'm not sure what else to use as a default
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

    fn add_message(&mut self, message: Message) {
        let channel = match self
            .servers
            .iter_mut()
            .find(|s| s.name == message.server)
            .or_else(|| {
                error!("Unable to add message, no server named {}", message.server);
                None
            }).and_then(|server| {
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
    }

    fn send_message(&mut self) {
        let contents = self.current_channel().message_buffer.clone();
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
                    .get()
                    .connection
                    .add_reaction(reaction, &current_channel_name, ts);
            } else {
                self.add_client_message(
                    "Can't react to most recent message if there are no messages in this channel!"
                        .to_string(),
                );
            }
        } else if contents.starts_with('/') {
            self.servers
                .get_mut()
                .connection
                .handle_cmd(&current_channel_name, &contents[1..]);
        } else {
            self.servers
                .get_mut()
                .connection
                .send_channel_message(&current_channel_name, &contents);
        }
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
            let _ = write!(render_buffer, "{}", ::termion::clear::All);

            for i in 1..=terminal_height {
                let _ = write!(render_buffer, "{}|", Goto(CHAN_WIDTH, i));
            }
            self.truncate_buffer_to = render_buffer.len();
            self.previous_terminal_height = terminal_height;
        } else {
            render_buffer.truncate(self.truncate_buffer_to);
        }

        let remaining_width = (terminal_width - CHAN_WIDTH) as usize;
        /*
        for message in &mut self.current_channel_mut().messages {
            message.format(remaining_width);
        }
        */

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
        let mut draw_unread_marker = num_unreads > 0;

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
                if c == server.current_channel {
                    let _ = write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        style::Bold
                    );
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    let _ = write!(render_buffer, "{}", style::Reset);
                } else if channel.num_unreads() > 0 {
                    let _ = write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(color::Red)
                    );
                    write_shortened_name(render_buffer, &channel.name, CHAN_WIDTH as usize);
                    let _ = write!(render_buffer, "{}", style::Reset);
                } else {
                    let gray = color::AnsiValue::rgb(3, 3, 3);
                    let _ = write!(
                        render_buffer,
                        "{}{}",
                        Goto(1, (c - server.channel_scroll_offset) as u16 + 1),
                        Fg(gray)
                    );
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
            let out = ::std::io::stdout();
            let mut lock = out.lock();
            lock.write_all(render_buffer.as_bytes())
                .expect("Unable to write to stdout");
            let _ = lock.flush();
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
                [27, 79, 65] => {
                    let _ = self.sender.send(Event::Input(Mouse(MouseEvent::Press(
                        MouseButton::WheelUp,
                        1,
                        1,
                    ))));
                }
                [27, 79, 66] => {
                    let _ = self.sender.send(Event::Input(Mouse(MouseEvent::Press(
                        MouseButton::WheelDown,
                        1,
                        1,
                    ))));
                }

                _ => {}
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
                self.add_message(message);
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
                            .find(|m| m.timestamp() == &timestamp)
                    }) {
                    msg.add_reaction(&reaction);
                } else {
                    error!(
                        "Couldn't add reaction {} to message: server: {}, channel: {}, timestamp: {}",
                        reaction, server, channel, timestamp
                    );
                }
            }
            Event::ReactionRemoved {
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
                    }) {
                        msg.remove_reaction(&reaction);
                } else {
                    error!(
                        "Couldn't remove reaction {} from message server: {}, channel: {}, timestamp: {}",
                        reaction, server, channel, timestamp
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
    sender: SyncSender<Event>,
    channel_names: [IString; 1],
}

impl ClientConn {
    pub fn create_on(sender: SyncSender<Event>) -> Box<Conn> {
        Box::new(ClientConn {
            sender,
            channel_names: ["Errors".into()],
        })
    }
}

impl Conn for ClientConn {
    fn name(&self) -> &str {
        "Client"
    }

    fn send_channel_message(&mut self, channel: &str, contents: &str) {
        let _ = self.sender.send(Event::Message(Message {
            server: "Client".into(),
            channel: channel.into(),
            contents: contents.into(),
            sender: "You".into(),
            timestamp: ::chrono::Utc::now().into(),
            reactions: Vec::new(),
        }));
    }

    fn channels(&self) -> &[IString] {
        &self.channel_names
    }
}
