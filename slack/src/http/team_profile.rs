use std::collections::HashMap;

/// Retrieve a team's profile.
///
/// Wraps https://api.slack.com/methods/team.profile.get

api_call!(get, "team.profile.get", GetRequest => GetResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct GetRequest {
    /// Filter by visibility.
    #[new(default)]
    pub visibility: Option<Visibility>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub enum Visibility {
    All,
    Visible,
    Hidden,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResponse {
    ok: bool,
    pub profile: Option<GetResponseProfile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResponseProfile {
    pub fields: Option<Vec<GetResponseProfileField>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetResponseProfileField {
    pub hint: Option<String>,
    pub id: Option<String>,
    pub is_hidden: Option<bool>,
    pub label: Option<String>,
    pub options: Option<HashMap<String, String>>,
    pub ordering: Option<i32>,
    pub possible_values: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
}
