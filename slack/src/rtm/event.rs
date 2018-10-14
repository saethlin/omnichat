use id::*;
use rtm::{
    App, Bot, Channel, ChannelType, Command, DndStatus, JustAFileId, Message, PinnedInfo,
    Subscription, TeamIcon, User,
};
use timestamp::Timestamp;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
//#[serde(deny_unknown_fields)]
pub enum Event {
    AppsChanged {
        app: App,
        event_ts: Timestamp,
    },
    AppsInstalled {
        app: App,
        event_ts: Timestamp,
    },
    AccountsChanged {
        events_ts: Timestamp,
    },
    BotAdded {
        bot: Bot,
        cache_ts: Timestamp,
        event_ts: Timestamp,
    },
    BotChanged {
        bot: Bot,
        cache_ts: Option<Timestamp>,
        event_ts: Timestamp,
    },
    ChannelJoined {
        channel: Channel,
    },
    ChannelLeft {
        actor_id: UserId,
        channel: ChannelId,
        events_ts: Timestamp,
    },
    ChannelMarked {
        channel: ChannelId,
        ts: Timestamp,
        unread_count: u32,
        unread_count_display: u32,
        num_mentions: u32,
        num_mentions_display: u32,
        mention_count: u32,
        mention_count_display: u32,
        event_ts: Timestamp,
    },
    ChannelRename {},
    CommandsChanged {
        // new in refactor
        commands_removed: Vec<Command>,
        commands_updated: Vec<Command>,
        event_ts: Timestamp,
    },
    #[allow(non_snake_case)]
    DesktopNotification {
        title: String,
        subtitle: String,
        msg: Timestamp,
        ts: Timestamp,
        content: String,
        channel: ConversationId,
        launchUri: String,
        avatarImage: String,
        ssbFilename: String,
        imageUri: Option<String>,
        is_shared: bool,
        event_ts: Timestamp,
    },
    DndUpdatedUser {
        user: UserId,
        dnd_status: DndStatus,
        event_ts: Timestamp,
    },
    EmojiChanged(EmojiChanged),
    FileChange {
        file_id: FileId,
        user_id: UserId,
        file: JustAFileId,
        event_ts: Timestamp,
    },
    FileCreated {
        file: JustAFileId,
        file_id: FileId,
        user_id: UserId,
        event_ts: Timestamp,
        ts: Option<Timestamp>,
    },
    FileDeleted {}, // TODO: new in refactor
    FilePublic {
        file_id: FileId,
        user_id: UserId,
        file: JustAFileId,
        event_ts: Timestamp,
        ts: Option<Timestamp>,
    },
    FileShared {
        file_id: FileId,
        user_id: UserId,
        channel_id: ConversationId,
        file: JustAFileId,
        event_ts: Timestamp,
        ts: Option<Timestamp>,
    },
    FileUnshared {
        channel_id: ConversationId,
        event_ts: Timestamp,
        file: JustAFileId,
        ts: Timestamp,
        user_id: UserId,
    },
    GroupClose {
        channel: GroupId,
        user: UserId,
        event_ts: Timestamp,
        is_mpim: bool,
    },
    GroupMarked {
        channel: GroupId,
        ts: Timestamp,
        unread_count: u32,
        unread_count_display: u32,
        num_mentions: u32,
        num_mentions_display: u32,
        mention_count: u32,
        mention_count_display: u32,
        event_ts: Timestamp,
        is_mpim: Option<bool>,
    },
    GroupOpen {
        channel: GroupId,
        user: UserId,
        event_ts: Timestamp,
        is_mpim: bool,
    },
    Hello {},
    ImClose {
        channel: DmId,
        user: UserId,
        event_ts: Timestamp,
    },
    ImCreated {
        channel: Channel,
        event_ts: Timestamp,
        user: UserId,
    },
    ImMarked {
        channel: DmId,
        ts: Timestamp,
        dm_count: u32,
        unread_count_display: u32,
        num_mentions_display: u32,
        mention_count_display: Option<u32>,
        event_ts: Timestamp,
    },
    ImOpen {
        channel: DmId,
        user: UserId,
        event_ts: Timestamp,
    },
    MpimClose {
        channel: GroupId,
        user: UserId,
        event_ts: Timestamp,
        is_mpim: bool,
    },
    MpimOpen {
        channel: GroupId,
        user: UserId,
        event_ts: Timestamp,
        is_mpim: bool,
    },
    MemberJoinedChannel {
        inviter: Option<UserId>,
        user: UserId,
        channel: ConversationId,
        channel_type: ChannelType,
        team: TeamId,
        event_ts: Timestamp,
        ts: Timestamp,
    },
    MemberLeftChannel {
        user: UserId,
        channel: ConversationId,
        channel_type: ChannelType,
        team: TeamId,
        ts: Timestamp,
        event_ts: Timestamp,
    },
    Message {
        #[serde(flatten)]
        message: Message,
        event_ts: Timestamp,
    },
    PinAdded {
        user: UserId,
        channel_id: ConversationId,
        item: Message,
        item_user: UserId,
        pin_count: u32,
        pinned_info: PinnedInfo,
        event_ts: Timestamp,
        ts: Option<Timestamp>,
    },
    PinRemoved {
        channel_id: ChannelId,
        event_ts: Timestamp,
        has_pins: bool,
        item: Message,
        pin_count: u32,
        pinned_info: PinnedInfo,
        ts: Timestamp,
        user: UserId,
    },
    PrefChange {
        name: String,
        value: ::serde_json::Value,
        event_ts: Timestamp,
    },
    ReactionAdded {
        user: UserId,
        item: Reactable,
        reaction: String,
        item_user: Option<UserId>,
        event_ts: Timestamp,
        ts: Timestamp,
    },
    ReactionRemoved {
        user: UserId,
        item: Reactable,
        reaction: String,
        item_user: Option<UserId>,
        event_ts: Timestamp,
        ts: Timestamp,
    },
    StarAdded {
        item: Message,
        user: UserId,
        event_ts: Timestamp,
    },
    StarRemoved {
        item: Message,
        user: UserId,
        event_ts: Timestamp,
    },
    TeamJoin {
        user: User,
        cache_ts: Timestamp,
        event_ts: Timestamp,
    },
    TeamIconChange {
        event_ts: Timestamp,
        icon: TeamIcon,
    },
    TeamPrefChange {
        event_ts: Timestamp,
        name: String,
        value: String,
    },
    ThreadSubscribed {
        event_ts: Timestamp,
        subscription: Subscription,
    },
    UpdateThreadState {
        event_ts: Timestamp,
        has_unreads: bool,
        mention_count: u32,
        mention_count_by_channel: Vec<u32>,
        timestamp: Timestamp,
    },
    UserChange {
        user: User,
        cache_ts: Timestamp,
        event_ts: Timestamp,
    },
    UserTyping {
        channel: ConversationId,
        user: UserId,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "subtype")]
#[serde(deny_unknown_fields)]
pub enum EmojiChanged {
    Add {
        name: String,
        value: String,
        event_ts: Timestamp,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
pub enum Reactable {
    Message {
        channel: ConversationId,
        ts: Timestamp,
    },
}
