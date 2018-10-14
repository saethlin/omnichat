use rtm::{File, Message, Paging};
use timestamp::Timestamp;

/// Adds a reaction to an item.
///
/// Wraps https://api.slack.com/methods/reactions.add

api_call!(add, "reactions.add", AddRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct AddRequest<'a> {
    /// Reaction (emoji) name.
    pub name: &'a str,
    #[serde(flatten)]
    pub item: Reactable,
}

/// Gets reactions for an item.
///
/// Wraps https://api.slack.com/methods/reactions.get

api_call!(get, "reactions.get", GetRequest => GetResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct GetRequest {
    #[serde(flatten)]
    pub item: Reactable,
    /// If true always return the complete reaction list.
    #[new(default)]
    pub full: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum GetResponse {
    Message(GetResponseMessage),
    File(GetResponseFile),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResponseFile {
    ok: bool,
    pub file: File,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GetResponseMessage {
    ok: bool,
    pub channel: String,
    pub message: Message,
}

/// Lists reactions made by a user.
///
/// Wraps https://api.slack.com/methods/reactions.list

api_call!(list, "reactions.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Show reactions made by this user. Defaults to the authed user.
    #[new(default)]
    pub user: Option<::UserId>,
    /// If true always return the complete reaction list.
    #[new(default)]
    pub full: Option<bool>,
    /// Number of items to return per page.
    #[new(default)]
    pub count: Option<u32>,
    /// Page number of results to return.
    #[new(default)]
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub items: Option<Vec<ListResponseItem>>,
    pub paging: Option<Paging>,
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
    pub file: File,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponseItemMessage {
    pub channel: String, // TODO: ConversationId probably
    pub message: Message,
}

/// Removes a reaction from an item.
///
/// Wraps https://api.slack.com/methods/reactions.remove

api_call!(remove, "reactions.remove", RemoveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct RemoveRequest<'a> {
    /// Reaction (emoji) name.
    pub name: &'a str,
    #[serde(flatten)]
    pub item: Reactable,
}

#[derive(Clone, Debug)]
pub enum Reactable {
    File(::FileId),
    Message {
        channel: ::ConversationId,
        timestamp: Timestamp,
    },
}

impl ::serde::Serialize for Reactable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            Reactable::File(file) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("file", file)?;
                map.end()
            }
            Reactable::Message { channel, timestamp } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("channel", channel)?;
                map.serialize_entry("timestamp", timestamp)?;
                map.end()
            }
        }
    }
}
