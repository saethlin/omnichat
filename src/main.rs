extern crate discord;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate slack_api;
extern crate spmc;
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
    discord_token: String,
    servers: Vec<conn::ServerConfig>,
}

fn main() {
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::Read;
    use std::sync::{Arc, RwLock};
    use slack_conn::SlackConn;
    use discord_conn::DiscordConn;
    use tui::ClientConn;
    use conn::ServerConfig;
    use std::thread;
    use termion::raw::IntoRawMode;

    // Hack to make static linking openssl work
    if let Err(std::env::VarError::NotPresent) = std::env::var("SSL_CERT_DIR") {
        std::env::set_var("SSL_CERT_DIR", "/etc/ssl/certs");
    }

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

    // Discord only permits one connection per user, so we need to redistribute the incoming events
    let dis = discord::Discord::from_user_token(&config.discord_token).unwrap();
    let (mut connection, info) = dis.connect().unwrap();
    let dis = Arc::new(RwLock::new(dis));

    let (discord_sender, discord_reciever) = spmc::channel();

    // Spawn a thread that copies the incoming Discord events out to every omnichat server
    let error_channel = tui.sender();
    thread::spawn(move || loop {
        match connection.recv_event() {
            Ok(ev) => discord_sender.send(ev).unwrap(),
            Err(discord::Error::Closed(..)) => break,
            Err(err) => error_channel
                .send(conn::Event::Error(format!("{:?}", err)))
                .unwrap(),
        }
    });

    let (conn_sender, conn_recv) = std::sync::mpsc::channel();
    for c in config.servers.iter().cloned() {
        let sender = tui.sender();
        let conn_sender = conn_sender.clone();

        let info = info.clone();
        let dis = dis.clone();
        let discord_reciever = discord_reciever.clone();
        thread::spawn(move || {
            let connection = match c {
                ServerConfig::Slack { token } => SlackConn::new(token, sender.clone()).unwrap(),
                ServerConfig::Discord { name } => {
                    DiscordConn::new(dis, info, discord_reciever, &name, sender.clone()).unwrap()
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
