use conn::{Conn, Event, IString};
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
                error!("{:#?}\n{}", e, pretty)
            })
        } else {
            match ::serde_json::from_str::<::discord::Error>(&text) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!("{:#?}\n{}", e, text);
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

        let guild = guilds.iter().find(|g| g.name == server).unwrap();

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

        let channel_names: Vec<IString> = channels
            .iter()
            .filter_map(|c| c.name.as_ref())
            .map(|name| name.as_str().into())
            .collect();

        let _ = sender.send(Event::Connected(Box::new(Self {
            name: guild.name.as_str().into(),
            channels: channel_names,
        })));

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
