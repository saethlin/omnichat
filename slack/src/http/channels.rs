//! Get info on your team's Slack channels, create or archive channels, invite users, set the topic and purpose, and mark a channel as read.

use rtm::{Channel, Cursor, Message, Paging};
use timestamp::Timestamp;

/// Archives a channel.
///
/// Wraps https://api.slack.com/methods/channels.archive
api_call!(archive, "channels.archive", ArchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct ArchiveRequest {
    /// Channel to archive
    pub channel: ::ChannelId,
}

/// Creates a channel.
///
/// Wraps https://api.slack.com/methods/channels.create

api_call!(create, "channels.create", CreateRequest => CreateResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct CreateRequest<'a> {
    /// Name of channel to create
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    ok: bool,
    pub channel: Channel,
}

/// Fetches history of messages and events from a channel.
///
/// Wraps https://api.slack.com/methods/channels.history

api_call!(history, "channels.history", HistoryRequest => HistoryResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct HistoryRequest {
    /// Channel to fetch history for.
    pub channel: ::ChannelId,
    /// End of time range of messages to include in results.
    #[new(default)]
    pub latest: Option<Timestamp>,
    /// Start of time range of messages to include in results.
    #[new(default)]
    pub oldest: Option<Timestamp>,
    /// Include messages with latest or oldest timestamp in results.
    #[new(default)]
    pub inclusive: Option<bool>,
    /// Number of messages to return, between 1 and 1000.
    #[new(default)]
    pub count: Option<u32>,
    /// Include unread_count_display in the output?
    #[new(default)]
    pub unreads: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryResponse {
    ok: bool,
    pub has_more: Option<bool>,
    pub latest: Option<Timestamp>,
    pub messages: Vec<Message>,
    pub is_limited: Option<bool>,
}

/// Gets information about a channel.
///
/// Wraps https://api.slack.com/methods/channels.info

api_call!(info, "channels.info", InfoRequest => InfoResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Channel to get info on
    pub channel: ::ChannelId,
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub channel: Channel,
}

/// Invites a user to a channel.
///
/// Wraps https://api.slack.com/methods/channels.invite

api_call!(invite, "channels.invite", InviteRequest => InviteResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InviteRequest {
    /// Channel to invite user to.
    pub channel: ::ChannelId,
    /// User to invite to channel.
    pub user: ::UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteResponse {
    ok: bool,
    pub channel: Channel,
}

/// Joins a channel, creating it if needed.
///
/// Wraps https://api.slack.com/methods/channels.join

api_call!(join, "channels.join", JoinRequest => JoinResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct JoinRequest<'a> {
    /// Name of channel to join
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinResponse {
    ok: bool,
    pub channel: Channel, //TODO: This contains different attributes depending on already_in_channel
    pub already_in_channel: Option<bool>,
}

/// Removes a user from a channel.
///
/// Wraps https://api.slack.com/methods/channels.kick

api_call!(kick, "channels.kick", KickRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct KickRequest {
    /// Channel to remove user from.
    pub channel: ::ChannelId,
    /// User to remove from channel.
    pub user: ::UserId,
}

/// Leaves a channel.
///
/// Wraps https://api.slack.com/methods/channels.leave

api_call!(leave, "channels.leave", LeaveRequest => LeaveResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct LeaveRequest {
    /// Channel to leave
    pub channel: ::ChannelId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaveResponse {
    ok: bool,
    not_in_channel: Option<bool>,
}

/// Lists all channels in a Slack team.
///
/// Wraps https://api.slack.com/methods/channels.list

api_call!(list, "channels.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Exclude archived channels from the list
    #[new(default)]
    pub exclude_archived: Option<bool>,
    /// Exclude the members collection from each channel
    #[new(default)]
    pub exclude_members: Option<bool>,
    #[new(default)]
    pub cursor: Option<Cursor>,
    #[new(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub channels: Vec<Channel>,
    pub response_metadata: Option<Paging>,
}

/// Sets the read cursor in a channel.
///
/// Wraps https://api.slack.com/methods/channels.mark

api_call!(mark, "channels.mark", MarkRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct MarkRequest {
    /// Channel to set reading cursor in.
    pub channel: ::ChannelId,
    /// Timestamp of the most recently seen message.
    pub ts: Timestamp,
}

/// Renames a channel.
///
/// Wraps https://api.slack.com/methods/channels.rename

api_call!(rename, "channels.rename", RenameRequest => RenameResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RenameRequest<'a> {
    /// Channel to rename
    pub channel: ::ChannelId,
    /// New name for channel.
    pub name: &'a str,
    /// Whether to return errors on invalid channel name instead of modifying it to meet the specified criteria.
    #[new(default)]
    pub validate: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameResponse {
    ok: bool,
    pub channel: Channel,
}

/// Retrieve a thread of messages posted to a channel
///
/// Wraps https://api.slack.com/methods/channels.replies

api_call!(replies, "channels.replies", RepliesRequest => RepliesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct RepliesRequest {
    /// Channel to fetch thread from
    pub channel: ::ChannelId,
    /// Unique identifier of a thread's parent message
    pub thread_ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepliesResponse {
    ok: bool,
    pub has_more: bool,
    pub messages: Vec<Message>,
}

/// Sets the purpose for a channel.
///
/// Wraps https://api.slack.com/methods/channels.setPurpose

api_call!(
    set_purpose,
    "channels.setPurpose",
    SetPurposeRequest =>
    SetPurposeResponse
);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetPurposeRequest<'a> {
    /// Channel to set the purpose of
    pub channel: ::ChannelId,
    /// The new purpose
    pub purpose: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPurposeResponse {
    ok: bool,
    pub purpose: String,
}

/// Sets the topic for a channel.
///
/// Wraps https://api.slack.com/methods/channels.setTopic

api_call!(set_topic, "channels.setTopic", SetTopicRequest => SetTopicResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetTopicRequest<'a> {
    /// Channel to set the topic of
    pub channel: ::ChannelId,
    /// The new topic
    pub topic: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetTopicResponse {
    ok: bool,
    pub topic: String,
}

/// Unarchives a channel.
///
/// Wraps https://api.slack.com/methods/channels.unarchive

api_call!(unarchive, "channels.unarchive", UnarchiveRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct UnarchiveRequest {
    /// Channel to unarchive
    pub channel: ::ChannelId,
}

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static! {
        pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
        pub static ref TOKEN: String = ::std::env::var("SLACK_API_TOKEN").unwrap();
    }

    #[test]
    fn test_archive_unarchive() {
        let id = ::ChannelId::from("CAGMCM14K");
        let _ = unarchive(&*CLIENT, &TOKEN, &UnarchiveRequest::new(id));

        archive(&*CLIENT, &TOKEN, &ArchiveRequest::new(id)).unwrap();

        unarchive(&*CLIENT, &TOKEN, &UnarchiveRequest::new(id)).unwrap();
    }

    #[test]
    fn test_create() {
        match create(&*CLIENT, &TOKEN, &CreateRequest::new("testchannel")) {
            Ok(_) => {}
            Err(::requests::Error::Slack(cause)) => {
                if cause != "name_taken" {
                    panic!(cause);
                }
            }
            Err(e) => panic!(e),
        }
    }

    #[test]
    fn test_history() {
        let id = ::ChannelId::from("CAGMCM14K");
        history(&*CLIENT, &TOKEN, &HistoryRequest::new(id)).unwrap();
    }

    #[test]
    fn test_info() {
        let id = ::ChannelId::from("CAGMCM14K");
        info(&*CLIENT, &TOKEN, &InfoRequest::new(id)).unwrap();
    }

    #[test]
    fn test_invite_kick() {
        let chan_id = ::ChannelId::from("CAGMCM14K");
        let user_id = ::UserId::from("UAJHFUB0C");
        let _ = kick(&*CLIENT, &TOKEN, &KickRequest::new(chan_id, user_id));

        invite(&*CLIENT, &TOKEN, &InviteRequest::new(chan_id, user_id)).unwrap();

        kick(&*CLIENT, &TOKEN, &KickRequest::new(chan_id, user_id)).unwrap();
    }

    #[test]
    fn test_join_leave() {
        let id = ::ChannelId::from("CAGMCM14K");
        let _ = leave(&*CLIENT, &TOKEN, &LeaveRequest::new(id));

        join(&*CLIENT, &TOKEN, &JoinRequest::new("#testchannel")).unwrap();

        leave(&*CLIENT, &TOKEN, &LeaveRequest::new(id)).unwrap();
    }

    #[test]
    fn test_list() {
        list(&*CLIENT, &TOKEN, &ListRequest::new()).unwrap();
    }

    #[test]
    fn test_mark() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
        let time_string = format!("{}", since_the_epoch.as_secs());
        let ts = ::serde_json::from_str(&time_string).unwrap();

        let id = ::ChannelId::from("C9VGPGBL4");
        mark(&*CLIENT, &TOKEN, &MarkRequest::new(id, ts)).unwrap();
    }

    #[test]
    fn test_rename() {
        let id = ::ChannelId::from("CAGMCM14K");
        let _ = rename(&*CLIENT, &TOKEN, &RenameRequest::new(id, "testchannel"));

        rename(
            &*CLIENT,
            &TOKEN,
            &RenameRequest::new(id, "other_testchannel"),
        ).unwrap();

        rename(&*CLIENT, &TOKEN, &RenameRequest::new(id, "testchannel")).unwrap();
    }

    #[test]
    fn test_replies() {
        let id = ::ChannelId::from("CAGMCM14K");
        let ts = ::serde_json::from_str("\"1525306421.000207\"").unwrap();
        replies(&*CLIENT, &TOKEN, &RepliesRequest::new(id, ts)).unwrap();
    }

    #[test]
    fn test_set_purpose() {
        let id = ::ChannelId::from("CAGMCM14K");

        join(&*CLIENT, &TOKEN, &JoinRequest::new("#testchannel"));

        let mut req = SetPurposeRequest::new(id, "test_purpose");
        let response = set_purpose(&*CLIENT, &TOKEN, &req).unwrap();
        assert_eq!(response.purpose, "test_purpose");

        req.purpose = "other_test_purpose";
        let response = set_purpose(&*CLIENT, &TOKEN, &req).unwrap();
        assert_eq!(response.purpose, "other_test_purpose");
    }

    #[test]
    fn test_set_topic() {
        let id = ::ChannelId::from("CAGMCM14K");

        join(&*CLIENT, &TOKEN, &JoinRequest::new("#testchannel"));

        let mut req = SetTopicRequest::new(id, "test_topic");

        let response = set_topic(&*CLIENT, &TOKEN, &req).unwrap();
        assert_eq!(response.topic, "test_topic");

        req.topic = "other_test_topic";
        let response = set_topic(&*CLIENT, &TOKEN, &req).unwrap();
        assert_eq!(response.topic, "other_test_topic");
    }
}
