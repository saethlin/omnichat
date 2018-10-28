use id::{BotId, UserId};
use timestamp::Timestamp;

#[derive(Clone, Debug, Deserialize)]
pub struct Message {
    pub text: String,
    pub user: Option<UserId>,
    pub username: Option<String>,
    pub bot_id: Option<BotId>,
    pub ts: Timestamp,
    pub reactions: Vec<Reaction>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Reaction {
    name: String,
    count: u32,
}
