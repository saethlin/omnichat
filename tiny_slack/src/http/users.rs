//! Get info on members of your Slack team.
use crate::http::{Cursor, Paging};
use crate::id::*;
use crate::Timestamp;

/// Lists all users in a Slack team.
///
/// Wraps https://api.slack.com/methods/users.list

/// At this time, providing no limit value will result in Slack
/// attempting to deliver you the entire result set.
/// If the collection is too large you may experience HTTP 500 errors.
/// Resolve this scenario by using pagination.
///
/// One day pagination will become required to use this method.
#[derive(Serialize, new)]
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

#[derive(Deserialize)]
pub struct ListResponse {
    pub ok: bool,
    pub members: Vec<User>,
    pub cache_ts: Option<Timestamp>,
    pub response_metadata: Option<Paging>,
    pub is_limited: Option<bool>,
}

#[derive(Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub real_name: Option<String>,
}
