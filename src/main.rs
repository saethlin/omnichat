extern crate chrono;
extern crate dirs;
extern crate discord;
extern crate futures;
extern crate inlinable_string;
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
extern crate openssl_probe;
extern crate regex;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde_urlencoded;
extern crate signal_hook;
extern crate slack;
extern crate termion;
extern crate textwrap;
extern crate tokio_core;
extern crate toml;
extern crate websocket;

#[macro_use]
mod conn;
mod bimap;
mod chan_message;
mod cursor_vec;
mod discord_conn;
mod logger;
mod slack_conn;
mod tui;

#[derive(Debug, Deserialize, Clone)]
struct SlackConfig {
    token: String,
}

#[derive(Debug, Deserialize, Clone)]
struct DiscordConfig {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    discord_token: Option<String>,
    slack: Option<Vec<SlackConfig>>,
    discord: Option<Vec<DiscordConfig>>,
}

fn main() {
    use discord_conn::DiscordConn;
    use slack_conn::SlackConn;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;
    use std::thread;

    openssl_probe::init_ssl_cert_env_vars();

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

    let tui = tui::Tui::new();

    // Init the global logger
    log::set_boxed_logger(Box::new(logger::Logger::new(tui.sender())))
        .expect("Unable to create global logger");
    log::set_max_level(log::LevelFilter::Warn);

    // Start all the slack connections first, because we can't do the Discord stuff fully async
    if let Some(slack) = config.slack {
        for c in slack {
            let sender = tui.sender();
            thread::spawn(move || {
                if let Err(err) = SlackConn::create_on(&c.token, sender.clone()) {
                    error!("Failed to create slack connection: {}\n{:#?}", err, c);
                }
            });
        }
    }

    if let (Some(discord_token), Some(discord)) = (config.discord_token, config.discord) {
        for d in discord {
            let sender = tui.sender();
            let token = discord_token.clone();
            thread::spawn(move || {
                let _ = DiscordConn::create_on(&token, sender.clone(), &d.name);
            });
        }
    }

    tui.run();
}
