use id::*;
use rtm::{File, FileComment, PinnedInfo, Reaction, UserProfile};
use timestamp::Timestamp;
use uuid::Uuid;

macro_rules! deserialize_internally_tagged {
    {
        tag_field = $tagfield:expr,
        default_variant = $default_variant:ident,
        default_struct = $default_struct:ty,
        $(#[$attr:meta])*
        pub enum $enumname:ident {
            $($variant_name:ident($struct_name:ty)),*,
        }
   } => {

        $(#[$attr])*
        pub enum $enumname {
            $($variant_name($struct_name),)*
        }

        impl<'de> ::serde::Deserialize<'de> for $enumname {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                D: ::serde::Deserializer<'de>,
            {
                let mut v: ::serde_json::Value = ::serde::Deserialize::deserialize(deserializer)?;

                #[derive(Deserialize)]
                #[serde(field_identifier, rename_all = "snake_case")]
                enum Tag {
                    $($variant_name,)*
                }

                v.as_object_mut().unwrap().remove("type"); // TODO: hack???
                v.as_object_mut().unwrap().remove("msg_subtype"); // TODO: hack???

                let maybe_tag = v
                    .as_object_mut()
                    .ok_or(::serde::de::Error::custom("Must be an object"))?
                    .remove($tagfield);

                match maybe_tag {
                    None => {
                        ::serde::Deserialize::deserialize(v)
                            .map($enumname::$default_variant)
                            .map_err(|e| ::serde::de::Error::custom(format!("{} while deserializing {}", e, stringify!($default_struct))))
                    }
                    Some(tag) => {
                        match ::serde::Deserialize::deserialize(tag).map_err(::serde::de::Error::custom)? {
                            $(
                            Tag::$variant_name => {
                                ::serde::Deserialize::deserialize(v)
                                .map($enumname::$variant_name)
                                .map_err(|e| ::serde::de::Error::custom(format!("{} while deserializing {}", e, stringify!($struct_name))))
                            }
                            )*
                        }
                    }
                }
            }
        }
    };
}

deserialize_internally_tagged! {
    tag_field = "subtype",
    default_variant = Standard,
    default_struct = MessageStandard,
    #[derive(Clone, Debug)]
    pub enum Message {
        Standard(MessageStandard),
        BotAdd(MessageBotAdd),
        BotRemove(MessageBotRemove),
        BotMessage(MessageBotMessage),
        ChannelArchive(MessageChannelArchive),
        ChannelJoin(MessageChannelJoin),
        ChannelLeave(MessageChannelLeave),
        ChannelName(MessageChannelName),
        ChannelPurpose(MessageChannelPurpose),
        ChannelTopic(MessageChannelTopic),
        ChannelUnarchive(MessageChannelUnarchive),
        FileComment(Box<MessageFileComment>),
        FileMention(Box<MessageFileMention>),
        FileShare(Box<MessageFileShare>),
        GroupArchive(MessageGroupArchive),
        GroupJoin(MessageGroupJoin),
        GroupLeave(MessageGroupLeave),
        GroupName(MessageGroupName),
        GroupPurpose(MessageGroupPurpose),
        GroupTopic(MessageGroupTopic),
        GroupUnarchive(MessageGroupUnarchive),
        MeMessage(MessageMeMessage),
        MessageChanged(MessageMessageChanged),
        MessageDeleted(MessageMessageDeleted),
        MessageReplied(MessageMessageReplied),
        PinnedItem(MessagePinnedItem),
        ReplyBroadcast(MessageReplyBroadcast),
        ReminderAdd(MessageReminderAdd),
        SlackbotResponse(MessageSlackbotResponse),
        ShRoomCreated(MessageShRoomCreated),
        ThreadBroadcast(Box<MessageThreadBroadcast>),
        Tombstone(MessageTombstone),
        UnpinnedItem(MessageUnpinnedItem),
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShRoom {
    pub channels: Vec<ConversationId>,
    pub created_by: UserId,
    pub date_end: Timestamp,
    pub date_start: Timestamp,
    pub has_ended: bool,
    pub id: String,
    pub is_dm_call: bool,
    pub name: String,
    pub participant_history: Vec<UserId>,
    pub participants: Vec<UserId>,
    pub participants_camera_off: Vec<UserId>,
    pub participants_camera_on: Vec<UserId>,
    pub participants_screenshare_off: Vec<UserId>,
    pub participants_screenshare_on: Vec<UserId>,
    pub was_accepted: bool,
    pub was_missed: bool,
    pub was_rejected: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageShRoomCreated {
    pub channel: ConversationId,
    pub room: ShRoom,
    pub user: UserId,
    pub permalink: String,
    pub text: String,
    pub ts: Timestamp,
    pub no_notifications: bool,
}

//TODO: Have only seen this once...
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageTombstone {
    pub edited: Box<Message>,
    pub hidden: bool,
    pub replies: Vec<Message>,
    pub reply_count: Option<u32>,
    pub subscribed: bool,
    pub text: String,
    pub user: UserId,
    pub unread_count: Option<u32>,
    pub thread_ts: Timestamp,
    pub ts: Timestamp,
    // It looks like tombstone messages are actually events even though they end up coming through
    // a conversations.history call
    #[serde(rename = "type")]
    pub ty: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelMarked {
    pub channel: ConversationId,
    pub ts: Option<Timestamp>,
    pub unread_count: Option<u32>,
    pub unread_count_display: Option<u32>,
    pub num_mentions: Option<u32>,
    pub num_mentions_display: Option<u32>,
    pub mention_count: Option<u32>,
    pub event_ts: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageBotAdd {
    pub bot_id: Option<BotId>,
    pub bot_link: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageBotRemove {
    pub bot_id: Option<BotId>,
    pub bot_link: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageBotMessage {
    pub bot_id: Option<BotId>,
    pub icons: Option<MessageBotMessageIcons>,
    pub text: Option<String>,
    pub ts: Option<Timestamp>,
    pub username: Option<String>,
    pub channel: Option<ConversationId>,
    pub team: Option<TeamId>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub attachments: Option<Vec<Attachment>>,
    pub user: Option<UserId>,
    pub replies: Option<Vec<MessageReply>>,
    pub pinned_info: Option<PinnedInfo>,
    pub reply_count: Option<u32>,
    pub pinned_to: Option<Vec<ConversationId>>,
    pub subscribed: Option<bool>,
    pub thread_ts: Option<Timestamp>,
    pub unread_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageBotMessageIcons {
    pub image_36: Option<String>,
    pub image_48: Option<String>,
    pub image_72: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelArchive {
    pub members: Option<Vec<UserId>>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelJoin {
    pub channel: Option<ConversationId>, // Not present when deserializing from history
    pub team: Option<TeamId>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub user_profile: Option<UserProfile>,
    pub reactions: Option<Vec<Reaction>>,
    pub inviter: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelLeave {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelName {
    pub name: Option<String>,
    pub old_name: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelPurpose {
    pub purpose: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelTopic {
    pub channel: Option<ConversationId>,
    pub team: Option<TeamId>,
    #[serde(default)]
    pub text: String,
    pub topic: Option<String>,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub user_profie: Option<UserProfile>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageChannelUnarchive {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageFileComment {
    pub comment: Option<FileComment>,
    pub file: Option<File>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub files: Option<Vec<File>>,
    pub is_intro: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageFileMention {
    pub file: Option<File>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageFileShare {
    pub channel: Option<ConversationId>,
    pub file: Option<File>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub upload: Option<bool>,
    pub user: Option<UserId>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupArchive {
    pub members: Option<Vec<UserId>>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupJoin {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub inviter: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupLeave {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupName {
    pub name: Option<String>,
    pub old_name: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupPurpose {
    pub purpose: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupTopic {
    #[serde(default)]
    pub text: String,
    pub topic: Option<String>,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageGroupUnarchive {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMeMessage {
    pub channel: Option<ConversationId>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub reactions: Option<Vec<Reaction>>,
    pub reply_count: Option<u32>,
    pub subscribed: Option<bool>,
    pub unread_count: Option<u32>,
    pub replies: Option<Vec<MessageReply>>,
    pub thread_ts: Option<Timestamp>,
    pub edited: Option<EditInfo>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditInfo {
    ts: Timestamp,
    user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageReply {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChanged {
    pub channel: ConversationId,
    pub event_ts: Option<Timestamp>,
    pub hidden: Option<bool>,
    pub message: Option<Box<Message>>,
    pub previous_message: Option<Box<Message>>,
    pub ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedMessage {
    pub bot_id: Option<BotId>,
    pub edited: Option<MessageMessageChangedMessageEdited>,
    pub last_read: Option<String>,
    pub parent_user_id: Option<UserId>,
    pub replies: Option<Vec<MessageMessageChangedMessageReply>>,
    pub reply_count: Option<u32>,
    pub subscribed: Option<bool>,
    #[serde(default)]
    pub text: String,
    pub thread_ts: Option<Timestamp>,
    pub ts: Timestamp,
    pub unread_count: Option<u32>,
    pub user: Option<UserId>,
    pub client_msg_id: Option<Uuid>,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedMessageEdited {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedMessageReply {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedPreviousMessage {
    pub bot_id: Option<BotId>,
    pub edited: Option<MessageMessageChangedPreviousMessageEdited>,
    pub last_read: Option<Timestamp>,
    pub parent_user_id: Option<UserId>,
    pub replies: Option<Vec<MessageMessageChangedPreviousMessageReply>>,
    pub reply_count: Option<u32>,
    pub subscribed: Option<bool>,
    #[serde(default)]
    pub text: String,
    pub thread_ts: Option<Timestamp>,
    pub ts: Timestamp,
    pub unread_count: Option<u32>,
    pub user: Option<UserId>,
    pub client_msg_id: Option<Uuid>,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedPreviousMessageEdited {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageChangedPreviousMessageReply {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageDeleted {
    pub channel: Option<String>,
    pub deleted_ts: Option<String>,
    pub event_ts: Option<String>,
    pub hidden: Option<bool>,
    pub previous_message: Option<Box<Message>>,
    pub ts: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageMessageReplied {
    pub channel: Option<ConversationId>,
    pub event_ts: Option<Timestamp>,
    pub hidden: Option<bool>,
    pub message: Option<Box<Message>>,
    pub thread_ts: Option<Timestamp>,
    pub ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagePinnedItem {
    pub channel: Option<ConversationId>,
    pub item: Option<MessagePinnedItemItem>,
    pub item_type: Option<String>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagePinnedItemItem {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageReminderAdd {
    pub message: Option<String>,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub channel: Option<ConversationId>,
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageReplyBroadcast {
    pub attachments: Option<Vec<Attachment>>,
    pub channel: Option<ConversationId>,
    pub event_ts: Option<Timestamp>,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageStandard {
    pub attachments: Option<Vec<Attachment>>,
    pub bot_id: Option<BotId>,
    pub channel: Option<ConversationId>,
    pub edited: Option<MessageStandardEdited>,
    pub event_ts: Option<Timestamp>,
    pub reply_broadcast: Option<bool>,
    pub source_team: Option<TeamId>,
    pub team: Option<TeamId>,
    #[serde(default)]
    pub text: String,
    pub thread_ts: Option<Timestamp>,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub client_msg_id: Option<Uuid>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub parent_user_id: Option<UserId>,
    pub replies: Option<Vec<MessageStandardReply>>,
    pub reply_count: Option<u32>,
    pub last_read: Option<Timestamp>,
    pub subscribed: Option<bool>,
    pub pinned_info: Option<PinnedInfo>,
    pub unread_count: Option<u32>,
    pub pinned_to: Option<Vec<String>>,
    pub is_starred: Option<bool>,
    pub display_as_bot: Option<bool>,
    // TODO: These fields should belong to a flattened struct
    pub files: Option<Vec<File>>,
    pub upload: Option<bool>,
    pub upload_reply_to: Option<Uuid>,
    pub x_files: Option<Vec<FileId>>,
    pub user_profile: Option<UserProfile>,
    pub user_team: Option<TeamId>,
    // TODO: this is only present when deserializing a history message,
    // eventually there should probably be a HistoryMessage struct that handles this more
    // gracefully
    #[serde(rename = "type")]
    ty: Option<String>,
}

// TODO: need to add the fields necessary here
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageStandardReply {
    pub ts: Timestamp,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    // TODO: This feels like an untagged enum...
    pub author_icon: Option<String>,
    pub author_link: Option<String>,
    pub author_name: Option<String>,
    pub author_id: Option<UserId>,
    pub color: Option<String>,
    pub fallback: Option<String>,
    pub fields: Option<Vec<MessageStandardAttachmentField>>,
    pub footer: Option<String>,
    pub footer_icon: Option<String>,
    pub image_url: Option<String>,
    pub pretext: Option<String>,
    #[serde(default)]
    pub text: String,
    pub title: Option<String>,
    pub title_link: Option<String>,
    pub ts: Option<Timestamp>,
    pub author_subname: Option<String>,
    pub from_url: Option<String>,
    pub id: Option<i64>,
    pub actions: Option<Vec<Action>>,
    pub mrkdwn_in: Option<Vec<String>>,
    pub original_url: Option<String>,
    pub image_bytes: Option<u64>,
    pub service_icon: Option<String>,
    pub service_name: Option<String>,
    pub service_url: Option<String>,
    pub thumb_height: Option<u32>,
    pub thumb_width: Option<u32>,
    pub thumb_url: Option<String>,
    pub channel_id: Option<ConversationId>,
    pub callback_id: Option<String>,
    pub image_height: Option<u32>,
    pub image_width: Option<u32>,
    pub channel_name: Option<String>,
    pub is_msg_unfurl: Option<bool>,
    pub video_html: Option<String>,
    pub video_html_height: Option<u32>,
    pub video_html_width: Option<u32>,
    pub is_animated: Option<bool>,
    pub is_share: Option<bool>,
    pub audio_html: Option<String>,
    pub audio_html_height: Option<u32>,
    pub audio_html_width: Option<u32>,
    pub app_unfurl_url: Option<String>,
    pub files: Option<Vec<File>>,
    pub video_url: Option<String>,
    pub indent: Option<bool>,
    pub bot_id: Option<BotId>,
    pub is_app_unfurl: Option<bool>,
    pub msg_subtype: Option<String>, // TODO: no idea what to do with this
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum Action {
    Button {
        id: String,
        name: Option<String>,
        style: String,
        text: String,
        url: Option<String>,
        value: Option<String>,
        confirm: Option<Confirmation>,
    },
    Select {
        data_source: String,
        id: String,
        name: String,
        options: Vec<ActionOption>,
        text: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confirmation {
    dismiss_text: String,
    ok_text: String,
    text: String,
    title: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOption {
    text: String,
    value: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageStandardAttachmentField {
    pub short: Option<bool>,
    pub title: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageStandardEdited {
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageUnpinnedItem {
    pub channel: Option<ConversationId>,
    pub item: Option<MessageUnpinnedItemItem>,
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageUnpinnedItemItem {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageSlackbotResponse {
    #[serde(default)]
    pub text: String,
    pub ts: Option<Timestamp>,
    pub user: Option<UserId>,
    pub channel: Option<ConversationId>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub attachments: Option<Vec<Attachment>>,
    pub source_team: Option<TeamId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageThreadBroadcast {
    pub attachments: Option<Vec<Attachment>>,
    pub root: Option<MessageStandard>,
    #[serde(default)]
    pub text: String,
    pub thread_ts: Option<String>,
    pub user: Option<UserId>,
    pub ts: Option<Timestamp>,
    pub client_msg_id: Option<Uuid>,
    // TODO: What the fuck
    pub is_thread_broadcast: Option<bool>,
    pub unfurl_links: Option<bool>,
    pub unfurl_media: Option<bool>,
    pub reactions: Option<Vec<Reaction>>,
    pub edited: Option<MessageStandardEdited>,
}
