//! Post chat messages to Slack.

use rtm::Message;
use timestamp::Timestamp;

/// Deletes a message.
///
/// Wraps https://api.slack.com/methods/chat.delete

api_call!(delete, "chat.delete", DeleteRequest => DeleteResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct DeleteRequest {
    /// Timestamp of the message to be deleted.
    pub ts: Timestamp,
    /// Channel containing the message to be deleted.
    pub channel: ::ConversationId,
    /// Pass true to delete the message as the authed user. Bot users in this context are considered authed users.
    #[new(default)]
    pub as_user: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteResponse {
    ok: bool,
    pub channel: ::ChannelId,
    pub ts: Timestamp,
}

/// Share a me message into a channel.
///
/// Wraps https://api.slack.com/methods/chat.meMessage

api_call!(me_message, "chat.meMessage", MeMessageRequest => MeMessageResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct MeMessageRequest<'a> {
    /// Channel to send message to. Can be a public channel, private group or IM channel. Can be an encoded ID, or a name.
    pub channel: ::ConversationId,
    /// Text of the message to send.
    pub text: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeMessageResponse {
    ok: bool,
    pub channel: Option<String>,
    pub ts: Option<Timestamp>,
}

/// Sends a message to a channel.
///
/// Wraps https://api.slack.com/methods/chat.postMessage

api_call!(
    post_message,
    "chat.postMessage",
    PostMessageRequest =>
    PostMessageResponse
);

#[derive(Clone, Debug, Serialize, new)]
pub struct PostMessageRequest<'a> {
    /// Channel, private group, or IM channel to send message to. Can be an encoded ID, or a name. See below for more details.
    pub channel: ::ConversationId,
    /// Text of the message to send. See below for an explanation of formatting. This field is usually required, unless you're providing only attachments instead.
    pub text: &'a str,
    /// Change how messages are treated. Defaults to none. See below.
    #[new(default)]
    pub parse: ParseMode,
    /// Find and link channel names and usernames.
    #[new(default)]
    pub link_names: Option<bool>,
    /// Structured message attachments.
    #[new(default)]
    pub attachments: Option<&'a str>,
    /// Pass true to enable unfurling of primarily text-based content.
    #[new(default)]
    pub unfurl_links: Option<bool>,
    /// Pass false to disable unfurling of media content.
    #[new(default)]
    pub unfurl_media: Option<bool>,
    /// Set your bot's user name. Must be used in conjunction with as_user set to false, otherwise ignored. See authorship below.
    #[new(default)]
    pub username: Option<&'a str>,
    /// Pass true to post the message as the authed user, instead of as a bot. Defaults to false. See authorship below.
    #[new(default)]
    pub as_user: Option<bool>,
    /// URL to an image to use as the icon for this message. Must be used in conjunction with as_user set to false, otherwise ignored. See authorship below.
    #[new(default)]
    pub icon_url: Option<&'a str>,
    /// Emoji to use as the icon for this message. Overrides icon_url. Must be used in conjunction with as_user set to false, otherwise ignored. See authorship below.
    #[new(default)]
    pub icon_emoji: Option<&'a str>,
    /// Provide another message's ts value to make this message a reply. Avoid using a reply's ts value; use its parent instead.
    #[new(default)]
    pub thread_ts: Option<::Timestamp>,
    /// Used in conjunction with thread_ts and indicates whether reply should be made visible to everyone in the channel or conversation. Defaults to false.
    #[new(default)]
    pub reply_broadcast: Option<bool>,
    /// Disable Slack markup parsing by setting to false. Enabled by default.
    #[new(default)]
    pub mrkdwn: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub enum ParseMode {
    None,
    Full,
    Client,
}

impl Default for ParseMode {
    fn default() -> ParseMode {
        ParseMode::None
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostMessageResponse {
    ok: bool,
    pub channel: ::ConversationId,
    pub message: Message,
    pub ts: Timestamp,
}

/// Unfurl a URL that a user posted
///
/// Wraps https://api.slack.com/methods/chat.unfurl

api_call!(unfurl, "chat.unfurl", UnfurlRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct UnfurlRequest<'a> {
    /// Channel ID of the message
    pub channel: ::ConversationId,
    /// Timestamp of the message to add unfurl behavior to
    pub ts: Timestamp,
    /// JSON mapping a set of URLs from the message to their unfurl attachments
    pub unfurls: &'a str, // TODO: this should be a serialize_with on a Vec<String> I think?
    /// Set to true or 1 to indicate the user must install your Slack app to trigger unfurls for this domain
    #[new(default)]
    pub user_auth_required: Option<bool>,
    /// Provide a simply-formatted string to send as an ephemeral message to the user as invitation to authenticate further and enable full unfurling behavior
    #[new(default)]
    pub user_auth_message: Option<&'a str>,
    /// Send users to this custom URL where they will complete authentication in your app to fully trigger unfurling. Value should be properly URL-encoded.
    #[new(default)]
    pub user_auth_url: Option<&'a str>,
}

/// Updates a message.
///
/// Wraps https://api.slack.com/methods/chat.update

api_call!(update, "chat.update", UpdateRequest => UpdateResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct UpdateRequest<'a> {
    /// Timestamp of the message to be updated.
    pub ts: Timestamp,
    /// Channel containing the message to be updated.
    pub channel: ::ConversationId,
    /// New text for the message, using the default formatting rules.
    pub text: &'a str,
    /// Structured message attachments.
    #[new(default)]
    pub attachments: Option<&'a str>, // TODO: this should be a serialize_with on a Vec
    /// Change how messages are treated. Defaults to client, unlike chat.postMessage. See below.
    #[new(default)]
    pub parse: ParseMode,
    /// Find and link channel names and usernames. Defaults to none. This parameter should be used in conjunction with parse. To set link_names to 1, specify a parse mode of full.
    #[new(default)]
    pub link_names: Option<bool>,
    /// Pass true to update the message as the authed user. Bot users in this context are considered authed users.
    #[new(default)]
    pub as_user: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateResponse {
    ok: bool,
    pub channel: String,
    pub text: String,
    pub ts: String,
}
