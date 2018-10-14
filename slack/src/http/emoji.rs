use std::collections::HashMap;

/// Lists custom emoji for a team.
///
/// Wraps https://api.slack.com/methods/emoji.list

api_call!(list, "emoji.list", => ListResponse);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub emoji: Option<HashMap<String, String>>,
    cache_ts: Option<::Timestamp>,
}
