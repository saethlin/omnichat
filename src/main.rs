#![recursion_limit = "1024"]

use serde::Deserialize;

mod bimap;
mod chan_message;
mod conn;
mod cursor_vec;
//mod discord_conn;
mod logger;
mod slack_conn;
mod tui;

#[derive(Deserialize)]
struct SlackConfig {
    token: String,
}

/*
#[derive(Deserialize)]
struct DiscordConfig {
    name: String,
}
*/

#[derive(Deserialize)]
struct Config {
    slack: Option<Vec<SlackConfig>>,
    //discord_token: Option<String>,
    //discord: Option<Vec<DiscordConfig>>,
}

#[tokio::main(core_threads = 4)]
async fn main() {
    //use crate::discord_conn::DiscordConn;
    use crate::slack_conn::SlackConn;
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;

    std::env::set_var("RUST_BACKTRACE", "1");

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
            let token = c.token.clone();
            tokio::spawn(async move { SlackConn::create_on(&token, sender).await });
        }
    }

    /*
    if let (Some(discord_token), Some(discord)) = (config.discord_token, config.discord) {
        for d in discord {
            let sender = tui.sender();
            let token = discord_token.clone();
            thread::spawn(move || {
                let _ = DiscordConn::create_on(&token, sender.clone(), &d.name);
            });
        }
    }
    */

    tui.run().await
}

use regex_automata::{DenseDFA, DFA};

trait DFAExtension: DFA {
    fn get_first<'a>(&self, text: &'a [u8]) -> Option<&'a [u8]>;
}

impl DFAExtension for DenseDFA<&'static [u16], u16> {
    fn get_first<'a>(&self, text: &'a [u8]) -> Option<&'a [u8]> {
        let end = self.find(text)?;
        for i in 1..=end {
            let start = end - i;
            let slice = &text[start..end];
            if self.is_match(slice) {
                return Some(text.get(start..end)?);
            }
        }
        None
    }
}
