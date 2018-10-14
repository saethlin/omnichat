/// Gets information about a bot user.
///
/// Wraps https://api.slack.com/methods/bots.info

api_call!(info, "bots.info", InfoRequest => InfoResponse);
// This is very silly to call without a bot in the request
// especially because that's the only situation in which we get an ok but no bot field
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_info() {
        let client = ::reqwest::Client::new();
        let token = env::var("SLACK_API_TOKEN").unwrap();

        info(&client, &token, &InfoRequest::new()).unwrap();
    }
}
