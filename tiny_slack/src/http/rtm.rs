use crate::TeamId;

/// Starts a Real Time Messaging session.
///
/// Wraps https://api.slack.com/methods/rtm.connect

#[derive(Clone, Debug, Serialize, new)]
pub struct ConnectRequest {
    #[new(default)]
    batch_presence_aware: Option<bool>,
    #[new(default)]
    presence_sub: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectResponse {
    ok: bool,
    #[serde(rename = "self")]
    pub slf: ConnectResponseSelf,
    pub team: ConnectResponseTeam,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectResponseSelf {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectResponseTeam {
    pub domain: String,
    pub enterprise_id: Option<String>,
    pub enterprise_name: Option<String>,
    pub id: TeamId,
    pub name: String,
}
