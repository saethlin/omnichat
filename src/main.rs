#![feature(box_patterns)]
#[macro_use]
extern crate failure;
extern crate slack;
extern crate termion;

use std::sync::{Arc, Mutex};

use termion::input::TermRead;
use termion::event::Event::*;
use termion::event::Key::*;
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
    use std::io::prelude::*;

    let mut file = File::open("/home/ben/slack_api_key").expect("Couldn't find API key");
    let mut api_key = String::with_capacity(128);
    file.read_to_string(&mut api_key).unwrap();
    api_key.trim().to_owned()
}

fn main() {
    std::io::stdout()
        .into_raw_mode()
        .expect("Couldn't put stdout into raw mode");

    let tui_handle = Arc::new(Mutex::new(TUI::new()));
    tui_handle.lock().unwrap().draw();

    let slack_config = conn::ServerConfig::Slack { token: api_key() };
    SlackConn::new(tui_handle.clone(), slack_config).unwrap();

    for event in std::io::stdin().events() {
        let event = event.unwrap();
        let mut tui = tui_handle.lock().unwrap();
        match event {
            Key(Char('\n')) => {
                tui.send_message();
            }
            Key(Char(c)) => {
                tui.message_buffer.push(c);
            }
            Key(Backspace) => {
                tui.message_buffer.pop();
            }
            Key(Ctrl('c')) => {
                // TODO: Move the cursor back to the bottom-left
                break;
            }
            _ => {}
        }
        tui.draw().unwrap();
    }
}
