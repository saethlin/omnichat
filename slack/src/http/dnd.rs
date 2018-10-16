//! Adjust and view Do Not Disturb settings for team members.

use rtm::Team;

/// Ends the current user's Do Not Disturb session immediately.
///
/// Wraps https://api.slack.com/methods/dnd.endDnd

/// Ends the current user's snooze mode immediately.
///
/// Wraps https://api.slack.com/methods/dnd.endSnooze

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndSnoozeResponse {
    ok: bool,
    pub dnd_enabled: Option<bool>,
    pub next_dnd_end_ts: Option<f32>,
    pub next_dnd_start_ts: Option<f32>,
    pub snooze_enabled: Option<bool>,
}

/// Retrieves a user's current Do Not Disturb status.
///
/// Wraps https://api.slack.com/methods/dnd.info

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// User to fetch status for (defaults to current user)
    #[new(default)]
    pub user: Option<::UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub dnd_enabled: Option<bool>,
    pub next_dnd_end_ts: Option<f32>,
    pub next_dnd_start_ts: Option<f32>,
    pub snooze_enabled: Option<bool>,
    pub snooze_endtime: Option<f32>,
    pub snooze_remaining: Option<f32>,
}

/// Turns on Do Not Disturb mode for the current user, or changes its duration.
///
/// Wraps https://api.slack.com/methods/dnd.setSnooze

#[derive(Clone, Debug, Serialize, new)]
pub struct SetSnoozeRequest {
    /// Number of minutes, from now, to snooze until.
    pub num_minutes: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSnoozeResponse {
    ok: bool,
    pub snooze_enabled: Option<bool>,
    pub snooze_endtime: Option<f32>,
    pub snooze_remaining: Option<f32>,
}

/// Retrieves the Do Not Disturb status for users on a team.
///
/// Wraps https://api.slack.com/methods/dnd.teamInfo

#[derive(Clone, Debug, Serialize, new)]
pub struct TeamInfoRequest<'a> {
    /// Comma-separated list of users to fetch Do Not Disturb status for
    #[new(default)]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub users: &'a [::UserId],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamInfoResponse {
    ok: bool,
    pub team: Team,
}
