use http::Cursor;
use id::*;
use timestamp::Timestamp;

#[derive(Clone, Debug, Deserialize)]
pub struct ResponseMetadata {
    pub next_cursor: String,
}

/// Fetches a conversation's history of messages and events.
///
/// Wraps https://api.slack.com/methods/conversations.history

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

/// Retrieve information about a conversation.
///
/// Wraps https://api.slack.com/methods/conversations.info

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Conversation ID to learn more about
    pub channel: ::ConversationId,
    /// Set this to true to receive the locale for this conversation. Defaults to false
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InfoResponse {
    ok: bool,
    pub channel: ConversationInfo,
}

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
pub struct ListResponse {
    ok: bool,
    pub channels: Vec<Conversation>,
    pub response_metadata: Option<ResponseMetadata>,
}

/// List conversations the calling user may access.
///
/// Wraps https://api.slack.com/methods/users.conversations

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
pub struct ConversationsResponse {
    ok: bool,
    pub channels: Vec<Conversation>,
    pub response_metadata: Option<ResponseMetadata>,
}

#[derive(Clone, Debug, Deserialize)]
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
        parent_conversation: Option<ConversationId>,
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
        parent_conversation: Option<ConversationId>,
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
pub struct ConversationPurpose {
    #[serde(deserialize_with = "deserialize_userid_or_empty")]
    pub creator: Option<UserId>,
    pub last_set: Timestamp,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
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
        parent_conversation: Option<ConversationId>,
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
        parent_conversation: Option<ConversationId>,
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
