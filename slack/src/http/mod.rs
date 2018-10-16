//! Functionality for sending requests to Slack.

#[derive(Clone, Debug, Deserialize)]
pub struct SlackError {
    pub ok: bool,
    pub error: Option<String>,
}

pub mod api;
pub mod auth;
pub mod bots;
pub mod channels;
pub mod chat;
pub mod conversations;
pub mod dnd;
pub mod emoji;
pub mod files;
pub mod groups;
pub mod im;
pub mod mpim;
pub mod oauth;
pub mod pins;
pub mod reactions;
pub mod reminders;
pub mod rtm;
pub mod search;
pub mod stars;
pub mod team;
pub mod team_profile;
pub mod usergroups;
pub mod usergroups_users;
pub mod users;
pub mod users_profile;
