use id::*;
use rtm::{Cursor, Message};
use timestamp::Timestamp;

/// Archives a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.archive

api_call!(archive, "conversations.archive", ArchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct ArchiveRequest {
    /// ID of conversation to archive
    pub channe: ::ConversationId,
}

/// Closes a direct message or multi-person direct message.
///
/// Wraps https://api.slack.com/methods/conversations.close

api_call!(close, "conversations.close", CloseRequest => CloseResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct CloseRequest {
    /// Conversation to close.
    pub channel: ::ConversationId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseResponse {
    ok: bool,
    no_op: Option<bool>,
    already_closed: Option<bool>,
}

/// Initiates a public or private channel-based conversation
///
/// Wraps https://api.slack.com/methods/conversations.create

api_call!(create, "conversations.create", CreateRequest => CreateResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct CreateRequest<'a> {
    /// Name of private channel to create
    pub name: &'a str,

    /// Create a private channel instead of a public one
    #[new(default)]
    pub is_private: Option<bool>,

    /// Required for workspace apps. A list of between 1 and 30 human users that will be added to the newly-created conversation. This argument has no effect when used by classic Slack apps.
    #[new(default)]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub user_ids: Vec<::UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    ok: bool,
    pub channel: Option<Conversation>,
}

/// Fetches a conversation's history of messages and events.
///
/// Wraps https://api.slack.com/methods/conversations.history

api_call!(history, "conversations.history", HistoryRequest => HistoryResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Conversation ID to fetch history for.
    pub channel: ::ConversationId,

    /// Paginate through collections of data by setting the cursor parameter to a next_cursor attribute returned by a previous request's response_metadata. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,

    /// Include messages with latest or oldest timestamp in results only when either timestamp is specified
    #[new(default)]
    pub inclusive: Option<bool>,

    /// End of time range of messages to include in results.
    #[new(default)]
    pub latest: Option<Timestamp>,

    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the users list hasn't been reached.
    #[new(default)]
    pub limit: Option<u32>,

    /// Start of time range of messages to include in results.
    #[new(default)]
    pub oldest: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryResponse {
    ok: bool,
    pub messages: Vec<Message>,
    pub has_more: bool,
    pub pin_count: u32,
    pub response_metadata: Option<ResponseMetadata>,
    pub is_limited: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseMetadata {
    next_cursor: Cursor,
}

/// Retrieve information about a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.info

api_call!(info, "conversations.info", InfoRequest => InfoResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Conversation ID to learn more about
    pub channel: ::ConversationId,
    /// Set this to true to receive the locale for this conversation. Defaults to false
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub channel: ConversationInfo,
}

/// Invites users to a channel.
///
/// Wraps https://api.slack.com/methods/conversations.invite

api_call!(invite, "conversations.invite", InviteRequest => InviteResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InviteRequest {
    /// The ID of the public or private channel to invite user(s) to.
    pub channel: ::ConversationId,
    /// A comma separated list of user IDs. Up to 30 users may be listed.
    #[new(default)]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub users: Vec<::UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteResponse {
    ok: bool,
    pub channel: Conversation,
}

/// Joins an existing conversation.
///
/// https://api.slack.com/methods/conversations.join

api_call!(join, "conversations.join", JoinRequest => JoinResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct JoinRequest {
    /// ID of conversation to join
    pub channel: ::ConversationId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinResponse {
    pub channel: Conversation,
    pub warning: Option<String>,
    pub response_metadata: Option<JoinResponseMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JoinResponseMetadata {
    pub warnings: Option<Vec<String>>,
}

/// Removes a user from a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.kick

api_call!(kick, "conversations.kick", KickRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct KickRequest {
    /// ID of conversation to remove user from.
    pub channel: ::ConversationId,
    /// User ID to be removed.
    pub user: ::UserId,
}

/// Leaves a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.leave

api_call!(leave, "conversations.leave", LeaveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct LeaveRequest {
    /// Conversation to leave
    pub channel: ::ConversationId,
}

/// Lists all channels in a Slack team.
///
/// Wraps https://api.slack.com/methods/conversations.list

api_call!(list, "conversations.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Paginate through collections of data by setting the cursor parameter to a next_cursor attribute returned by a previous request's response_metadata. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,

    /// Set to true to exclude archived channels from the list
    #[new(default)]
    pub exclude_archived: Option<bool>,

    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the list hasn't been reached. Must be an integer no larger than 1000.
    #[new(default)]
    pub limit: Option<u32>,

    /// Mix and match channel types by providing a comma-separated list of any combination of public_channel, private_channel, mpim, im
    #[new(value = "vec![ChannelType::PublicChannel]")]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub types: Vec<ChannelType>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    PublicChannel,
    PrivateChannel,
    Mpim,
    Im,
}

impl ::std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        use self::ChannelType::*;
        match self {
            PublicChannel => write!(f, "public_channel"),
            PrivateChannel => write!(f, "private_channel"),
            Mpim => write!(f, "mpim"),
            Im => write!(f, "im"),
        }
    }
}

// TODO: This returns a _partial_ conversation object, per the slack docs
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub channels: Vec<Conversation>,
    pub response_metadata: Option<ResponseMetadata>,
}

/// Retrieve members of a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.members

api_call!(members, "conversations.members", MembersRequest => MembersResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct MembersRequest {
    /// ID of the conversation to retrieve members for
    pub channel: ::ConversationId,

    /// Paginate through collections of data by setting the cursor parameter to a next_cursor attribute returned by a previous request's response_metadata. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,

    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the users list hasn't been reached.
    #[new(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MembersResponse {
    ok: bool,
    pub members: Vec<::UserId>,
    pub response_metadata: Option<ResponseMetadata>,
}

// TODO: Undocumented method
/// Sets the read cursor in a private channel.
///
/// Wraps https://api.slack.com/methods/conversations.mark

api_call!(mark, "conversations.mark", MarkRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Private channel to set reading cursor in.
    pub channel: ::ConversationId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Renames a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.rename

api_call!(rename, "conversations.rename", RenameRequest => RenameResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RenameRequest<'a> {
    /// ID of conversation to rename
    pub channel: ::ConversationId,
    /// New name for conversation.
    pub name: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResponse {
    ok: bool,
    pub channel: Conversation,
}

/// Retrieve a thread of messages posted to a conversation
///
/// Wraps https://api.slack.com/methods/conversations.replies

api_call!(replies, "conversations.replies", RepliesRequest => RepliesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Conversation ID to fetch thread from.
    pub channel: ::ConversationId,
    /// Unique identifier of a thread's parent message.
    pub ts: Timestamp,

    /// Paginate through collections of data by setting the cursor parameter to a next_cursor attribute returned by a previous request's response_metadata. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,

    /// Include messages with latest or oldest timestamp in results only when either timestamp is specified.
    #[new(default)]
    pub inclusivie: Option<bool>,

    /// End of time range of messages to include in results.
    #[new(default)]
    pub latest: Timestamp,

    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the users list hasn't been reached.
    #[new(default)]
    pub limit: Option<u32>,

    /// Start of time range of messages to include in results.
    #[new(default)]
    pub oldest: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepliesResponse {
    ok: bool,
    #[serde(default)]
    pub messages: Vec<Message>,
    pub has_more: Option<bool>,
    pub response_metadata: Option<ResponseMetadata>,
}

/// Sets the purpose for a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.setPurpose

api_call!(set_purpose, "conversations.setPurpose", SetPurposeRequest => SetPurposeResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetPurposeRequest<'a> {
    /// Conversation to set the purpose of
    pub channel: ::ConversationId,
    /// A new, specialer purpose
    pub purpose: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPurposeResponse {
    ok: bool,
    pub purpose: String,
}

/// Sets the topic for a conversation
///
/// Wraps https://api.slack.com/methods/conversations.setTopic

api_call!(set_topic, "conversations.setTopic", SetTopicRequest => SetTopicResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetTopicRequest<'a> {
    /// Conversation to set the topic of
    pub channel: ::ConversationId,
    /// The new topic string. Does not support formatting or linkification.
    pub topic: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTopicResponse {
    ok: bool,
    pub topic: String,
}

/// Reverses conversation archival.
///
/// Wraps https://api.slack.com/methods/conversations.unarchive

api_call!(unarchive, "conversations.unarchive", UnarchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct UnarchiveRequest {
    /// ID of conversation to unarchive
    pub channel: ::ConversationId,
}

/// List conversations the calling user may access.
///
/// Wraps https://api.slack.com/methods/users.conversations

api_call!(conversations, "users.conversations", ConversationsRequest => ConversationsResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ConversationsRequest {
    /// Paginate through collections of data by setting the cursor parameter to a next_cursor attribute returned by a previous request's response_metadata. Default value fetches the first "page" of the collection. See pagination for more detail.
    #[new(default)]
    pub cursor: Option<Cursor>,

    /// Set to true to exclude archived channels from the list
    #[new(default)]
    pub exclude_archived: Option<bool>,

    /// The maximum number of items to return. Fewer than the requested number of items may be returned, even if the end of the list hasn't been reached. Must be an integer no larger than 1000.
    #[new(default)]
    pub limit: Option<u32>,

    /// Mix and match channel types by providing a comma-separated list of any combination of public_channel, private_channel, mpim, im
    #[new(default)]
    pub types: Option<Vec<ChannelType>>,

    /// Browse conversations by a specific user ID's membership. Non-public channels are restricted to those where the calling user shares membership.
    #[new(default)]
    pub user: Option<::UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationsResponse {
    ok: bool,
    pub channels: Vec<Conversation>,
    pub response_metadata: Option<ResponseMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Conversation {
    Channel {
        created: Timestamp,
        creator: UserId,
        id: ConversationId,
        is_archived: bool,
        is_channel: bool,
        is_ext_shared: bool,
        is_general: bool,
        is_group: bool,
        is_im: bool,
        is_member: bool,
        is_mpim: bool,
        is_org_shared: bool,
        is_pending_ext_shared: bool,
        is_private: bool,
        is_shared: bool,
        name: String,
        name_normalized: String,
        num_members: u32,
        pending_shared: Vec<String>,
        previous_names: Vec<String>,
        purpose: ConversationPurpose,
        shared_team_ids: Vec<TeamId>,
        topic: ConversationTopic,
        unlinked: u32,
    },
    Group {
        created: Timestamp,
        creator: UserId,
        id: ConversationId,
        is_archived: bool,
        is_channel: bool,
        is_ext_shared: bool,
        is_general: bool,
        is_group: bool,
        is_im: bool,
        is_member: bool,
        is_mpim: bool,
        is_open: Option<bool>,
        is_org_shared: bool,
        is_pending_ext_shared: bool,
        is_private: bool,
        is_shared: bool,
        last_read: Timestamp,
        name: String,
        name_normalized: String,
        pending_shared: Vec<String>,
        priority: f32,
        purpose: ConversationPurpose,
        shared_team_ids: Vec<TeamId>,
        topic: ConversationTopic,
        unlinked: u32,
    },
    DirectMessage {
        created: Timestamp,
        id: ConversationId,
        is_im: bool,
        is_org_shared: bool,
        is_user_deleted: bool,
        priority: f32,
        user: UserId,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationPurpose {
    #[serde(deserialize_with = "deserialize_userid_or_empty")]
    pub creator: Option<UserId>,
    pub last_set: Timestamp,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTopic {
    #[serde(deserialize_with = "deserialize_userid_or_empty")]
    pub creator: Option<UserId>,
    pub last_set: Timestamp,
    pub value: String,
}

fn deserialize_userid_or_empty<'de, D>(deserializer: D) -> Result<Option<UserId>, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    struct TheVisitor;
    impl<'de> ::serde::de::Visitor<'de> for TheVisitor {
        type Value = Option<UserId>;

        fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            formatter.write_str("an empty string or a UserId")
        }

        fn visit_str<E: ::serde::de::Error>(self, value: &str) -> Result<Option<UserId>, E> {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(UserId::from(value)))
            }
        }
    }

    deserializer.deserialize_str(TheVisitor)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum ConversationInfo {
    Channel {
        created: Timestamp,
        creator: UserId,
        id: ChannelId,
        is_archived: bool,
        is_channel: bool,
        is_ext_shared: bool,
        is_general: bool,
        is_group: bool,
        is_im: bool,
        is_member: bool,
        is_mpim: bool,
        is_org_shared: bool,
        is_pending_ext_shared: bool,
        is_private: bool,
        /// Present on the general channel for free plans, possibly all channels otherwise
        is_read_only: Option<bool>,
        is_shared: bool,
        /// Present if is_member is true
        last_read: Option<Timestamp>,
        name: String,
        name_normalized: String,
        pending_shared: Vec<String>,
        previous_names: Vec<String>,
        purpose: ConversationPurpose,
        shared_team_ids: Vec<TeamId>,
        topic: ConversationTopic,
        unlinked: u32,
    },
    Group {
        created: Timestamp,
        creator: UserId,
        id: GroupId,
        is_archived: bool,
        is_channel: bool,
        is_ext_shared: bool,
        is_general: bool,
        is_group: bool,
        is_im: bool,
        is_member: bool,
        is_mpim: bool,
        is_open: bool,
        is_org_shared: bool,
        is_pending_ext_shared: bool,
        is_private: bool,
        is_shared: bool,
        last_read: Timestamp,
        name: String,
        name_normalized: String,
        pending_shared: Vec<String>,
        purpose: ConversationPurpose,
        shared_team_ids: Vec<TeamId>,
        topic: ConversationTopic,
        unlinked: u32,
    },
    OpenDirectMessage {
        created: Timestamp,
        id: ConversationId,
        is_im: bool,
        is_open: bool,
        is_org_shared: bool,
        last_read: Timestamp,
        // Just... why...
        latest: Option<::rtm::Message>,
        priority: f32,
        unread_count: u32,
        unread_count_display: u32,
        user: UserId,
    },
    ClosedDirectMessage {
        created: Timestamp,
        id: ConversationId,
        is_im: bool,
        is_org_shared: bool,
        is_user_deleted: bool,
        priority: f32,
        user: UserId,
    },
}
