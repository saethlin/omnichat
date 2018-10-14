use rtm::{File, Message, Paging};
use timestamp::Timestamp;

/// Adds a star to an item.
///
/// Wraps https://api.slack.com/methods/stars.add

api_call!(add, "stars.add", AddRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct AddRequest {
    #[serde(flatten)]
    pub item: Starrable,
}

/// Lists stars for a user.
///
/// Wraps https://api.slack.com/methods/stars.list

api_call!(list, "stars.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
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
pub enum ListResponseItem {
    Message { channel: String, message: Message },
    File { file: File },
    Channel { channel: String },
    Im { channel: String },
    Group { group: String },
}

/// Removes a star from an item.
///
/// Wraps https://api.slack.com/methods/stars.remove

api_call!(remove, "stars.remove", RemoveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct RemoveRequest {
    #[serde(flatten)]
    pub item: Starrable,
}

#[derive(Clone, Debug)]
pub enum Starrable {
    File(::FileId),
    Channel(::ConversationId),
    Message {
        channel: ::ConversationId,
        timestamp: Timestamp,
    },
}

impl ::serde::Serialize for Starrable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            Starrable::File(file) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("File", file)?;
                map.end()
            }
            Starrable::Channel(channel) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("Channel", channel)?;
                map.end()
            }
            Starrable::Message { channel, timestamp } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("Channel", channel)?;
                map.serialize_entry("timestamp", timestamp)?;
                map.end()
            }
        }
    }
}
