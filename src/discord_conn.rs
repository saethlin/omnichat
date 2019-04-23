use crate::bimap::BiMap;
use crate::conn;
use crate::conn::{ConnEvent, DateTime, IString};
use log::error;
use std::borrow::Borrow;
use std::sync::mpsc::SyncSender;

use futures::{Future, Stream};

pub struct DiscordConn {
    token: String,
    guild_name: IString,
    guild_id: discord::Snowflake,
    channel_map: BiMap<IString, discord::Snowflake>,
    tui_sender: std::sync::mpsc::SyncSender<ConnEvent>,
    last_typing_message: chrono::DateTime<chrono::Utc>,
}

fn format_json(text: &[u8]) -> String {
    ::serde_json::from_slice::<::serde_json::Value>(text)
        .and_then(|v| ::serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| String::from_utf8(text.to_vec()).unwrap_or_default())
}

macro_rules! deserialize_or_log {
    ($response:expr, $type:ty) => {{
        if $response.status().is_success() {
            ::serde_json::from_slice::<$type>(&$response.bytes())
                .map_err(|e| error!("{}\n{:#?}", format_json(&$response.bytes()), e))
        } else {
            match ::serde_json::from_slice::<::slack::http::Error>(&$response.bytes()) {
                Ok(e) => {
                    error!("{:#?}", e);
                    Err(())
                }
                Err(e) => {
                    error!(
                        "{}\n{:#?}",
                        std::str::from_utf8($response.bytes()).unwrap(),
                        e
                    );
                    Err(())
                }
            }
        }
    }};
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
        let mut client = weeqwest::Client::new();

        let me_resp = client
            .get(&format!("{}/users/@me", ::discord::BASE_URL))
            .unwrap()
            .header("Authorization", token)
            .send()
            .wait()
            .map_err(|e| error!("{:#?}", e))?;
        let me = deserialize_or_log!(me_resp, ::discord::User)?;

        let guild_resp = client
            .get(&format!("{}{}", ::discord::BASE_URL, "/users/@me/guilds"))
            .unwrap()
            .header("Authorization", token)
            .send()
            .wait()
            .map_err(|e| error!("{:#?}", e))?;
        let guilds = deserialize_or_log!(guild_resp, Vec<::discord::Guild>)?;

        let guild = guilds.into_iter().find(|g| g.name == server).unwrap();
        let guild_name = IString::from(guild.name.as_str());
        let guild_id = guild.id.clone();

        let me_resp = client
            .get(&format!(
                "{}/guilds/{}/members/{}",
                ::discord::BASE_URL,
                guild.id,
                me.id
            ))
            .unwrap()
            .header("Authorization", token)
            .send()
            .wait()
            .map_err(|e| error!("{:#?}", e))?;

        let me = deserialize_or_log!(me_resp, discord::GuildMember)?;
        let mut my_roles = me.roles;

        let channels_resp = client
            .get(&format!(
                "{}/guilds/{}/channels",
                ::discord::BASE_URL,
                guild.id
            ))
            .unwrap()
            .header("Authorization", token)
            .send()
            .wait()
            .map_err(|e| error!("{:#?}", e))?;
        let channels = deserialize_or_log!(channels_resp, Vec<::discord::Channel>)?;

        let roles_resp = client
            .get(&format!(
                "{}/guilds/{}/roles",
                ::discord::BASE_URL,
                guild.id
            ))
            .unwrap()
            .header("Authorization", token)
            .send()
            .wait()
            .map_err(|e| error!("{:#?}", e))?;
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
            /*
            .filter(|c| {
                permissions_in(c, Some(&guild), &my_roles)
                    .contains(::discord::Permissions::READ_MESSAGES)
            })
            */
            .filter(|c| c.name.is_some())
            .collect();

        let mut channel_map: BiMap<IString, discord::Snowflake> = BiMap::new();
        for channel in &channels {
            let name = channel.name.clone().unwrap();
            let name = IString::from(name);
            channel_map.insert(name, channel.id.clone());
        }

        // This is how the TUI sends me events
        let (tx, events_from_tui) = std::sync::mpsc::sync_channel(100);

        let _ = sender.send(ConnEvent::ServerConnected(crate::tui::Server {
            channels: channels
                .iter()
                .map(|c| crate::tui::Channel {
                    messages: Vec::new(),
                    name: IString::from(c.name.clone().unwrap_or(String::from("NONAME"))),
                    read_at: crate::conn::DateTime::now(),
                    message_scroll_offset: 0,
                    message_buffer: String::new(),
                    channel_type: crate::conn::ChannelType::Normal,
                })
                .collect(),
            completer: None,
            name: guild_name.clone(),
            sender: tx,
            channel_scroll_offset: 0,
            current_channel: 0,
        }));

        let mut history_responses = Vec::new();
        for channel in channels.into_iter().filter(|c| c.name.is_some()) {
            let channel_name = IString::from(channel.name.unwrap().borrow());
            let token = token.to_string();
            let id = channel.id.clone();

            history_responses.push((
                channel_name,
                client
                    .get(&format!("{}/channels/{}/messages", ::discord::BASE_URL, id))
                    .unwrap()
                    .header("Authorization", token.as_str())
                    .send(),
            ));
        }

        for (channel, history_resp) in history_responses {
            let history_resp = history_resp.wait().unwrap();

            deserialize_or_log!(history_resp, Vec<::discord::Message>)
                .map(|history| {
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
                                channel: channel.clone(),
                                reactions: Vec::new(),
                            }
                        })
                        .collect();

                    let _ = sender.send(ConnEvent::HistoryLoaded {
                        server: guild_name.clone(),
                        channel,
                        read_at: DateTime::now(),
                        messages,
                    });
                })
                .map_err(|e| error!("{:#?}", e));
        }

        use std::sync::Arc;
        use std::sync::RwLock;
        let (tx, rx) = futures::sync::mpsc::channel(1);
        let connection = Arc::new(RwLock::new(DiscordConn {
            token: token.to_string(),
            guild_name: guild_name.clone(),
            guild_id: guild_id.clone(),
            channel_map: channel_map.clone(),
            tui_sender: sender.clone(),
            last_typing_message: chrono::offset::Utc::now(),
        }));

        let tconnection = connection.clone();
        std::thread::spawn(move || {
            for ev in events_from_tui.iter() {
                match ev {
                    conn::TuiEvent::SendMessage {
                        channel, contents, ..
                    } => tconnection
                        .read()
                        .unwrap()
                        .send_message(&channel, &contents),
                    conn::TuiEvent::SendTyping { channel, .. } => {
                        tconnection.write().unwrap().send_typing(&channel)
                    }
                    _ => error!("unsupported event {:?}", ev),
                }
            }
        });

        // Spin off a thread that will feed message events back to the TUI
        // websocket does not support the new tokio :(
        let i_token = token.to_string();
        std::thread::spawn(move || {
            use discord::gateway::GatewayEvent;
            use discord::gateway::GatewayMessage;

            let url_resp = client
                .get(&format!("{}{}", ::discord::BASE_URL, "/gateway"))
                .unwrap()
                .header("Authorization", &i_token)
                .send()
                .wait()
                .map_err(|e| error!("{:#?}", e))
                .unwrap();

            let resp = deserialize_or_log!(url_resp, ::discord::GatewayResponse).unwrap();

            //use websocket::result::WebSocketError;
            use futures::sink::Sink;
            use websocket::result::WebSocketError;
            use websocket::OwnedMessage::{Close, Ping, Pong, Text};
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
                                match msg {
                                    Ok(GatewayMessage {
                                        d:
                                            GatewayEvent::Hello {
                                                heartbeat_interval: interval,
                                                ..
                                            },
                                        ..
                                    }) => {
                                        let itx = tx.clone();
                                        std::thread::spawn(move || loop {
                                            let itx = itx.clone();
                                            itx.send(Text("{\"op\":1,\"d\":null}".to_string()))
                                                .wait()
                                                .map_err(|e| error!("{:#?}", e))
                                                .unwrap();
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                interval as u64,
                                            ));
                                        });
                                        let identify = serde_json::json! {{
                                                "op": 2,
                                                "d": {
                                                    "token": i_token,
                                                    "properties": {
                                                        "$os": ::std::env::consts::OS,
                                                        "$browser": "test-browser",
                                                        "$device": "test-device",
                                                    },
                                                    "large_threshold": 250,
                                                    "compress": false,
                                                    "v": 6,
                                                }
                                        }}
                                        .to_string();
                                        Some(Text(identify))
                                    }
                                    Ok(gateway_message) => {
                                        connection.read().unwrap().handle_websocket(gateway_message)
                                    }
                                    _ => None,
                                }
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

    fn handle_websocket(
        &self,
        message: discord::gateway::GatewayMessage,
    ) -> Option<websocket::OwnedMessage> {
        use discord::gateway::*;
        match message {
            GatewayMessage {
                d:
                    GatewayEvent::MessageCreate {
                        content,
                        author: Author { username, .. },
                        channel_id,
                        guild_id,
                        ..
                    },
                ..
            } => {
                if self.guild_id == guild_id {
                    self.channel_map.get_left(&channel_id).map(|channel| {
                        self.tui_sender
                            .send(ConnEvent::Message(conn::Message {
                                server: self.guild_name.clone(),
                                channel: channel.clone(),
                                contents: content,
                                reactions: Vec::new(),
                                sender: IString::from(username),
                                timestamp: DateTime::now(),
                            }))
                            .unwrap();
                    });
                }
            }
            _ => {}
        }
        None
    }

    fn send_message(&self, channel: &str, content: &str) {
        let channel = channel.to_string();
        let id: discord::Snowflake = self
            .channel_map
            .get_right(channel.as_str())
            .unwrap()
            .clone();
        let token = self.token.clone();
        let body = serde_json::json! {{
            "content": content,
            "tts": false,
        }}
        .to_string();
        let sender = self.tui_sender.clone();
        let guild_name = self.guild_name.clone();

        std::thread::spawn(move || {
            let request =
                weeqwest::Request::post(&format!("{}/channels/{}/messages", discord::BASE_URL, id))
                    .unwrap()
                    .header("Authorization", &token)
                    .json(body);
            if let Ok(response) = weeqwest::send(&request) {
                if let Ok(ack) = serde_json::from_slice::<MessageAck>(response.bytes()) {
                    sender
                        .send(ConnEvent::Message(conn::Message {
                            server: guild_name.clone(),
                            channel: IString::from(channel),
                            contents: ack.content,
                            reactions: Vec::new(),
                            sender: IString::from(ack.author.username),
                            timestamp: DateTime::now(),
                        }))
                        .unwrap();
                }
            }
        });
    }

    fn send_typing(&mut self, channel: &str) {
        let now = chrono::Utc::now();
        if (now - self.last_typing_message) < chrono::Duration::seconds(3) {
            return;
        } else {
            self.last_typing_message = now;
        }
        let id: discord::Snowflake = self.channel_map.get_right(channel).unwrap().clone();
        let token = self.token.clone();
        std::thread::spawn(move || {
            let req =
                weeqwest::Request::post(&format!("{}/channels/{}/typing", discord::BASE_URL, id))
                    .unwrap()
                    .header("Content-Length", "0")
                    .header("Authorization", &token);
            let _ = weeqwest::send(&req).map_err(|e| error!("{:#?}", e));
        });
    }
}

#[derive(serde::Deserialize)]
struct MessageAck {
    //timestamp: String,
    //id: discord::Snowflake,
    author: discord::gateway::Author,
    content: String,
}
