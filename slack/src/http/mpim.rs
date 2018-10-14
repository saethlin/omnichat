//! Get info on your multiparty direct messages.

use rtm::{Message, Mpim, ThreadInfo};
use timestamp::Timestamp;

/// Closes a multiparty direct message channel.
///
/// Wraps https://api.slack.com/methods/mpim.close

api_call!(close, "mpim.close", CloseRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct CloseRequest {
    /// MPIM to close.
    pub channel: ::GroupId,
}

/// Fetches history of messages and events from a multiparty direct message.
///
/// Wraps https://api.slack.com/methods/mpim.history

api_call!(history, "mpim.history", HistoryRequest => HistoryResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Multiparty direct message to fetch history for.
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
    pub has_more: Option<bool>,
    pub latest: Option<String>,
    pub messages: Option<Vec<Message>>,
}

/// Lists multiparty direct message channels for the calling user.
///
/// Wraps https://api.slack.com/methods/mpim.list

api_call!(list, "mpim.list", => ListResponse);

#[derive(Clone, Debug, Deserialize)]
pub struct ListResponse {
    ok: bool,
    pub groups: Vec<Mpim>,
}

/// Sets the read cursor in a multiparty direct message channel.
///
/// Wraps https://api.slack.com/methods/mpim.mark

api_call!(mark, "mpim.mark", MarkRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// multiparty direct message channel to set reading cursor in.
    pub channel: ::GroupId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// This method opens a multiparty direct message.
///
/// Wraps https://api.slack.com/methods/mpim.open

api_call!(open, "mpim.open", OpenRequest => OpenResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct OpenRequest<'a> {
    /// Comma separated lists of users.  The ordering of the users is preserved whenever a MPIM group is returned.
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub users: &'a [::UserId],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenResponse {
    ok: bool,
    pub group: Option<Mpim>,
}

/// Retrieve a thread of messages posted to a direct message conversation from a multiparty direct message.
///
/// Wraps https://api.slack.com/methods/mpim.replies

api_call!(replies, "mpim.replies", RepliesRequest => RepliesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Multiparty direct message channel to fetch thread from.
    pub channel: ::GroupId,
    /// Unique identifier of a thread's parent message.
    pub thread_ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepliesResponse {
    ok: bool,
    pub messages: Option<Vec<Message>>,
    pub thread_info: Option<ThreadInfo>,
}
