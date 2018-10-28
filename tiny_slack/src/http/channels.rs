use timestamp::Timestamp;

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Channel to set reading cursor in.
    pub channel: ::ChannelId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}
