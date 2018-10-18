extern crate serde;
#[macro_use]
extern crate serde_derive;

pub const BASE_URL: &'static str = "https://discordapp.com/api";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error<'a> {
    pub code: u64,
    pub message: &'a str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum GatewayMessage {
    Hello {
        // Opcode 10
        heartbeat_interval: u64,
        _trace: Vec<String>,
    },
    Heartbeat,   // Opcode 1
    HearbeatAck, // Opcode 11
    Identify {
        // Opcode 2
        token: String,
        properties: Properties,
        compress: bool,
        large_threshold: u64,
        shard: (u8, u8),
        //presence: Presence,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Properties {
    #[serde(rename = "$os")]
    os: String,
    #[serde(rename = "$browser")]
    browser: String,
    #[serde(rename = "$device")]
    device: String,
}

//{"id":1,"type":"message","channel":"C3QV41U6M","text":"test"}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub id: Snowflake,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<Snowflake>,
    pub bot: Option<bool>,
    pub mfa_enabled: Option<bool>,
    pub locale: Option<String>,
    pub verified: Option<bool>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub flags: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snowflake(String); // Actually a u64

impl ::std::fmt::Display for Snowflake {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guild {
    pub id: Snowflake,
    pub name: String,
    pub icon: Option<Snowflake>,
    pub owner: bool,
    pub permissions: u64, // Oh no they've encoded them strangely
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub ty: u8,
    pub guild_id: Option<Snowflake>,
    pub position: Option<u64>,
    pub permission_overwrites: Vec<Overwrite>,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub nsfw: Option<bool>,
    pub last_message_id: Option<Snowflake>,
    pub bitrate: Option<u64>,
    pub user_limit: Option<u64>,
    pub rate_limit_per_user: Option<u64>,
    pub recipients: Option<Vec<User>>,
    pub icon: Option<String>,
    pub owner_id: Option<Snowflake>,
    pub application_id: Option<Snowflake>,
    pub parent_id: Option<Snowflake>,
    pub last_pin_timestamp: Option<Timestamp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timestamp(String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GuildType {
    GUILD_TEXT,
    DM,
    GUILD_VOICE,
    GROUP_DM,
    GUILD_CATEGORY,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overwrite {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub ty: String,
    pub allow: u64,
    pub deny: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverwriteType {
    Role,
    Member,
}
