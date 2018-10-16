/// Revokes a token.
///
/// Wraps https://api.slack.com/methods/auth.revoke

#[derive(Clone, Debug, Serialize, new)]
pub struct RevokeRequest {
    /// Setting this parameter to 1 triggers a testing mode where the specified token will not actually be revoked.
    #[new(default)]
    pub test: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeResponse {
    ok: bool,
    pub revoked: bool,
}

/// Checks authentication & identity.
///
/// Wraps https://api.slack.com/methods/auth.test

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestResponse {
    ok: bool,
    pub team: String,
    pub team_id: ::TeamId,
    pub url: String,
    pub user: String,
    pub user_id: ::UserId,
}
