use crate::id::*;
use crate::timestamp::Timestamp;

#[derive(Serialize, new)]
pub struct MarkRequest {
    /// Channel to set reading cursor in.
    pub channel: ChannelId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Retrieve information about a channel
///
/// Wraps https://api.slack.com/methods/channels.info

#[derive(Serialize, new)]
pub struct InfoRequest {
    /// Channel ID to learn more about
    pub channel: ChannelId,
    /// Set this to true to receive the locale for this conversation. Defaults to false
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Deserialize)]
pub struct InfoResponse {
    pub ok: bool,
    pub channel: Info,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    pub created: Timestamp,
    pub creator: UserId,
    pub id: ChannelId,
    pub is_archived: bool,
    pub is_channel: bool,
    pub is_general: bool,
    pub is_member: bool,
    pub is_mpim: bool,
    pub is_org_shared: bool,
    pub is_private: bool,
    /// Present on the general channel for free plans, possibly all channels otherwise
    pub is_read_only: Option<bool>,
    pub is_shared: bool,
    /// Present if is_member is true
    pub last_read: Option<Timestamp>,
    pub latest: crate::http::conversations::LatestInfo,
    pub members: Vec<UserId>,
    pub name: String,
    pub name_normalized: String,
    pub previous_names: Vec<String>,
    pub purpose: crate::http::conversations::ConversationPurpose,
    pub topic: crate::http::conversations::ConversationTopic,
    pub unlinked: u32,
    pub unread_count: u32,
    pub unread_count_display: u32,
}
