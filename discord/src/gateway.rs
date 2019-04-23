use serde::{Deserialize, Serialize};

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
#[derive(Debug, Deserialize)]
pub struct GatewayMessage {
    pub op: u64,
    #[serde(rename = "d")]
    pub d: GatewayEvent,
    #[serde(rename = "s")]
    pub s: Option<u64>,
    #[serde(rename = "t")]
    pub t: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum GatewayEvent {
    Hello {
        heartbeat_interval: u32,
        _trace: Vec<String>,
    },
    MessageCreate {
        tts: bool,
        timestamp: String,
        pinned: bool,
        nonce: String,
        // mentions stuff
        content: String,
        author: Author,
        channel_id: crate::Snowflake,
        guild_id: crate::Snowflake,
    },
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub username: String,
    pub id: String,
    pub discriminator: String,
    pub avatar: String,
}

/*
use serde::de::{Deserialize, Deserializer, Visitor};
impl<'de> Deserialize<'de> for GatewayMessage {
    fn deserialize<D>(deserializer: D) -> Result<GatewayMessage, D::Error>
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
            type Value = GatewayMessage;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<M>(self, mut map: M) -> Result<GatewayMessage, M::Error>
            where
                M: ::serde::de::MapAccess<'de>,
            {
                let mut op = None;
                let mut d = None;
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
                let d = d.ok_or_else(|| serde::de::Error::missing_field("d"))?;
                let s = s.ok_or_else(|| serde::de::Error::missing_field("s"))?;
                let t = t.ok_or_else(|| serde::de::Error::missing_field("t"))?;
                Ok(GatewayMessage { op, d, s, t })
            }
        }

        const FIELDS: &'static [&'static str] = &["op", "d", "s", "t"];
        deserializer.deserialize_struct("GatewayMessage", FIELDS, GatewayVisitor)
    }
}
*/

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
