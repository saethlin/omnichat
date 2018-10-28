use timestamp::Timestamp;

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Direct message channel to set reading cursor in.
    pub channel: ::DmId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}
