use crate::conn::{ConnEvent, DateTime, IString};
use log::error;
use std::borrow::Borrow;
use std::sync::mpsc::SyncSender;

use futures::{Future, Stream};

::lazy_static::lazy_static! {
    pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
}

#[allow(dead_code)]
pub struct DiscordConn {
    name: IString,
    channels: Vec<IString>,
}

macro_rules! deserialize_or_log {
    ($response:expr, $type:ty) => {{
        if $response.status.is_success() {
            ::serde_json::from_str::<$type>(&$response.text).map_err(|e| {
                let pretty = ::serde_json::from_str::<::serde_json::Value>(&$response.text)
                    .and_then(|v| ::serde_json::to_string_pretty(&v))
                    .unwrap_or_else(|_| String::from("Cannot pretty-print response"));
                error!("{}\n{:#?}", pretty, e)
            })
        } else {
            match ::serde_json::from_str::<::discord::Error>(&$response.text) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!("{}\n{:#?}", $response.text, e);
                    Err(())
                }
            }
        }
    }};
}

struct Response {
    text: String,
    status: ::reqwest::StatusCode,
}

pub fn permissions_in(
    chan: &::discord::Channel,
    guild: Option<&::discord::Guild>,
    roles: &[::discord::Snowflake],
) -> ::discord::Permissions {
    let mut perms = match guild {
        Some(guild) => {
            let perms = guild.permissions;
            if (perms.contains(::discord::Permissions::ADMINISTRATOR)) || guild.owner {
                ::discord::Permissions::all()
            } else {
                perms
            }
        }
        None => ::discord::Permissions::all(),
    };

    // Role overrides
    for overwrite in chan
        .permission_overwrites
        .iter()
        .filter(|owrite| roles.iter().any(|r| *r == owrite.id))
    {
        perms.insert(overwrite.allow);
        perms.remove(overwrite.deny);
    }

    perms
}

impl DiscordConn {
    pub fn create_on(token: &str, sender: SyncSender<ConnEvent>, server: &str) -> Result<(), ()> {
        let me_resp = CLIENT
            .get(&format!("{}/users/@me", ::discord::BASE_URL))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .map(|mut r| Response {
                text: r.text().unwrap(),
                status: r.status(),
            })?;
        let me = deserialize_or_log!(me_resp, ::discord::User)?;

        let guild_resp = CLIENT
            .get(&format!("{}{}", ::discord::BASE_URL, "/users/@me/guilds"))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .map(|mut r| Response {
                text: r.text().unwrap(),
                status: r.status(),
            })?;
        let guilds = deserialize_or_log!(guild_resp, Vec<::discord::Guild>)?;

        let guild = guilds.into_iter().find(|g| g.name == server).unwrap();
        let guild_name = IString::from(guild.name.as_str());

        let me_resp = CLIENT
            .get(&format!(
                "{}/guilds/{}/members/{}",
                ::discord::BASE_URL,
                guild.id,
                me.id
            ))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .map(|mut r| Response {
                text: r.text().unwrap(),
                status: r.status(),
            })?;

        let me = deserialize_or_log!(me_resp, discord::GuildMember)?;
        let mut my_roles = me.roles;

        let channels_resp = CLIENT
            .get(&format!(
                "{}/guilds/{}/channels",
                ::discord::BASE_URL,
                guild.id
            ))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .map(|mut r| Response {
                text: r.text().unwrap(),
                status: r.status(),
            })?;
        let channels = deserialize_or_log!(channels_resp, Vec<::discord::Channel>)?;

        let roles_resp = CLIENT
            .get(&format!(
                "{}/guilds/{}/roles",
                ::discord::BASE_URL,
                guild.id
            ))
            .header("Authorization", token)
            .send()
            .map_err(|e| error!("{:#?}", e))
            .map(|mut r| Response {
                text: r.text().unwrap(),
                status: r.status(),
            })?;
        let roles = deserialize_or_log!(roles_resp, Vec<::discord::Role>)?;
        let everyone_role = roles
            .into_iter()
            .find(|r| r.name == "@everyone")
            .unwrap()
            .id;

        my_roles.push(everyone_role);

        let channels: Vec<_> = channels
            .into_iter()
            .filter(|c| c.ty == 0)
            .filter(|c| {
                permissions_in(c, Some(&guild), &my_roles)
                    .contains(::discord::Permissions::READ_MESSAGES)
            })
            .collect();

        let channel_names: Vec<IString> = channels
            .iter()
            .filter_map(|c| c.name.as_ref())
            .map(|name| IString::from(name.as_str()))
            .collect();

        // This is how the TUI sends me events
        let (tx, _rx) = std::sync::mpsc::sync_channel(100);

        let _ = sender.send(ConnEvent::ServerConnected {
            channels: channel_names,
            completer: None,
            name: guild_name.clone(),
            sender: tx,
        });

        for channel in channels.into_iter().filter(|c| c.name.is_some()) {
            let channel_name = IString::from(channel.name.unwrap().borrow());
            let token = token.to_string();
            let sender = sender.clone();
            let guild_name = guild_name.clone();
            let id = channel.id.clone();

            ::std::thread::spawn(move || {
                if let Ok(history_resp) = CLIENT
                    .get(&format!("{}/channels/{}/messages", ::discord::BASE_URL, id))
                    .header("Authorization", token.as_str())
                    .send()
                    .map_err(|e| error!("{:#?}", e))
                    .map(|mut r| Response {
                        text: r.text().unwrap(),
                        status: r.status(),
                    })
                {
                    let history =
                        deserialize_or_log!(history_resp, Vec<::discord::Message>).unwrap();
                    let messages = history
                        .into_iter()
                        .map(|message| {
                            let timestamp =
                                ::chrono::DateTime::parse_from_rfc3339(&message.timestamp)
                                    .map(|d| d.with_timezone(&::chrono::Utc))
                                    .map(|d| d.into())
                                    .unwrap_or_else(|_| DateTime::now());

                            crate::conn::Message {
                                sender: IString::from(message.author.username),
                                server: guild_name.clone(),
                                timestamp,
                                contents: String::from(message.content),
                                channel: channel_name.clone(),
                                reactions: Vec::new(),
                            }
                        })
                        .collect();

                    let _ = sender.send(ConnEvent::HistoryLoaded {
                        server: guild_name,
                        channel: channel_name.clone(),
                        read_at: DateTime::now(),
                        messages,
                    });
                }
            });
        }

        // Spin off a thread that will feed message events back to the TUI
        // websocket does not support the new tokio :(
        let i_token = token.to_string();
        std::thread::spawn(move || {
            use discord::gateway::GatewayEvent;
            use discord::gateway::GatewayMessage;

            let url_resp = CLIENT
                .get(&format!("{}{}", ::discord::BASE_URL, "/gateway"))
                .header("Authorization", i_token)
                .send()
                .map_err(|e| error!("{:#?}", e))
                .map(|mut r| Response {
                    text: r.text().unwrap(),
                    status: r.status(),
                })
                .unwrap();

            let resp = deserialize_or_log!(url_resp, ::discord::GatewayResponse).unwrap();

            //use websocket::result::WebSocketError;
            use futures::sink::Sink;
            use websocket::result::WebSocketError;
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
            let (tx, rx) = futures::sync::mpsc::channel(1);
            let runner = ::websocket::ClientBuilder::new(&resp.url)
                .unwrap()
                .async_connect_secure(None)
                .and_then(|(duplex, _)| {
                    let (sink, stream) = duplex.split();
                    stream
                        // Maps a message to maybe a response
                        .filter_map(|message| match message {
                            Close(_) => {
                                error!("websocket closed");
                                None
                            }
                            Ping(m) => Some(Pong(m)),
                            Text(text) => {
                                let msg = ::serde_json::from_str::<GatewayMessage>(&text);
                                if let Ok(GatewayMessage {
                                    d:
                                        GatewayEvent::Hello {
                                            heartbeat_interval: interval,
                                            ..
                                        },
                                    ..
                                }) = msg
                                {
                                    let tx = tx.clone();
                                    std::thread::spawn(move || loop {
                                        let tx = tx.clone();
                                        tx.send(Text("{\"op\": 1}".to_string()))
                                            .wait()
                                            .map_err(|e| error!("{:#?}", e));
                                        std::thread::sleep_ms(interval as u32);
                                    });
                                }
                                error!("{}", text);
                                None
                            }
                            _ => None,
                        })
                        .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
                        .forward(sink)
                });
            ::tokio::runtime::current_thread::block_on_all(runner).unwrap();
        });

        Ok(())
    }
}
