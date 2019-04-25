use serde::{Deserialize, Serialize};

use crate::{Snowflake, User};

// Things we send to Discord
#[derive(Serialize)]
pub enum GatewayCommand {
    Identify {
        token: String,
        properties: Properties,
        compress: Option<bool>,
        large_threshold: Option<u64>,
        shard: Option<[u64; 2]>,
        presence: Option<Presence>,
    },
    Resume {
        token: String,
        session_id: String,
        seq: u64,
    },
    Heartbeat,
    RequestGuildMembers,
    UpdateVoiceState,
    UpdateStatus,
}

// Things we get from Discord
#[derive(Debug)]
pub struct GatewayEvent {
    pub s: Option<u64>,
    pub d: Option<Event>,
    pub op: u64,
}

// Values of the d field in a GatewayEvent
#[derive(Debug)]
pub enum Event {
    Hello(Hello),
    MessageCreate(Message),
    MessageDelete(MessageDelete),
    MessageUpdate(Message),
    MessageReactionAdd(MessageReactionAdd),
    PresenceUpdate(PresenceUpdate),
    SessionsReplace(Vec<SessionsReplace>),
    MessageAck(MessageAck),
}

#[derive(Debug, Deserialize)]
pub struct MessageAck {
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Hello {
    pub _trace: Vec<String>,
    pub heartbeat_interval: u64,
}

// TODO: I've omitted a lot of fields from here for now
#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    pub guild_id: Option<Snowflake>,
    pub author: Option<User>,
    //pub member: Option<GuildMember>,
    pub content: Option<String>,
    pub timestamp: Option<String>,
    pub edited_timestamp: Option<String>,
    pub tts: Option<bool>,
    pub mention_everyone: Option<bool>,
}
#[derive(Debug, Deserialize)]
pub struct MessageDelete {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    pub guild_id: Option<Snowflake>,
}

#[derive(Debug, Deserialize)]
pub struct MessageReactionAdd {
    pub user_id: Snowflake,
    pub channel_id: Snowflake,
    pub message_id: Snowflake,
    pub guild_id: Option<Snowflake>,
    pub emoji: crate::Emoji,
}

#[derive(Debug, Deserialize)]
pub struct PresenceUpdate {
    pub user: PartialUser,
    pub roles: Option<Vec<Snowflake>>,
    pub game: Option<Activity>,
    pub guild_id: Option<Snowflake>,
    pub status: String, // this looks more like an enum
    pub activities: Vec<Activity>,
    pub client_status: ClientStatus,
}

// TODO No documentation at all?
#[derive(Debug, Deserialize)]
pub struct SessionsReplace {}

#[derive(Debug, Deserialize)]
pub struct ClientInfo {
    client: String,
    os: String,
    version: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartialUser {
    pub id: Snowflake,
    pub username: Option<String>,
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub bot: Option<bool>,
    pub mfa_enabled: Option<bool>,
    pub locale: Option<String>,
    pub verified: Option<bool>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub flags: Option<u64>,
    pub premium_type: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Activity {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: u64,
    pub url: Option<String>,
    pub timestamps: Option<Timestamps>,
    pub application_id: Option<Snowflake>,
    pub details: Option<String>,
    pub state: Option<String>,
    pub party: Option<Party>,
    pub assets: Option<Assets>,
    pub secrets: Option<Secrets>,
    pub instance: Option<bool>,
    pub flags: Option<u8>, // Actually an ActivityFlags bitflag object
}

#[derive(Debug, Deserialize)]
pub struct Party {
    pub id: Option<String>,
    pub size: Option<[u64; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct Assets {
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Secrets {
    pub join: Option<String>,
    pub spectate: Option<String>,
    #[serde(rename = "match")]
    pub mtch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClientStatus {
    pub desktop: Option<String>,
    pub mobile: Option<String>,
    pub web: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Timestamps {
    start: Option<u64>,
    end: Option<u64>,
}

use serde::de::{Deserializer, Visitor};
impl<'de> Deserialize<'de> for GatewayEvent {
    fn deserialize<D>(deserializer: D) -> Result<GatewayEvent, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Op,
            D,
            S,
            T,
        }

        struct GatewayVisitor;

        impl<'de> Visitor<'de> for GatewayVisitor {
            type Value = GatewayEvent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<M>(self, mut map: M) -> Result<GatewayEvent, M::Error>
            where
                M: ::serde::de::MapAccess<'de>,
            {
                let mut op = None;
                let mut d: Option<serde_json::Value> = None;
                let mut s = None;
                let mut t = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Op => op = Some(map.next_value()?),
                        Field::D => d = Some(map.next_value()?),
                        Field::S => s = Some(map.next_value()?),
                        Field::T => t = Some(map.next_value()?),
                    }
                }

                let op = op.ok_or_else(|| serde::de::Error::missing_field("op"))?;
                let t = t.ok_or_else(|| serde::de::Error::missing_field("t"))?;
                let d: Option<Event> = match op {
                    0 => match (t, d) {
                        (Some("MESSAGE_CREATE"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::MessageCreate(inner))
                        }
                        (Some("MESSAGE_REACTION_ADD"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::MessageReactionAdd(inner))
                        }
                        (Some("MESSAGE_UPDATE"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::MessageUpdate(inner))
                        }
                        (Some("MESSAGE_DELETE"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::MessageDelete(inner))
                        }
                        (Some("MESSAGE_ACK"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::MessageAck(inner))
                        }
                        (Some("PRESENCE_UPDATE"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::PresenceUpdate(inner))
                        }
                        (Some("SESSIONS_REPLACE"), Some(d)) => {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::SessionsReplace(inner))
                        }
                        _ => {
                            return Err(serde::de::Error::custom(format!(
                                "Unable to deserialize message with type {:?}",
                                t
                            )))
                        }
                    },
                    10 => {
                        if let Some(d) = d {
                            let inner = serde_json::from_value(d)
                                .map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::Hello(inner))
                        } else {
                            None
                        }
                    }
                    11 => None,
                    /*
                    1 => {
                        if let Some(d) = d {
                            let inner = serde_json::from_value::<Heartbeat>(d).map_err(|e| serde::de::Error::custom(e))?;
                            Some(Event::Heartbeat(inner))
                        } else {
                            None
                        }
                    }
                    */
                    _ => {
                        return Err(serde::de::Error::custom(format!(
                            "Unrecognized opcode {}",
                            op
                        )))
                    }
                };
                let s = s.ok_or_else(|| serde::de::Error::missing_field("s"))?;
                Ok(GatewayEvent { d, s, op })
            }
        }

        const FIELDS: &'static [&'static str] = &["op", "d", "s", "t"];
        deserializer.deserialize_struct("GatewayEvent", FIELDS, GatewayVisitor)
    }
}

#[derive(Serialize)]
pub struct GatewayIdentify {
    pub token: String,
    pub properties: Properties,
    pub compress: bool,
    pub large_threshold: u64,
    pub shard: Option<[u64; 2]>,
    pub presence: Presence,
}

#[derive(Serialize)]
pub struct Properties {
    #[serde(rename = "$os")]
    pub os: String,
    #[serde(rename = "$browser")]
    pub browser: String,
    #[serde(rename = "$device")]
    pub device: String,
}

#[derive(Serialize)]
pub struct Presence {
    pub game: Game,
    pub status: String,
    pub since: u64,
    pub afk: bool,
}

#[derive(Serialize)]
pub struct Game {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: u64,
}
