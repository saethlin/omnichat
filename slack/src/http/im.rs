//! Get info on your direct messages.

use rtm::{Cursor, Im, Message, ThreadInfo};
use timestamp::Timestamp;

/// Close a direct message channel.
///
/// Wraps https://api.slack.com/methods/im.close

api_call!(close, "im.close", CloseRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct CloseRequest {
    /// Direct message channel to close.
    pub channel: ::DmId,
}

/// Fetches history of messages and events from direct message channel.
///
/// Wraps https://api.slack.com/methods/im.history

api_call!(history, "im.history", HistoryRequest => HistoryResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Direct message channel to fetch history for.
    pub channel: ::DmId,
    /// End of time range of messages to include in results.
    #[new(default)]
    pub latest: Option<::Timestamp>,
    /// Start of time range of messages to include in results.
    #[new(default)]
    pub oldest: Option<::Timestamp>,
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
    #[serde(default)]
    pub messages: Vec<Message>,
    pub is_limited: Option<bool>,
}

/// Lists direct message channels for the calling user.
///
/// Wraps https://api.slack.com/methods/im.list

api_call!(list, "im.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Paginate through collections of data by setting the `cursor` parameter to a `next_cursor` attribute returned by a previous request's `response_metadata`. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,
    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the users list hasn't been reached.
    #[new(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    #[serde(default)]
    pub ims: Vec<Im>,
}

/// Sets the read cursor in a direct message channel.
///
/// Wraps https://api.slack.com/methods/im.mark

api_call!(mark, "im.mark", MarkRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Direct message channel to set reading cursor in.
    pub channel: ::DmId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Opens a direct message channel.
///
/// Wraps https://api.slack.com/methods/im.open

api_call!(open, "im.open", OpenRequest => OpenResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct OpenRequest {
    /// User to open a direct message channel with.
    pub user: ::UserId,
    /// Boolean, indicates you want the full IM channel definition in the response.
    #[new(default)]
    pub return_im: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpenResponse {
    ok: bool,
    pub channel: Option<Im>,
}

/// Retrieve a thread of messages posted to a direct message conversation
///
/// Wraps https://api.slack.com/methods/im.replies

api_call!(replies, "im.replies", RepliesRequest => RepliesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Direct message channel to fetch thread from
    pub channel: ::DmId,
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
