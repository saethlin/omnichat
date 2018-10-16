/// Gets information about a bot user.
///
/// Wraps https://api.slack.com/methods/bots.info

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// Bot user to get info on
    #[new(default)]
    pub bot: Option<::BotId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub bot: Option<InfoResponseBot>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponseBot {
    pub app_id: ::AppId,
    pub deleted: bool,
    pub icons: InfoResponseBotIcons,
    pub id: ::BotId,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponseBotIcons {
    pub image_36: Option<String>,
    pub image_48: Option<String>,
    pub image_72: Option<String>,
}
