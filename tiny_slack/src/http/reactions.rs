use timestamp::Timestamp;

/// Adds a reaction to an item.
///
/// Wraps https://api.slack.com/methods/reactions.add

#[derive(Clone, Debug, Serialize, new)]
pub struct AddRequest<'a> {
    /// Reaction (emoji) name.
    pub name: &'a str,
    #[serde(flatten)]
    pub item: Reactable,
}

/// Removes a reaction from an item.
///
/// Wraps https://api.slack.com/methods/reactions.remove

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
