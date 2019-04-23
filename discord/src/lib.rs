use std::borrow::Cow;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

pub mod gateway;

pub const BASE_URL: &'static str = "https://discordapp.com/api";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayResponse {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    // for rate limits
    pub global: Option<bool>,
    // in ms
    pub retry_after: Option<u64>,
    // max error code is 90001
    pub code: Option<u32>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub flags: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuildMember {
    pub user: User,
    pub nick: Option<String>,
    pub roles: Vec<Snowflake>,
    pub joined_at: String,
    pub deaf: bool,
    pub mute: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub color: u64,
    pub hoist: bool,
    pub id: Snowflake,
    pub managed: bool,
    pub mentionable: bool,
    pub name: String,
    pub permissions: Permissions,
    pub position: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Snowflake(String); // Actually a u64

impl ::std::fmt::Display for Snowflake {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guild {
    pub id: Snowflake,
    pub name: String,
    pub icon: Option<Snowflake>,
    pub owner: bool,
    pub permissions: Permissions, // Oh no they've encoded them strangely
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug)]
pub enum ChannelType {
    GuildText,
    Dm,
    GuildVoice,
    GroupDm,
    GuildCategory,
}

struct ChannelTypeVisitor;

impl<'de> ::serde::de::Visitor<'de> for ChannelTypeVisitor {
    type Value = ChannelType;
    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.write_str("an integer")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: ::serde::de::Error,
    {
        match value {
            0 => Ok(ChannelType::GuildText),
            1 => Ok(ChannelType::Dm),
            2 => Ok(ChannelType::GuildVoice),
            3 => Ok(ChannelType::GroupDm),
            4 => Ok(ChannelType::GuildCategory),
            _ => Err(::serde::de::Error::custom(format!(
                "invalid channel type {}",
                value
            ))),
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for ChannelType {
    fn deserialize<D>(deserializer: D) -> Result<ChannelType, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ChannelTypeVisitor)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overwrite {
    pub id: Snowflake,
    #[serde(rename = "type")]
    pub ty: OverwriteType,
    pub allow: Permissions,
    pub deny: Permissions,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverwriteType {
    Role,
    Member,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Message<'a> {
    pub id: Snowflake,
    pub channel_id: Snowflake,
    pub guild_id: Option<Snowflake>,
    // There's an author field but it's an untagged enum
    pub author: User,
    //pub member: Option<PartialGuild>,
    #[serde(borrow)]
    pub content: Cow<'a, str>,
    pub timestamp: &'a str,
    pub edited_timestamp: Option<&'a str>,
    pub tts: bool,
    pub mention_everyone: bool,
    pub mentions: Vec<User>,
    pub mention_roles: Vec<Snowflake>,
    //pub attachments: Vec<Attachment>,
    //pub embeds: Vec<Embed>,
    //pub reactions: Option<Vec<Reaction>>,
    pub nonce: Option<Snowflake>,
    pub pinned: bool,
    pub webhook_id: Option<Snowflake>,
    #[serde(rename = "type")]
    pub ty: u64,
    //pub activity: Option<MessageActivity>,
    //pub application: Option<MessageApplication>,
}

// All this was taken from spacemaniac/discord-rs
macro_rules! serial_single_field {
    ($typ:ident as $field:ident: $inner:path) => {
        impl ::serde::Serialize for $typ {
            fn serialize<S: ::serde::ser::Serializer>(
                &self,
                s: S,
            ) -> ::std::result::Result<S::Ok, S::Error> {
                self.$field.serialize(s)
            }
        }

        impl<'d> ::serde::Deserialize<'d> for $typ {
            fn deserialize<D: ::serde::de::Deserializer<'d>>(
                d: D,
            ) -> ::std::result::Result<$typ, D::Error> {
                <$inner as ::serde::de::Deserialize>::deserialize(d).map(|v| $typ { $field: v })
            }
        }
    };
}

serial_single_field!(Permissions as bits: u64);

bitflags! {
    /// Set of permissions assignable to a Role or PermissionOverwrite
    pub struct Permissions: u64 {
        const CREATE_INVITE = 1;
        const KICK_MEMBERS = 1 << 1;
        const BAN_MEMBERS = 1 << 2;
        /// Grant all permissions, bypassing channel-specific permissions
        const ADMINISTRATOR = 1 << 3;
        /// Modify roles below their own
        const MANAGE_ROLES = 1 << 28;
        /// Create channels or edit existing ones
        const MANAGE_CHANNELS = 1 << 4;
        /// Change the server's name or move regions
        const MANAGE_SERVER = 1 << 5;
        /// Change their own nickname
        const CHANGE_NICKNAMES = 1 << 26;
        /// Change the nickname of other users
        const MANAGE_NICKNAMES = 1 << 27;
        /// Manage the emojis in a a server.
        const MANAGE_EMOJIS = 1 << 30;
        /// Manage channel webhooks
        const MANAGE_WEBHOOKS = 1 << 29;

        const READ_MESSAGES = 1 << 10;
        const SEND_MESSAGES = 1 << 11;
        /// Send text-to-speech messages to those focused on the channel
        const SEND_TTS_MESSAGES = 1 << 12;
        /// Delete messages by other users
        const MANAGE_MESSAGES = 1 << 13;
        const EMBED_LINKS = 1 << 14;
        const ATTACH_FILES = 1 << 15;
        const READ_HISTORY = 1 << 16;
        /// Trigger a push notification for an entire channel with "@everyone"
        const MENTION_EVERYONE = 1 << 17;
        /// Use emojis from other servers
        const EXTERNAL_EMOJIS = 1 << 18;
        /// Add emoji reactions to messages
        const ADD_REACTIONS = 1 << 6;

        const VOICE_CONNECT = 1 << 20;
        const VOICE_SPEAK = 1 << 21;
        const VOICE_MUTE_MEMBERS = 1 << 22;
        const VOICE_DEAFEN_MEMBERS = 1 << 23;
        /// Move users out of this channel into another
        const VOICE_MOVE_MEMBERS = 1 << 24;
        /// When denied, members must use push-to-talk
        const VOICE_USE_VAD = 1 << 25;
    }
}
