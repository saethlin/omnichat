//! Get info on members of your Slack team.

use id::*;
use rtm::Cursor;
use rtm::{Paging, Team};
use timestamp::Timestamp;

/// Delete the user profile photo
///
/// Wraps https://api.slack.com/methods/users.deletePhoto

api_call!(delete_photo, "users.deletePhoto");

/// Gets user presence information.
///
/// Wraps https://api.slack.com/methods/users.getPresence

api_call!(
    get_presence,
    "users.getPresence",
    GetPresenceRequest =>
    GetPresenceResponse
);

#[derive(Clone, Debug, Serialize, new)]
pub struct GetPresenceRequest {
    /// User to get presence info on. Defaults to the authed user.
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetPresenceResponse {
    ok: bool,
    pub presence: Option<String>,
}

/// Get a user's identity.
///
/// Wraps https://api.slack.com/methods/users.identity

api_call!(identity, "users.identity", => IdentityResponse);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityResponse {
    ok: bool,
    pub team: Option<Team>,
    pub user: Option<User>,
}

/// Gets information about a user.
///
/// Wraps https://api.slack.com/methods/users.info

api_call!(info, "users.info", InfoRequest => InfoResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// User to get info on
    pub user: UserId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub user: Option<User>,
}

/// Lists all users in a Slack team.
///
/// Wraps https://api.slack.com/methods/users.list

api_call!(list, "users.list", ListRequest => ListResponse);

/// At this time, providing no limit value will result in Slack
/// attempting to deliver you the entire result set.
/// If the collection is too large you may experience HTTP 500 errors.
/// Resolve this scenario by using pagination.
///
/// One day pagination will become required to use this method.
#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Whether to include presence data in the output
    #[new(default)]
    pub presence: Option<bool>,
    #[new(default)]
    pub cursor: Option<Cursor>,
    #[new(default)]
    pub limit: Option<usize>,
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub members: Vec<User>,
    pub cache_ts: Option<Timestamp>,
    pub response_metadata: Option<Paging>,
    pub is_limited: Option<bool>,
}

/// Gets a users's preferences
///
/// Wraps https://api.slack.com/methods/users.prefs.get

api_call!(prefs_get, "users.prefs.get", => PrefsResponse);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefsResponse {
    ok: bool,
    pub prefs: UserPrefs,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPrefs {
    muted_channels: Vec<ChannelId>,
}

/// Marks a user as active.
///
/// Wraps https://api.slack.com/methods/users.setActive

api_call!(set_active, "users.setActive");

/// Manually sets user presence.
///
/// Wraps https://api.slack.com/methods/users.setPresence

api_call!(set_presence, "users.setPresence", SetPresenceRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct SetPresenceRequest {
    /// Either auto or away
    pub presence: Presence,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub enum Presence {
    Auto,
    Away,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub color: Option<String>,
    pub deleted: bool,
    pub has_2fa: Option<bool>,
    pub two_factor_type: Option<String>,
    pub id: UserId,
    pub is_admin: Option<bool>,
    pub is_app_user: bool,
    pub is_bot: bool,
    pub is_owner: Option<bool>,
    pub is_primary_owner: Option<bool>,
    pub is_restricted: Option<bool>,
    pub is_ultra_restricted: Option<bool>,
    pub name: String,
    pub profile: UserProfile,
    pub real_name: Option<String>,
    pub team_id: TeamId,
    pub tz: Option<String>, // TODO: Might be an enum
    pub tz_label: Option<String>,
    pub tz_offset: Option<i64>,
    pub updated: Timestamp,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserProfile {
    pub always_active: Option<bool>,
    pub bot_id: Option<BotId>,
    pub api_app_id: Option<String>,
    pub avatar_hash: String, // TOOD: static length
    pub display_name: String,
    pub display_name_normalized: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub image_1024: Option<String>,
    pub image_192: String,
    pub image_24: String,
    pub image_32: String,
    pub image_48: String,
    pub image_512: String,
    pub image_72: String,
    pub image_original: Option<String>,
    pub is_custom_image: Option<bool>,
    pub last_name: Option<String>,
    pub phone: String,
    pub real_name: String,
    pub real_name_normalized: String,
    pub skype: String,
    pub status_emoji: String,
    pub status_expiration: i64,
    pub status_text: String,
    pub status_text_canonical: String,
    pub team: TeamId,
    pub title: String,
    pub fields: Option<()>, // No idea what goes here
}
