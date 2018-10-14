/// Exchanges a temporary OAuth code for an API token.
///
/// Wraps https://api.slack.com/methods/oauth.access

api_call!(access, "oauth.access", AccessRequest => AccessResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct AccessRequest<'a> {
    /// Issued when you created your application.
    pub client_id: &'a str,
    /// Issued when you created your application.
    pub client_secret: &'a str,
    /// The code param returned via the OAuth callback.
    pub code: &'a str,
    /// This must match the originally submitted URI (if one was sent).
    #[new(default)]
    pub redirect_uri: Option<&'a str>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessResponse {
    pub access_token: Option<String>,
    pub scope: Option<String>,
}
