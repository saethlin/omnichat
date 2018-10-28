use id::*;
use timestamp::Timestamp;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Event {
    ChannelMarked {
        channel: ChannelId,
        ts: Timestamp,
    },
    GroupMarked {
        channel: GroupId,
        ts: Timestamp,
    },
    ImMarked {
        channel: DmId,
        ts: Timestamp,
    },
    Message {
        channel: ConversationId,
        text: String,
        user: Option<UserId>,
        username: Option<String>,
        ts: Timestamp,
    },
    ReactionAdded {
        item: Reactable,
        reaction: String,
    },
    ReactionRemoved {
        item: Reactable,
        reaction: String,
    },
    Hello {},
    PrefChange {},
    UserTyping {},
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum Reactable {
    Message {
        channel: ConversationId,
        ts: Timestamp,
    },
}
