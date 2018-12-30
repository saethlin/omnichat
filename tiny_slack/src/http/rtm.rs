use crate::TeamId;

/// Starts a Real Time Messaging session.
///
/// Wraps https://api.slack.com/methods/rtm.connect

#[derive(Serialize, new)]
pub struct ConnectRequest {
    #[new(default)]
    batch_presence_aware: Option<bool>,
    #[new(default)]
    presence_sub: Option<bool>,
}

#[derive(Deserialize)]
pub struct ConnectResponse {
    pub ok: bool,
    #[serde(rename = "self")]
    pub slf: ConnectResponseSelf,
    pub team: ConnectResponseTeam,
    pub url: String,
}

#[derive(Deserialize)]
pub struct ConnectResponseSelf {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct ConnectResponseTeam {
    pub domain: String,
    pub enterprise_id: Option<String>,
    pub enterprise_name: Option<String>,
    pub id: TeamId,
    pub name: String,
}
