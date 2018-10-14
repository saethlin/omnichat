use rtm::{Bot, Channel, Group, Im, Mpim, Team, User};
use timestamp::Timestamp;

/// Starts a Real Time Messaging session.
///
/// Wraps https://api.slack.com/methods/rtm.connect

api_call!(connect, "rtm.connect", ConnectRequest => ConnectResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ConnectRequest {
    #[new(default)]
    batch_presence_aware: Option<bool>,
    #[new(default)]
    presence_sub: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectResponse {
    ok: bool,
    #[serde(rename = "self")]
    pub slf: ConnectResponseSelf,
    pub team: ConnectResponseTeam,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectResponseSelf {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectResponseTeam {
    pub domain: String,
    pub enterprise_id: Option<String>,
    pub enterprise_name: Option<String>,
    pub id: ::TeamId,
    pub name: String,
}

/// Starts a Real Time Messaging session.
///
/// Wraps https://api.slack.com/methods/rtm.start

api_call!(start, "rtm.start", StartRequest => StartResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct StartRequest {
    /// Skip unread counts for each channel (improves performance).
    #[new(default)]
    pub no_unreads: Option<bool>,
    /// Returns MPIMs to the client in the API response.
    #[new(default)]
    pub mpim_aware: Option<bool>,
    /// Exclude latest timestamps for channels, groups, mpims, and ims. Automatically sets no_unreads to 1
    #[new(default)]
    pub no_latest: Option<bool>,
    /// Only deliver presence events when requested by subscription. See [presence subscriptions](/docs/presence-and-status#subscriptions).
    #[new(default)]
    pub batch_presence_aware: Option<bool>,
    /// Set this to `true` to receive the locale for users and channels. Defaults to `false`
    #[new(default)]
    pub include_locale: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartResponse {
    ok: bool,
    pub bots: Option<Vec<Bot>>,
    pub channels: Option<Vec<Channel>>,
    pub groups: Option<Vec<Group>>,
    pub ims: Option<Vec<Im>>,
    pub mpims: Option<Vec<Mpim>>,
    #[serde(rename = "self")]
    pub slf: Option<User>,
    pub team: Option<Team>,
    pub url: Option<String>,
    pub users: Option<Vec<User>>,
    pub latest_event_ts: Option<Timestamp>,
    pub cache_ts: Option<Timestamp>,
    pub read_only_channels: Option<Vec<Channel>>,
    pub non_threadable_channels: Option<Vec<Channel>>,
    pub thread_only_channels: Option<Vec<Channel>>,
    pub can_manage_shared_channels: Option<bool>,
    pub cache_version: Option<String>,
    pub cache_ts_version: Option<String>,
    pub dnd: Option<Dnd>,
    pub subteams: Option<Subteams>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dnd {
    pub dnd_enabled: bool,
    pub next_dnd_end_ts: Timestamp,
    pub next_dnd_start_ts: Timestamp,
    pub snooze_enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subteams {
    pub all: Option<Vec<Team>>,
    #[serde(rename = "self")]
    pub slf: Option<Vec<String>>,
}
