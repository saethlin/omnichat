extern crate backoff;
extern crate discord;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate irc;
#[macro_use]
extern crate lazy_static;
extern crate regex;
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
//mod irc_conn;

#[derive(Debug, Deserialize, Clone)]
struct SlackConfig {
    token: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DiscordConfig {
    name: String,
}

/*
#[derive(Debug, Deserialize, Clone)]
struct IrcConfig {
    name: String,
    nick: String,
    port: u16,
}
*/

#[derive(Debug, Deserialize)]
struct Config {
    discord_token: Option<String>,
    slack: Option<Vec<SlackConfig>>,
    discord: Option<Vec<DiscordConfig>>,
    //irc: Option<Vec<IrcConfig>>,
}

fn main() {
    use std::path::PathBuf;
    use std::fs::File;
    use std::io::Read;
    use std::sync::{Arc, RwLock};
    use slack_conn::SlackConn;
    use discord_conn::DiscordConn;
    use std::thread;
    use termion::raw::IntoRawMode;
    //use irc_conn::IrcConn;

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
            println!(
                "No config file found, expected a config file at {:?}",
                config_path
            );
            std::process::exit(1)
        })
        .read_to_string(&mut contents)
        .unwrap_or_else(|_| {
            println!("Unable to read config file at {:?}", &config_path);
            std::process::exit(1)
        });

    let config: Config = toml::from_str(&contents).unwrap_or_else(|_| {
        println!("{:?} is not a valid omnichat config file", &config_path);
        std::process::exit(1)
    });

    let _screenguard = termion::screen::AlternateScreen::from(std::io::stdout());
    let _rawguard = std::io::stdout().into_raw_mode().unwrap();

    let mut tui = TUI::new();

    let (conn_sender, conn_recv) = std::sync::mpsc::channel();

    /*
    if let Some(irc) = config.irc {
        for i in irc {
            let sender = tui.sender();
            let conn_sender = conn_sender.clone();
            thread::spawn(
                move || match IrcConn::new(i.nick, i.name, i.port, sender.clone()) {
                    Ok(connection) => conn_sender.send(connection).unwrap(),
                    Err(err) => sender
                        .send(conn::Event::Error(format!("{:?}", err)))
                        .unwrap(),
                },
            );
        }
    }
    */

    // Start all the slack connections first, because we can't do the Discord stuff fully async
    if let Some(slack) = config.slack {
        for c in slack {
            let sender = tui.sender();
            let conn_sender = conn_sender.clone();
            thread::spawn(move || match SlackConn::new(c.token, sender.clone()) {
                Ok(connection) => conn_sender.send(connection).unwrap(),
                Err(err) => sender
                    .send(conn::Event::Error(format!("{:?}", err)))
                    .unwrap(),
            });
        }
    }

    // Discord only permits one connection per user, so we need to redistribute the incoming events
    if let (&Some(ref discord_token), &Some(ref discord)) = (&config.discord_token, &config.discord)
    {
        // This operation is blocking, but is the only I/O required to hook up to Discord, and we
        // only need to do this operation once per token, and we only permit one token so far so it
        // doesn't matter

        use backoff::{Error, ExponentialBackoff, Operation};
        let mut op = || discord::Discord::from_user_token(&discord_token).map_err(Error::Transient);
        let mut backoff = ExponentialBackoff::default();
        let dis = op.retry(&mut backoff).unwrap_or_else(|e| {
            println!("Unable to connect to Discord:\n{:#?}", e);
            std::process::exit(1);
        });

        let (mut connection, info) = {
            let mut op = || dis.connect().map_err(Error::Transient);
            let mut backoff = ExponentialBackoff::default();
            op.retry(&mut backoff).unwrap_or_else(|e| {
                println!("Unable to connect to Discord:\n{:#?}", e);
                std::process::exit(1);
            })
        };

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

        for c in discord.iter().cloned() {
            let sender = tui.sender();
            let info = info.clone();
            let dis = dis.clone();
            let discord_reciever = discord_reciever.clone();
            let conn_sender = conn_sender.clone();
            thread::spawn(move || {
                match DiscordConn::new(dis, info, discord_reciever, &c.name, sender.clone()) {
                    Ok(connection) => conn_sender.send(connection).unwrap(),
                    Err(err) => sender
                        .send(conn::Event::Error(format!("{:?}", err)))
                        .unwrap(),
                }
            });
        }
    }

    // When all the threads drop their senders, the below loop will terminate
    // But we must also drop ours, or it will block forever
    drop(conn_sender);

    while let Ok(connection) = conn_recv.recv() {
        tui.add_server(connection);
    }

    tui.run();
}
