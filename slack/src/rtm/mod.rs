use serde::de::{self, Deserialize, Deserializer, Visitor};
use std::collections::HashMap;

use id::*;
use timestamp::Timestamp;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelName {
    len: u8,
    buf: [u8; 22],
}

impl ::std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        for c in self.buf.iter().take(self.len as usize) {
            write!(f, "{}", *c as char)?;
        }
        Ok(())
    }
}

impl ::std::fmt::Debug for ChannelName {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "\"{}\"", self).map(|_| ())
    }
}

struct ChannelNameVisitor;

impl<'de> Visitor<'de> for ChannelNameVisitor {
    type Value = ChannelName;

    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        formatter.write_str("a 9-byte str")
    }

    fn visit_str<E>(self, value: &str) -> Result<ChannelName, E>
    where
        E: de::Error,
    {
        if value.len() < 22 {
            let mut ret = ChannelName::default();
            ret.len = value.len() as u8;
            ret.buf[..value.len()].copy_from_slice(value.as_bytes());
            Ok(ret)
        } else {
            Err(E::custom(format!(
                "Channel names must be shorter than 22 characters,found {:?}",
                value,
            )))
        }
    }
}

impl<'de> Deserialize<'de> for ChannelName {
    fn deserialize<D>(deserializer: D) -> Result<ChannelName, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ChannelNameVisitor)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bot {
    pub app_id: Option<AppId>,
    pub deleted: Option<bool>,
    pub icons: Option<BotIcons>,
    pub id: BotId,
    pub name: String,
    pub updated: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotIcons {
    pub image_36: Option<String>,
    pub image_48: Option<String>,
    pub image_72: Option<String>,
}

// TODO: Actually implement a type
pub type Conversation = Channel;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub accepted_user: Option<UserId>,
    pub created: Option<Timestamp>,
    pub creator: Option<UserId>,
    pub id: ChannelId,
    pub is_archived: Option<bool>,
    pub is_channel: Option<bool>,
    pub is_general: Option<bool>,
    pub is_member: Option<bool>,
    pub is_moved: Option<u32>,
    pub is_mpim: Option<bool>,
    pub is_org_shared: Option<bool>,
    pub is_pending_ext_shared: Option<bool>,
    pub is_private: Option<bool>,
    pub is_read_only: Option<bool>,
    pub is_shared: Option<bool>,
    pub last_read: Option<Timestamp>,
    pub latest: Option<Box<Event>>,
    pub members: Option<Vec<UserId>>,
    pub name: String,
    pub name_normalized: Option<String>,
    pub num_members: Option<u32>,
    pub previous_names: Option<Vec<String>>,
    pub priority: Option<u32>,
    pub purpose: Option<ChannelPurpose>,
    pub topic: Option<ChannelTopic>,
    pub unlinked: Option<u32>,
    pub unread_count: Option<u32>,
    pub unread_count_display: Option<u32>,
    pub is_starred: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelPurpose {
    pub creator: Option<String>,
    pub last_set: Option<Timestamp>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelTopic {
    pub creator: Option<String>,
    pub last_set: Option<Timestamp>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub channels: Option<Vec<String>>,
    pub comments_count: Option<u32>,
    pub created: Option<Timestamp>,
    pub display_as_bot: Option<bool>,
    pub edit_link: Option<String>,
    pub editable: Option<bool>,
    pub external_type: Option<String>,
    pub filetype: Option<String>,
    pub groups: Option<Vec<String>>,
    pub id: Option<String>,
    pub ims: Option<Vec<String>>,
    pub initial_comment: Option<FileComment>,
    pub is_external: Option<bool>,
    pub is_public: Option<bool>,
    pub is_starred: Option<bool>,
    pub lines: Option<u32>,
    pub lines_more: Option<u32>,
    pub mimetype: Option<String>,
    pub mode: Option<String>,
    pub name: Option<String>,
    pub num_stars: Option<u32>,
    pub permalink: Option<String>,
    pub permalink_public: Option<String>,
    pub pinned_to: Option<Vec<String>>,
    pub pretty_type: Option<String>,
    pub preview: Option<String>,
    pub preview_highlight: Option<String>,
    pub public_url_shared: Option<bool>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub size: Option<u64>,
    pub thumb_160: Option<String>,
    pub thumb_360: Option<String>,
    pub thumb_360_gif: Option<String>,
    pub thumb_360_h: Option<u32>,
    pub thumb_360_w: Option<u32>,
    pub thumb_480: Option<String>,
    pub thumb_480_h: Option<u32>,
    pub thumb_480_w: Option<u32>,
    pub thumb_480_gif: Option<String>,
    pub thumb_64: Option<String>,
    pub thumb_80: Option<String>,
    pub thumb_720: Option<String>,
    pub thumb_720_w: Option<u32>,
    pub thumb_720_h: Option<u32>,
    pub thumb_960: Option<String>,
    pub thumb_960_w: Option<u32>,
    pub thumb_960_h: Option<u32>,
    pub thumb_800: Option<String>,
    pub thumb_800_w: Option<u32>,
    pub thumb_800_h: Option<u32>,
    pub thumb_1024: Option<String>,
    pub thumb_1024_w: Option<u32>,
    pub thumb_1024_h: Option<u32>,
    pub timestamp: Option<Timestamp>,
    pub title: Option<String>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub user: Option<UserId>,
    pub username: Option<String>,
    pub deanimate_gif: Option<String>,
    pub image_exif_rotation: Option<u32>,
    pub thumb_video: Option<String>,
    pub thumb_pdf: Option<String>,
    pub thumb_pdf_h: Option<u32>,
    pub thumb_pdf_w: Option<u32>,
    pub preview_is_truncated: Option<bool>,
    pub original_h: Option<u32>,
    pub original_w: Option<u32>,
    pub editor: Option<UserId>,
    pub last_editor: Option<UserId>,
    pub state: Option<String>, // TODO Probably an enum
    pub updated: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileComment {
    pub comment: Option<String>,
    pub id: Option<String>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    pub timestamp: Option<Timestamp>,
    pub user: Option<UserId>,
    pub created: Option<Timestamp>,
    pub is_intro: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub created: Option<Timestamp>,
    pub creator: Option<String>,
    pub id: GroupId,
    pub is_archived: Option<bool>,
    pub is_group: Option<bool>,
    pub is_mpim: Option<bool>,
    pub is_open: Option<bool>,
    pub last_read: Option<Timestamp>,
    pub latest: Option<Message>,
    pub members: Option<Vec<String>>,
    pub name: String,
    pub name_normalized: String,
    pub purpose: Option<GroupPurpose>,
    pub topic: Option<GroupTopic>,
    pub unread_count: Option<u32>,
    pub unread_count_display: Option<u32>,
    pub last_set: Option<Timestamp>,
    pub priority: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupPurpose {
    pub creator: Option<String>,
    pub last_set: Option<Timestamp>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupTopic {
    pub creator: Option<String>,
    pub last_set: Option<Timestamp>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Im {
    pub created: Option<Timestamp>,
    pub id: DmId,
    pub is_im: Option<bool>,
    pub is_user_deleted: Option<bool>,
    pub is_org_shared: Option<bool>,
    pub priority: Option<f64>,
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type")]
pub enum Subscription {
    Thread {
        active: bool,
        channel: ConversationId,
        date_create: Timestamp,
        last_read: Timestamp,
        thread_ts: Timestamp,
    },
}

// TODO: Need to test this for more variants and capitalization
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum ChannelType {
    C,
    G,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortChannelDescription {
    pub id: ::ConversationId,
    pub is_channel: bool,
    pub name: String,
    pub name_normalized: String,
    pub created: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedInfo {
    pub channel: ConversationId,
    pub pinned_by: UserId,
    pub pinned_ts: Timestamp,
    pub ts: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub id: AppId,
    pub name: String,
    pub icons: Option<AppIcons>,
    pub deleted: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppIcons {
    pub image_32: Option<String>,
    pub image_36: Option<String>,
    pub image_48: Option<String>,
    pub image_64: Option<String>,
    pub image_72: Option<String>,
    pub image_96: Option<String>,
    pub image_128: Option<String>,
    pub image_192: Option<String>,
    pub image_512: Option<String>,
    pub image_1024: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DndStatus {
    pub dnd_enabled: bool,
    pub next_dnd_start_ts: Timestamp,
    pub next_dnd_end_ts: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JustAFileId {
    pub id: FileId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mpim {
    pub created: Option<Timestamp>,
    pub creator: Option<String>,
    pub id: Option<String>,
    pub is_group: Option<bool>,
    pub is_mpim: Option<bool>,
    pub last_read: Option<Timestamp>,
    pub latest: Option<Message>,
    pub members: Option<Vec<UserId>>,
    pub name: Option<String>,
    pub unread_count: Option<u32>,
    pub unread_count_display: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Cursor(String); // TODO: Type safety goes here

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paging {
    pub count: Option<u32>,
    pub page: Option<u32>,
    pub pages: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reaction {
    pub count: Option<u32>,
    pub name: String,
    pub users: Option<Vec<UserId>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reminder {
    pub complete_ts: Option<f32>,
    pub creator: Option<String>,
    pub id: Option<String>,
    pub recurring: Option<bool>,
    #[serde(default)]
    pub text: String,
    pub time: Option<f32>,
    pub user: Option<UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Team {
    pub domain: Option<String>,
    pub email_domain: Option<String>,
    pub icon: Option<TeamIcon>,
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamIcon {
    pub image_102: String,
    pub image_132: String,
    pub image_230: String,
    pub image_34: String,
    pub image_44: String,
    pub image_68: String,
    pub image_88: String,
    pub image_original: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadInfo {
    pub complete: Option<bool>,
    pub count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub color: Option<String>,
    pub deleted: Option<bool>,
    pub has_2fa: Option<bool>,
    pub id: UserId,
    pub is_admin: Option<bool>,
    pub is_app_user: Option<bool>,
    pub is_bot: Option<bool>,
    pub is_owner: Option<bool>,
    pub is_primary_owner: Option<bool>,
    pub is_restricted: Option<bool>,
    pub is_ultra_restricted: Option<bool>,
    pub locale: Option<String>,
    pub name: Option<String>,
    pub profile: Option<UserProfile>,
    pub real_name: Option<String>,
    pub team_id: Option<String>,
    pub two_factor_type: Option<String>,
    pub tz: Option<String>,
    pub tz_label: Option<String>,
    pub tz_offset: Option<f32>,
    pub updated: Option<Timestamp>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usergroup {
    pub auto_type: Option<String>,
    pub created_by: Option<String>,
    pub date_create: Option<Timestamp>,
    pub date_delete: Option<Timestamp>,
    pub date_update: Option<Timestamp>,
    pub deleted_by: Option<UserId>,
    pub description: Option<String>,
    pub handle: Option<String>,
    pub id: Option<GroupId>,
    pub is_external: Option<bool>,
    pub is_usergroup: Option<bool>,
    pub name: Option<String>,
    pub prefs: Option<UsergroupPrefs>,
    pub team_id: Option<TeamId>,
    pub updated_by: Option<UserId>,
    pub user_count: Option<String>, // TODO: What on Earth
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsergroupPrefs {
    pub channels: Option<Vec<String>>,
    pub groups: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserProfile {
    pub name: Option<String>,
    pub avatar_hash: Option<String>,
    pub display_name: Option<String>,
    pub display_name_normalized: Option<String>,
    pub email: Option<String>,
    #[serde(deserialize_with = "optional_struct_or_empty_array")]
    #[serde(default)]
    pub fields: Option<HashMap<String, UserProfileFields>>,
    pub first_name: Option<String>,
    pub guest_channels: Option<String>,
    pub image_192: Option<String>,
    pub image_24: Option<String>,
    pub image_32: Option<String>,
    pub image_48: Option<String>,
    pub image_72: Option<String>,
    pub image_512: Option<String>,
    pub image_1024: Option<String>,
    pub image_original: Option<String>,
    pub is_custom_image: Option<bool>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub real_name: Option<String>,
    pub real_name_normalized: Option<String>,
    pub skype: Option<String>,
    pub status_emoji: Option<String>,
    pub status_text: Option<String>,
    pub team: Option<TeamId>,
    pub title: Option<String>,
    pub status_expiration: Option<Timestamp>,
    pub status_text_canonical: Option<String>,
    pub is_restricted: Option<bool>,
    pub is_ultra_restricted: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserProfileFields {
    pub alt: Option<String>,
    pub label: Option<String>,
    pub value: Option<String>,
}

fn optional_struct_or_empty_array<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: ::serde::Deserialize<'de> + Default,
    D: ::serde::Deserializer<'de>,
{
    use serde::de;
    use std::marker::PhantomData;

    struct StructOrEmptyArray<T>(PhantomData<T>);

    impl<'de, T> de::Visitor<'de> for StructOrEmptyArray<T>
    where
        T: de::Deserialize<'de> + Default,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            formatter.write_str("struct or empty array")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Option<T>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            match seq.next_element::<T>()? {
                Some(_) => Err(de::Error::custom("non-empty array is not valid")),
                None => Ok(Some(T::default())),
            }
        }

        fn visit_unit<E>(self) -> Result<Option<T>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_map<M>(self, access: M) -> Result<Option<T>, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            T::deserialize(de::value::MapAccessDeserializer::new(access)).map(Some)
        }
    }

    deserializer.deserialize_any(StructOrEmptyArray(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::UserProfile;
    use serde_json;

    #[test]
    fn test_user_profile_fields_empty_array_deserialize() {
        let user_profile: UserProfile = serde_json::from_str(r#"{"fields": []}"#).unwrap();
        assert_eq!(0, user_profile.fields.unwrap().len());
    }

    #[test]
    fn test_user_profile_fields_empty_map_deserialize() {
        let user_profile: UserProfile = serde_json::from_str(r#"{"fields": {}}"#).unwrap();
        assert_eq!(0, user_profile.fields.unwrap().len());
    }

    #[test]
    fn test_user_profile_fields_nonempty_map_deserialize() {
        let user_profile: UserProfile =
            serde_json::from_str(r#"{"fields": {"some_field": {"alt": "foo", "label": "bar"}}}"#)
                .unwrap();
        assert_eq!(1, user_profile.fields.unwrap().len());
    }

    #[test]
    fn test_user_profile_fields_null_deserialize() {
        let user_profile: UserProfile = serde_json::from_str(r#"{"fields": null}"#).unwrap();
        assert!(user_profile.fields.is_none());
    }

    #[test]
    fn test_user_profile_fields_undefined_deserialize() {
        let user_profile: UserProfile = serde_json::from_str(r#"{}"#).unwrap();
        assert!(user_profile.fields.is_none());
    }
}

mod event;
pub use self::event::*;
mod message;
pub use self::message::*;
