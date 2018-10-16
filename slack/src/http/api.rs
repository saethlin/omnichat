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
