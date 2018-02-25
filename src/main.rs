extern crate discord;
#[macro_use]
extern crate failure;
extern crate futures;
#[macro_use]
extern crate lazy_static;
extern crate pancurses;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate slack_api;
extern crate termion;
extern crate tokio_core;
extern crate toml;
extern crate websocket;

mod tui;
use tui::TUI;
mod conn;
mod slack_conn;
mod bimap;
mod discord_conn;

#[derive(Debug, Deserialize)]
struct Config {
    servers: Vec<conn::ServerConfig>,
}

fn main() {
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::Read;

    let homedir = std::env::var("HOME").expect("You don't even have a $HOME? :'(");
    let config_path = PathBuf::from(homedir).join(".omnichat.toml");
    let mut contents = String::new();
    File::open(config_path)
        .expect("No config file found")
        .read_to_string(&mut contents)
        .expect("Couldn't read config file");

    let config: Config = toml::from_str(&contents).expect("Config is not valid TOML");

    let mut tui = TUI::new();
    for c in config.servers.into_iter() {
        tui.add_server(c);
    }
    tui.run();
}
