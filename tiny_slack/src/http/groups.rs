use timestamp::Timestamp;

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Private channel to set reading cursor in.
    pub channel: ::GroupId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}
