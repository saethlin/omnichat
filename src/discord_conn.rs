use conn::{Conn, DateTime, Event, IString};
use std::sync::mpsc::SyncSender;

lazy_static! {
    pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
}

pub struct DiscordConn {
    name: IString,
    channels: Vec<IString>,
}

macro_rules! deserialize_or_log {
    ($response:expr, $type:ty) => {{
        let text = $response.text().unwrap();
        if $response.status().is_success() {
            ::serde_json::from_str::<$type>(&text).map_err(|e| {
                let pretty = ::serde_json::from_str::<::serde_json::Value>(&text)
                    .and_then(|v| ::serde_json::to_string_pretty(&v))
                    .unwrap_or_else(|_| String::from("Cannot pretty-print response"));
                error!("{}\n{:#?}", pretty, e)
            })
        } else {
            match ::serde_json::from_str::<::discord::Error>(&text) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!("{}\n{:#?}", text, e);
                    Err(())
                }
            }
        }
    }};
}

impl DiscordConn {
    pub fn create_on(token: &str, sender: SyncSender<Event>, server: &str) -> Result<(), ()> {
        let guilds = CLIENT
            .get(&format!("{}{}", ::discord::BASE_URL, "/users/@me/guilds"))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .and_then(|mut r| deserialize_or_log!(r, Vec<::discord::Guild>))?;

        let guild = guilds.into_iter().find(|g| g.name == server).unwrap();
        let guild_name: IString = guild.name.as_str().into();

        let channels = CLIENT
            .get(&format!(
                "{}/guilds/{}/channels",
                ::discord::BASE_URL,
                guild.id
            ))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .and_then(|mut r| deserialize_or_log!(r, Vec<::discord::Channel>))?;

        let channels: Vec<_> = channels.into_iter().filter(|c| c.ty == 0).collect();

        let channel_names: Vec<IString> = channels
            .iter()
            .filter_map(|c| c.name.as_ref())
            .map(|name| name.as_str().into())
            .collect();

        let _ = sender.send(Event::Connected(Box::new(Self {
            name: guild_name.clone(),
            channels: channel_names,
        })));

        for channel in channels.into_iter().filter(|c| c.name.is_some()) {
            let channel_name: IString = channel.name.as_ref().unwrap().as_str().into();
            let token = token.to_string();
            let sender = sender.clone();
            let guild_name = guild_name.clone();

            ::std::thread::spawn(move || {
                if let Ok(history) = CLIENT
                    .get(&format!(
                        "{}/channels/{}/messages",
                        ::discord::BASE_URL,
                        channel.id
                    ))
                    .header("Authorization", token.as_str())
                    .send()
                    .map_err(|e| error!("{:#?}", e))
                    .and_then(|mut r| deserialize_or_log!(r, Vec<::discord::Message>))
                {
                    for message in history {
                        let timestamp = ::chrono::DateTime::parse_from_rfc3339(&message.timestamp)
                            .unwrap()
                            .with_timezone(&::chrono::Utc);
                        let _ = sender.send(Event::Message(::conn::Message {
                            sender: message.author.username.as_str().into(),
                            server: guild_name.clone(),
                            timestamp: timestamp.into(),
                            contents: message.content,
                            channel: channel_name.clone(),
                            is_mention: false,
                            reactions: Vec::new(),
                        }));
                    }

                    let _ = sender.send(Event::HistoryLoaded {
                        server: guild_name,
                        channel: channel_name.clone(),
                        read_at: DateTime::now(),
                    });
                }
            });
        }

        Ok(())
    }
}

impl Conn for DiscordConn {
    fn name(&self) -> &str {
        &self.name
    }

    fn channels(&self) -> &[IString] {
        &self.channels
    }
}
