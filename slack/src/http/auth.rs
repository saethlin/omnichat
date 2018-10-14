/// Revokes a token.
///
/// Wraps https://api.slack.com/methods/auth.revoke

api_call!(revoke, "auth.revoke", RevokeRequest => RevokeResponse);

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

api_call!(test, "auth.test", => TestResponse);

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

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static! {
        pub static ref CLIENT: ::reqwest::Client = ::reqwest::Client::new();
        pub static ref TOKEN: String = ::std::env::var("SLACK_API_TOKEN").unwrap();
    }

    #[test]
    fn test_revoke() {
        let req = RevokeRequest { test: Some(true) };
        assert_eq!(revoke(&*CLIENT, &TOKEN, &req).unwrap().revoked, false);
    }

    #[test]
    fn test_test() {
        test(&*CLIENT, &TOKEN).unwrap();
    }
}
