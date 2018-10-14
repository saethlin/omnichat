use rtm::{File, Message};
use timestamp::Timestamp;

/// Pins an item to a channel.
///
/// Wraps https://api.slack.com/methods/pins.add

#[derive(Clone, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub enum Pinnable {
    /// File to pin or unpin
    File(::FileId),
    /// Timestamp of the message to pin or unpin
    Timestamp(::Timestamp),
}

api_call!(add, "pins.add", AddRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct AddRequest {
    /// Channel to pin the item in.
    pub channel: ::ConversationId,
    #[serde(flatten)]
    pub item: Pinnable,
}

/// Lists items pinned to a channel.
///
/// Wraps https://api.slack.com/methods/pins.list

api_call!(list, "pins.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Channel to get pinned items for.
    pub channel: ::ConversationId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    pub items: Option<Vec<ListResponseItem>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum ListResponseItem {
    Message(ListResponseItemMessage),
    File(ListResponseItemFile),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponseItemFile {
    pub created: Option<Timestamp>,
    pub created_by: Option<::UserId>,
    pub file: File,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponseItemMessage {
    pub channel: ::ConversationId,
    pub created: Option<Timestamp>,
    pub created_by: Option<::UserId>,
    pub message: Message,
}

/// Un-pins an item from a channel.
///
/// Wraps https://api.slack.com/methods/pins.remove

api_call!(remove, "pins.remove", RemoveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct RemoveRequest {
    /// Channel where the item is pinned to.
    pub channel: ::ConversationId,
    #[serde(flatten)]
    pub item: Pinnable,
}
