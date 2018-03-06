extern crate discord;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate slack_api;
extern crate termion;
extern crate textwrap;
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
    use slack_conn::SlackConn;
    use discord_conn::DiscordConn;
    use tui::ClientConn;
    use conn::ServerConfig;
    use std::thread;
    use termion::raw::IntoRawMode;

    let homedir = std::env::var("HOME").unwrap_or_else(|_| {
        println!("You don't even have a $HOME? :'(");
        std::process::exit(1)
    });
    let config_path = PathBuf::from(homedir).join(".omnichat.toml");
    let mut contents = String::new();
    File::open(&config_path)
        .unwrap_or_else(|_| {
            println!("No config file found");
            std::process::exit(1)
        })
        .read_to_string(&mut contents)
        .unwrap_or_else(|_| {
            println!("Unable to read config file");
            std::process::exit(1)
        });

    let config: Config = toml::from_str(&contents).unwrap_or_else(|_| {
        println!("Config file {:?} is not valid TOML", &config_path);
        std::process::exit(1)
    });

    let _screenguard = termion::screen::AlternateScreen::from(std::io::stdout());
    let _rawguard = std::io::stdout().into_raw_mode().unwrap();

    let mut tui = TUI::new();
    let (conn_sender, conn_recv) = std::sync::mpsc::channel();
    for c in config.servers.iter().cloned() {
        let sender = tui.sender();
        let conn_sender = conn_sender.clone();
        thread::spawn(move || {
            let connection = match c {
                ServerConfig::Slack { token } => SlackConn::new(token, sender.clone()).unwrap(),
                ServerConfig::Discord { token, name } => {
                    DiscordConn::new(token, name, sender.clone()).unwrap()
                }
                ServerConfig::Client => ClientConn::new(sender.clone()).unwrap(),
            };
            conn_sender.send(connection).unwrap();
        });
    }
    for _ in 0..config.servers.len() {
        tui.add_server(conn_recv.recv().unwrap());
    }
    tui.run();
}
