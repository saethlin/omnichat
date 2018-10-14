//! Search your team's files and messages.

use rtm::{File, Message, Paging};

#[derive(Clone, Debug, Serialize)]
pub enum SortDirection {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename = "snake_case")]
pub enum SortBy {
    Score,
    Timestamp,
}

/// Searches for messages and files matching a query.
///
/// Wraps https://api.slack.com/methods/search.all

api_call!(all, "search.all", AllRequest => AllResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct AllRequest<'a> {
    /// Search query. May contains booleans, etc.
    pub query: &'a str,
    /// Return matches sorted by either score or timestamp.
    pub sort: Option<SortBy>,
    /// Change sort direction to ascending (asc) or descending (desc).
    pub sort_dir: Option<SortDirection>,
    /// Pass a value of true to enable query highlight markers (see below).
    pub highlight: Option<bool>,
    /// Number of items to return per page.
    pub count: Option<u32>,
    /// Page number of results to return.
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllResponse {
    ok: bool,
    pub files: Option<AllResponseFiles>,
    pub messages: Option<AllResponseMessages>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllResponseFiles {
    pub matches: Vec<File>,
    pub paging: Paging,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllResponseMessages {
    pub matches: Vec<Message>,
    pub paging: Paging,
}

/// Searches for files matching a query.
///
/// Wraps https://api.slack.com/methods/search.files

api_call!(files, "search.files", FilesRequest => FilesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct FilesRequest<'a> {
    /// Search query. May contain booleans, etc.
    #[new(default)]
    pub query: &'a str,
    /// Return matches sorted by either score or timestamp.
    #[new(default)]
    pub sort: Option<SortBy>,
    /// Change sort direction to ascending (asc) or descending (desc).
    #[new(default)]
    pub sort_dir: Option<SortDirection>,
    /// Pass a value of true to enable query highlight markers (see below).
    #[new(default)]
    pub highlight: Option<bool>,
    /// Number of items to return per page.
    #[new(default)]
    pub count: Option<u32>,
    /// Page number of results to return.
    #[new(default)]
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesResponse {
    ok: bool,
    pub files: Option<FilesResponseFiles>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesResponseFiles {
    pub matches: Option<Vec<File>>,
    pub paging: Option<Paging>,
    pub total: Option<u32>,
}

/// Searches for messages matching a query.
///
/// Wraps https://api.slack.com/methods/search.messages

api_call!(messages, "search.messages", MessagesRequest => MessagesResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct MessagesRequest<'a> {
    /// Search query. May contains booleans, etc.
    pub query: &'a str,
    /// Return matches sorted by either score or timestamp.
    #[new(default)]
    pub sort: Option<SortBy>,
    /// Change sort direction to ascending (asc) or descending (desc).
    #[new(default)]
    pub sort_dir: Option<SortDirection>,
    /// Pass a value of true to enable query highlight markers (see below).
    #[new(default)]
    pub highlight: Option<bool>,
    /// Number of items to return per page.
    #[new(default)]
    pub count: Option<u32>,
    /// Page number of results to return.
    #[new(default)]
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagesResponse {
    ok: bool,
    pub messages: Option<MessagesResponseMessages>,
    pub query: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessagesResponseMessages {
    pub matches: Option<Vec<Message>>,
    pub paging: Option<Paging>,
    pub total: Option<u32>,
}
