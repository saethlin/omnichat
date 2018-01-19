#![feature(box_patterns)]
#[macro_use]
extern crate failure;
extern crate itertools;
extern crate serde_json;
extern crate slack_api;
extern crate termion;
extern crate websocket;

use termion::raw::IntoRawMode;

mod tui;
use tui::TUI;
mod conn;
use conn::Conn;
mod slack_conn;
use slack_conn::SlackConn;
mod bimap;

fn api_key() -> String {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open("/home/ben/slack_api_key").expect("Couldn't find API key");
    let mut api_key = String::with_capacity(128);
    file.read_to_string(&mut api_key)
        .expect("Unable to read API key");
    api_key.trim().to_owned()
}

fn main() {
    let _guard = std::io::stdout()
        .into_raw_mode()
        .expect("Couldn't put stdout into raw mode");

    let mut tui = TUI::new();
    tui.draw();

    let slack_config = conn::ServerConfig::Slack { token: api_key() };
    let connection =
        SlackConn::new(slack_config, tui.sender()).expect("Failed to crate slack connection");
    tui.add_server(connection);
    tui.draw();

    tui.run();
}
