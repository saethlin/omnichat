use tui::TUI;
use std::sync::{Arc, Mutex};
use failure::Error;

#[derive(Debug)]
pub enum ServerConfig {
    Slack {
        token: String,
    },
    IRC {
        domain: String,
        port: usize,
        hostname: String,
        realname: String,
        nicks: Vec<String>,
        auto_cmds: Vec<String>,
    },
}

#[derive(Debug, Fail)]
pub enum ConnError {
    #[fail(display = "Slack response was damaged")] SlackError,
    #[fail(display = "Websocket shut down")] WebsocketError,
}

pub trait Conn: Send {
    fn new(tui_handle: Arc<Mutex<TUI>>, config: ServerConfig) -> Result<(), Error>
    where
        Self: Sized;

    fn name(&self) -> String;

    fn handle_cmd(&mut self, cmd: String, args: Vec<String>);

    fn send_channel_message(&self, channel: &str, contents: &str);

    fn channels(&self) -> Vec<String>;

    fn autocomplete(&self, word: &str) -> Option<String>;
}
