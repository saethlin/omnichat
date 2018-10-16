//! Get info on your team's Slack channels, create or archive channels, invite users, set the topic and purpose, and mark a channel as read.

use rtm::{Channel, Cursor, Message, Paging};
use timestamp::Timestamp;

/// Archives a channel.
///
/// Wraps https://api.slack.com/methods/channels.archive
#[derive(Clone, Debug, Serialize, new)]
pub struct ArchiveRequest {
    /// Channel to archive
    pub channel: ::ChannelId,
}

/// Creates a channel.
///
/// Wraps https://api.slack.com/methods/channels.create
#[derive(Clone, Debug, Serialize, new)]
pub struct CreateRequest<'a> {
    /// Name of channel to create
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    ok: bool,
    pub channel: Channel,
}

/// Fetches history of messages and events from a channel.
///
/// Wraps https://api.slack.com/methods/channels.history

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Channel to fetch history for.
    pub channel: ::ChannelId,
    /// End of time range of messages to include in results.
    #[new(default)]
    pub latest: Option<Timestamp>,
    /// Start of time range of messages to include in results.
    #[new(default)]
    pub oldest: Option<Timestamp>,
    /// Include messages with latest or oldest timestamp in results.
    #[new(default)]
    pub inclusive: Option<bool>,
    /// Number of messages to return, between 1 and 1000.
    #[new(default)]
    pub count: Option<u32>,
    /// Include unread_count_display in the output?
    #[new(default)]
    pub unreads: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryResponse {
    ok: bool,
    pub has_more: Option<bool>,
    pub latest: Option<Timestamp>,
    pub messages: Vec<Message>,
    pub is_limited: Option<bool>,
}

/// Gets information about a channel.
///
/// Wraps https://api.slack.com/methods/channels.info

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Channel to get info on
    pub channel: ::ChannelId,
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub channel: Channel,
}

/// Invites a user to a channel.
///
/// Wraps https://api.slack.com/methods/channels.invite

#[derive(Clone, Debug, Serialize, new)]
pub struct InviteRequest {
    /// Channel to invite user to.
    pub channel: ::ChannelId,
    /// User to invite to channel.
    pub user: ::UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteResponse {
    ok: bool,
    pub channel: Channel,
}

/// Joins a channel, creating it if needed.
///
/// Wraps https://api.slack.com/methods/channels.join

#[derive(Clone, Debug, Serialize, new)]
pub struct JoinRequest<'a> {
    /// Name of channel to join
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinResponse {
    ok: bool,
    pub channel: Channel, //TODO: This contains different attributes depending on already_in_channel
    pub already_in_channel: Option<bool>,
}

/// Removes a user from a channel.
///
/// Wraps https://api.slack.com/methods/channels.kick

#[derive(Clone, Debug, Serialize, new)]
pub struct KickRequest {
    /// Channel to remove user from.
    pub channel: ::ChannelId,
    /// User to remove from channel.
    pub user: ::UserId,
}

/// Leaves a channel.
///
/// Wraps https://api.slack.com/methods/channels.leave

#[derive(Clone, Debug, Serialize, new)]
pub struct LeaveRequest {
    /// Channel to leave
    pub channel: ::ChannelId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaveResponse {
    ok: bool,
    not_in_channel: Option<bool>,
}

/// Lists all channels in a Slack team.
///
/// Wraps https://api.slack.com/methods/channels.list

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Exclude archived channels from the list
    #[new(default)]
    pub exclude_archived: Option<bool>,
    /// Exclude the members collection from each channel
    #[new(default)]
    pub exclude_members: Option<bool>,
    #[new(default)]
    pub cursor: Option<Cursor>,
    #[new(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub channels: Vec<Channel>,
    pub response_metadata: Option<Paging>,
}

/// Sets the read cursor in a channel.
///
/// Wraps https://api.slack.com/methods/channels.mark

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Channel to set reading cursor in.
    pub channel: ::ChannelId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Renames a channel.
///
/// Wraps https://api.slack.com/methods/channels.rename

#[derive(Clone, Debug, Serialize, new)]
pub struct RenameRequest<'a> {
    /// Channel to rename
    pub channel: ::ChannelId,
    /// New name for channel.
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResponse {
    ok: bool,
    pub channel: Channel,
}

/// Retrieve a thread of messages posted to a channel
///
/// Wraps https://api.slack.com/methods/channels.replies

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Channel to fetch thread from
    pub channel: ::ChannelId,
    /// Unique identifier of a thread's parent message
    pub thread_ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepliesResponse {
    ok: bool,
    pub has_more: bool,
    pub messages: Vec<Message>,
}

/// Sets the purpose for a channel.
///
/// Wraps https://api.slack.com/methods/channels.setPurpose

#[derive(Clone, Debug, Serialize, new)]
pub struct SetPurposeRequest<'a> {
    /// Channel to set the purpose of
    pub channel: ::ChannelId,
    /// The new purpose
    pub purpose: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPurposeResponse {
    ok: bool,
    pub purpose: String,
}

/// Sets the topic for a channel.
///
/// Wraps https://api.slack.com/methods/channels.setTopic

#[derive(Clone, Debug, Serialize, new)]
pub struct SetTopicRequest<'a> {
    /// Channel to set the topic of
    pub channel: ::ChannelId,
    /// The new topic
    pub topic: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTopicResponse {
    ok: bool,
    pub topic: String,
}

/// Unarchives a channel.
///
/// Wraps https://api.slack.com/methods/channels.unarchive

#[derive(Clone, Debug, Serialize, new)]
pub struct UnarchiveRequest {
    /// Channel to unarchive
    pub channel: ::ChannelId,
}
