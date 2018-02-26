extern crate discord;
#[macro_use]
extern crate failure;
extern crate pancurses;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate slack_api;
extern crate termion;
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

    let homedir = std::env::var("HOME").expect("You don't even have a $HOME? :'(");
    let config_path = PathBuf::from(homedir).join(".omnichat.toml");
    let mut contents = String::new();
    File::open(config_path)
        .expect("No config file found")
        .read_to_string(&mut contents)
        .expect("Couldn't read config file");

    let config: Config = toml::from_str(&contents).expect("Config is not valid TOML");

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
