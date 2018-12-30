use std::collections::HashMap;

/// Lists custom emoji for a team.
///
/// Wraps https://api.slack.com/methods/emoji.list

#[derive(Deserialize)]
pub struct ListResponse {
    pub ok: bool,
    pub emoji: Option<HashMap<String, String>>,
}
