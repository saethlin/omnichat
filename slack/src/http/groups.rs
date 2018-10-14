//! Get info on your team's private channels.

use rtm::{Group, Message, ThreadInfo};
use timestamp::Timestamp;

/// Archives a private channel.
///
/// Wraps https://api.slack.com/methods/groups.archive

api_call!(archive, "groups.archive", ArchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct ArchiveRequest {
    /// Private channel to archive
    pub channel: ::GroupId,
}

/// Closes a private channel.
///
/// Wraps https://api.slack.com/methods/groups.close

api_call!(close, "groups.close", CloseRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct CloseRequest {
    /// Private channel to close.
    pub channel: ::GroupId,
}

/// Creates a private channel.
///
/// Wraps https://api.slack.com/methods/groups.create

api_call!(create, "groups.create", CreateRequest => CreateResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct CreateRequest<'a> {
    /// Name of private channel to create
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    ok: bool,
    pub group: Option<Group>,
}

/// Clones and archives a private channel.
///
/// Wraps https://api.slack.com/methods/groups.createChild

api_call!(
    create_child,
    "groups.createChild",
    CreateChildRequest =>
    CreateChildResponse
);

#[derive(Clone, Debug, Serialize, new)]
pub struct CreateChildRequest {
    /// Private channel to clone and archive.
    pub channel: ::GroupId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateChildResponse {
    ok: bool,
    pub group: Option<Group>,
}

/// Fetches history of messages and events from a private channel.
///
/// Wraps https://api.slack.com/methods/groups.history

api_call!(history, "groups.history", HistoryRequest => HistoryResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Private channel to fetch history for.
    pub channel: ::GroupId,
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
    pub has_more: bool,
    pub latest: Option<Timestamp>,
    pub messages: Vec<Message>,
    pub is_limited: Option<bool>,
}

/// Gets information about a private channel.
///
/// Wraps https://api.slack.com/methods/groups.info

api_call!(info, "groups.info", InfoRequest => InfoResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Private channel to get info on
    pub channel: ::GroupId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub group: Group,
}

/// Invites a user to a private channel.
///
/// Wraps https://api.slack.com/methods/groups.invite

api_call!(invite, "groups.invite", InviteRequest => InviteResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InviteRequest {
    /// Private channel to invite user to.
    pub channel: ::GroupId,
    /// User to invite.
    pub user: ::UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteResponse {
    ok: bool,
    pub group: Option<Group>,
}

/// Removes a user from a private channel.
///
/// Wraps https://api.slack.com/methods/groups.kick

api_call!(kick, "groups.kick", KickRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct KickRequest {
    /// Private channel to remove user from.
    pub channel: ::GroupId,
    /// User to remove from private channel.
    pub user: ::UserId,
}

/// Leaves a private channel.
///
/// Wraps https://api.slack.com/methods/groups.leave

api_call!(leave, "groups.leave", LeaveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct LeaveRequest {
    /// Private channel to leave
    pub channel: ::GroupId,
}

/// Lists private channels that the calling user has access to.
///
/// Wraps https://api.slack.com/methods/groups.list

api_call!(list, "groups.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Don't return archived private channels.
    #[new(default)]
    pub exclude_archived: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub groups: Vec<Group>,
}

/// Sets the read cursor in a private channel.
///
/// Wraps https://api.slack.com/methods/groups.mark

api_call!(mark, "groups.mark", MarkRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Private channel to set reading cursor in.
    pub channel: ::GroupId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Opens a private channel.
///
/// Wraps https://api.slack.com/methods/groups.open

api_call!(open, "groups.open", OpenRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct OpenRequest {
    /// Private channel to open.
    pub channel: ::GroupId,
}

/// Renames a private channel.
///
/// Wraps https://api.slack.com/methods/groups.rename

api_call!(rename, "groups.rename", RenameRequest => RenameResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RenameRequest<'a> {
    /// Private channel to rename
    pub channel: ::GroupId,
    /// New name for private channel.
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResponse {
    ok: bool,
    pub channel: Option<RenameResponseGroup>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResponseGroup {
    pub created: Option<Timestamp>,
    pub id: Option<::GroupId>,
    pub is_group: Option<bool>,
    pub name: Option<String>,
}

/// Retrieve a thread of messages posted to a private channel
///
/// Wraps https://api.slack.com/methods/groups.replies

api_call!(replies, "groups.replies", RepliesRequest => RepliesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Private channel to fetch thread from
    pub channel: ::GroupId,
    /// Unique identifier of a thread's parent message
    pub thread_ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepliesResponse {
    ok: bool,
    pub messages: Option<Vec<Message>>,
    pub thread_info: Option<ThreadInfo>,
}

/// Sets the purpose for a private channel.
///
/// Wraps https://api.slack.com/methods/groups.setPurpose

api_call!(set_purpose, "groups.setPurpose", SetPurposeRequest => SetPurposeResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetPurposeRequest<'a> {
    /// Private channel to set the purpose of
    pub channel: ::GroupId,
    /// The new purpose
    pub purpose: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPurposeResponse {
    ok: bool,
    pub purpose: Option<String>,
}

/// Sets the topic for a private channel.
///
/// Wraps https://api.slack.com/methods/groups.setTopic

api_call!(set_topic, "groups.setTopic", SetTopicRequest => SetTopicResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetTopicRequest<'a> {
    /// Private channel to set the topic of
    pub channel: ::GroupId,
    /// The new topic
    pub topic: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTopicResponse {
    ok: bool,
    pub topic: Option<String>,
}

/// Unarchives a private channel.
///
/// Wraps https://api.slack.com/methods/groups.unarchive

api_call!(unarchive, "groups.unarchive", UnarchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct UnarchiveRequest {
    /// Private channel to unarchive
    pub channel: ::GroupId,
}
