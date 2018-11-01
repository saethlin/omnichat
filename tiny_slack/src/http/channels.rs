use crate::id::*;
use crate::timestamp::Timestamp;

#[derive(Serialize, new)]
pub struct MarkRequest {
    /// Channel to set reading cursor in.
    pub channel: ChannelId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}
