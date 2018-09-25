use chan_message::ChanMessage;
use conn::{Conn, DateTime, Event, IString, Message};
use cursor_vec::CursorVec;
use pancurses::{
    COLOR_BLACK, COLOR_BLUE, COLOR_CYAN, COLOR_GREEN, COLOR_MAGENTA, COLOR_RED, COLOR_WHITE,
    COLOR_YELLOW,
};
use std::cmp::{max, min};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

const CHAN_WIDTH: i32 = 20;

const COLOR_TABLE: [i16; 8] = [
    COLOR_BLACK,
    COLOR_BLUE,
    COLOR_GREEN,
    COLOR_CYAN,
    COLOR_RED,
    COLOR_MAGENTA,
    COLOR_YELLOW,
    COLOR_WHITE,
];

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
            .take_while(|m| m.timestamp() > &self.read_at)
            .count()
    }
}

impl Tui {
    pub fn new() -> Self {
        use std::thread;
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

        Self {
            servers: CursorVec::new(Server {
                channels: vec!["Errors", "Mentions"]
                    .iter()
                    .map(|name| Channel {
                        messages: Vec::new(),
                        name: (*name).into(),
                        read_at: ::chrono::Utc::now(),
                        message_scroll_offset: 0,
                        message_buffer: String::new(),
                    }).collect(),
                connection: ClientConn::new(sender.clone()),
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
            server.channels[server.current_channel].read_at = ::chrono::Utc::now();
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
                is_mention: false,
                timestamp: ::chrono::Utc::now(),
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
                    name: name,
                    read_at: ::chrono::Utc::now(), // This is a Bad Idea; we've marked everything as read by default, when we have no right to but I'm not sure what else to use as a default
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
        if message.is_mention {
            self.servers.get_first_mut().channels[1]
                .messages
                .push(message.clone().into());
        }

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
            .unwrap_or(message.timestamp.clone())
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
                .map(|m| m.timestamp().clone())
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
        } else {
            self.servers
                .get_mut()
                .connection
                .send_channel_message(&current_channel_name, &contents);
        }
    }

    fn pancurses_draw(&mut self, window: &mut ::pancurses::Window) {
        window.erase();
        let (terminal_height, terminal_width) = window.get_max_yx();

        let remaining_width = (terminal_width - CHAN_WIDTH - 1) as usize;

        // Draw the vertical line
        window.mv(0, CHAN_WIDTH);
        window.vline('|', terminal_height);

        // Draw the message input area
        // We need this message area height to render the channel messages
        // More NLL hacking
        let total_chars = self.current_channel().message_buffer.chars().count();
        let rows = (total_chars / remaining_width) + 1;
        for row in (0..rows).rev() {
            window.mv(terminal_height - (rows - row) as i32, CHAN_WIDTH + 1);
            window.addstr(
                self.current_channel()
                    .message_buffer
                    .chars()
                    .skip(remaining_width * row)
                    .take(remaining_width)
                    .collect::<String>(), // TODO: senesless allocation
            );
        }

        let message_area_height = terminal_height - rows as i32 + 1;

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
                window.mv(row, CHAN_WIDTH + 1);
                window.attrset(::pancurses::COLOR_PAIR(4));
                window.hline('-', remaining_width as i32);
                window.attrset(::pancurses::COLOR_PAIR(7));

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
                window.mvaddstr(row, CHAN_WIDTH + 1, line);
                row -= 1;
                if row == 1 {
                    break 'outer;
                }
            }
        }

        // If we didn't draw the unread marker, put it at the top of the screen
        if draw_unread_marker {
            window.mv(max(2, row), CHAN_WIDTH + 1); // TODO: unclear on this 2
            window.attrset(::pancurses::COLOR_PAIR(4));
            window.hline('-', remaining_width as i32);
            window.attrset(::pancurses::COLOR_PAIR(7));
        }

        // Draw all the server names across the top
        window.mv(0, CHAN_WIDTH + 1); // Move to top-left corner
        let num_servers = self.servers.len();
        for (s, server) in self
            .servers
            .iter()
            .enumerate()
            .skip(self.server_scroll_offset)
        {
            if s == self.servers.tell() {
                window.attron(::pancurses::Attribute::Bold);
                window.addstr(&server.name);
                window.attroff(::pancurses::Attribute::Bold);
            } else if server.has_unreads() {
                window.attrset(::pancurses::COLOR_PAIR(4));
                window.addstr(&server.name);
                window.attrset(::pancurses::COLOR_PAIR(7));
            } else {
                window.addstr(&server.name);
            }
            window.addstr(if s == num_servers - 1 { "" } else { " • " });
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

            fn add_shortened_name(win: &::pancurses::Window, name: &str) {
                if name.chars().count() < CHAN_WIDTH as usize {
                    win.addstr(name);
                } else {
                    win.addstr(&name[..CHAN_WIDTH as usize - 4]);
                    win.addstr("...");
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
                    window.attron(::pancurses::Attribute::Bold);
                    window.mv((c - server.channel_scroll_offset) as i32, 0);
                    add_shortened_name(&window, &channel.name);
                    window.attroff(::pancurses::Attribute::Bold);
                } else if channel.num_unreads() > 0 {
                    window.attrset(::pancurses::COLOR_PAIR(4));
                    window.mv((c - server.channel_scroll_offset) as i32, 0);
                    add_shortened_name(&window, &channel.name);
                    window.attrset(::pancurses::COLOR_PAIR(7));
                } else {
                    window.mv((c - server.channel_scroll_offset) as i32, 0);
                    add_shortened_name(&window, &channel.name);
                }
            }
        }

        window.mv(terminal_height - 1, CHAN_WIDTH + self.cursor_pos as i32 + 1);
        window.refresh();
    }

    fn handle_input(&mut self, event: ::pancurses::Input) {
        use pancurses::Input::*;
        match event {
            Character('\n') => {
                if !self.current_channel().message_buffer.is_empty() {
                    self.send_message();
                    self.current_channel_mut().message_buffer.clear();
                    self.cursor_pos = 0;
                }
            }
            Character('\u{7f}') => {
                // Backspace
                if self.cursor_pos > 0 {
                    let remove_pos = self.cursor_pos as usize - 1;
                    self.current_channel_mut().message_buffer.remove(remove_pos);
                    self.cursor_pos -= 1;
                }
            }
            Character('\u{3}') => self.shutdown = true,
            Character('\u{1}') => {
                self.previous_server();
            }
            Character('\u{4}') => {
                self.next_server();
            }
            Character('\u{17}') => {
                self.previous_channel();
            }
            Character('\u{13}') => {
                self.next_channel();
            }
            /*
            Key(Ctrl('q')) | Mouse(MouseEvent::Press(MouseButton::WheelUp, ..)) => {
                self.current_channel_mut().message_scroll_offset += 1;
            }
            Key(Ctrl('e')) | Mouse(MouseEvent::Press(MouseButton::WheelDown, ..)) => {
                let chan = self.current_channel_mut();
                let previous_offset = chan.message_scroll_offset;
                chan.message_scroll_offset = previous_offset.saturating_sub(1);
            }
            KeyLeft => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            KeyRight => {
                if self.cursor_pos < self.current_channel().message_buffer.len() {
                    self.cursor_pos += 1;
                }
            }
            */
            Character('\t') => {
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
            Character(c) => {
                self.autocompletions.clear();
                self.autocomplete_index = 0;
                let current_pos = self.cursor_pos as usize;
                self.current_channel_mut()
                    .message_buffer
                    .insert(current_pos, c);
                /*
                self.current_channel_mut()
                    .message_buffer
                    .push_str(&format!("{:x}", c));
                */
                self.cursor_pos += 1;
            }
            /*
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
            */
            _ => {}
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Resize => {} // Will be redrawn because we got an event
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

    pub fn run(mut self) {
        use std::time::{Duration, Instant};
        let mut window = ::pancurses::initscr();
        ::pancurses::raw();
        ::pancurses::noecho();
        window.nodelay(true);

        if ::pancurses::has_colors() {
            ::pancurses::start_color();
        }

        ::pancurses::init_color(COLOR_BLACK, 0, 0, 0);

        for (i, color) in COLOR_TABLE.into_iter().enumerate() {
            ::pancurses::init_pair(i as i16, *color, COLOR_BLACK);
        }

        loop {
            let start = Instant::now();
            //TODO: There is an error condition here, the channel can hang up
            while let Ok(event) = self.events.try_recv() {
                self.handle_event(event);
            }
            while let Some(event) = window.getch() {
                self.handle_input(event);
            }
            self.pancurses_draw(&mut window);
            let duration = Instant::now() - start;
            if duration < Duration::from_millis(16) {
                ::std::thread::sleep(Duration::from_millis(16) - duration);
            }

            if self.shutdown {
                return;
            }
        }
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        ::pancurses::endwin();
    }
}

pub struct ClientConn {
    sender: SyncSender<Event>,
    channel_names: [IString; 2],
}

impl ClientConn {
    pub fn new(sender: SyncSender<Event>) -> Box<Conn> {
        Box::new(ClientConn {
            sender,
            channel_names: ["Errors".into(), "Messages".into()],
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
            is_mention: false,
            timestamp: ::chrono::Utc::now(),
            reactions: Vec::new(),
        }));
    }

    fn channels(&self) -> &[IString] {
        &self.channel_names
    }
}
