use std::collections::HashMap;

/// Checks API calling code.
///
/// Wraps https://api.slack.com/methods/api.test
#[derive(Debug, Clone, Serialize, new)]
pub struct TestRequest<'a> {
    /// Error response to return
    #[new(default)]
    error: Option<&'a str>,
    /// example property to return
    #[new(default)]
    foo: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestResponse {
    args: HashMap<String, String>,
}

api_call!(test, "api.test", TestRequest => TestResponse);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test() {
        let client = ::reqwest::Client::new();
        let token = ::std::env::var("SLACK_API_TOKEN").unwrap();

        let mut req = TestRequest::new();
        req.foo = Some("bar");

        let response = test(&client, &token, &req).unwrap();
        assert_eq!(response.args["foo"], "bar".to_string());
    }
}
