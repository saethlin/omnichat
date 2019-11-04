use crate::id::*;
use crate::timestamp::Timestamp;

#[derive(Serialize, new)]
pub struct MarkRequest {
    /// Private channel to set reading cursor in.
    pub channel: GroupId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Retrieve information about a group
///
/// Wraps https://api.slack.com/methods/groups.info

#[derive(Serialize, new)]
pub struct InfoRequest {
    /// Group ID to learn more about
    pub channel: crate::GroupId,
    /// Set this to true to receive the locale for this conversation. Defaults to false
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Deserialize)]
pub struct InfoResponse {
    pub ok: bool,
    pub group: Info,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    pub created: Timestamp,
    pub creator: UserId,
    pub id: GroupId,
    pub is_archived: bool,
    pub is_group: bool,
    pub is_mpim: bool,
    pub is_open: bool,
    pub last_read: Option<Timestamp>,
    pub latest: crate::http::conversations::LatestInfo,
    pub members: Vec<UserId>,
    pub name: String,
    pub name_normalized: String,
    pub previous_names: Option<Vec<String>>,
    pub purpose: crate::http::conversations::ConversationPurpose,
    pub topic: crate::http::conversations::ConversationTopic,
    pub unread_count: u32,
    pub unread_count_display: u32,
}
