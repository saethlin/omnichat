extern crate discord;
#[macro_use]
extern crate failure;
extern crate futures;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate slack_api;
extern crate termion;
extern crate tokio_core;
extern crate toml;
extern crate websocket;

use termion::raw::IntoRawMode;

mod tui;
use tui::TUI;
mod conn;
use conn::Conn;
mod slack_conn;
use slack_conn::SlackConn;
mod bimap;
mod discord_conn;
use discord_conn::DiscordConn;


#[derive(Debug, Deserialize)]
struct Config {
    servers: Vec<conn::ServerConfig>,
}

fn main() {
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::Read;

    let _guard = std::io::stdout()
        .into_raw_mode()
        .expect("Couldn't put stdout into raw mode");

    let mut tui = TUI::new();
    tui.draw();

    let homedir = std::env::var("HOME").expect("You don't even have a $HOME? :'(");
    let config_path = PathBuf::from(homedir).join(".omnichat.toml");
    let mut contents = String::new();
    File::open(config_path)
        .expect("No config file found")
        .read_to_string(&mut contents)
        .expect("Couldn't read config file");

    let config: Config = toml::from_str(&contents).expect("Config is not valid TOML");

    for c in config.servers.into_iter() {
        let connection = match c {
            conn::ServerConfig::Slack{..} => SlackConn::new(c, tui.sender()),
            conn::ServerConfig::Discord{..} => DiscordConn::new(c, tui.sender()),
            _ => panic!("Unsupported config"),
        };
        tui.add_server(connection.unwrap());
    }
    tui.draw();
    tui.run();
}
