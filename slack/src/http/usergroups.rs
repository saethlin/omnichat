//! Get info on your team's User Groups.

use rtm::Usergroup;

/// Create a User Group
///
/// Wraps https://api.slack.com/methods/usergroups.create

#[derive(Clone, Debug, Serialize, new)]
pub struct CreateRequest<'a> {
    /// A name for the User Group. Must be unique among User Groups.
    pub name: &'a str,
    /// A mention handle. Must be unique among channels, users and User Groups.
    #[new(default)]
    pub handle: Option<&'a str>,
    /// A short description of the User Group.
    #[new(default)]
    pub description: Option<&'a str>,
    /// A comma separated string of encoded channel IDs for which the User Group uses as a default.
    #[new(default)]
    pub channels: Option<&'a str>,
    /// Include the number of users in each User Group.
    #[new(default)]
    pub include_count: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    ok: bool,
    pub usergroup: Option<Usergroup>,
}

/// Disable an existing User Group
///
/// Wraps https://api.slack.com/methods/usergroups.disable

#[derive(Clone, Debug, Serialize, new)]
pub struct DisableRequest {
    /// The encoded ID of the User Group to disable.
    pub usergroup: ::UsergroupId,
    /// Include the number of users in the User Group.
    #[new(default)]
    pub include_count: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisableResponse {
    ok: bool,
    pub usergroup: Option<Usergroup>,
}

/// Enable a User Group
///
/// Wraps https://api.slack.com/methods/usergroups.enable

#[derive(Clone, Debug, Serialize, new)]
pub struct EnableRequest {
    /// The encoded ID of the User Group to enable.
    pub usergroup: ::UsergroupId,
    /// Include the number of users in the User Group.
    #[new(default)]
    pub include_count: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnableResponse {
    ok: bool,
    pub usergroup: Option<Usergroup>,
}

/// List all User Groups for a team
///
/// Wraps https://api.slack.com/methods/usergroups.list

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// Include disabled User Groups.
    #[new(default)]
    pub include_disabled: Option<bool>,
    /// Include the number of users in each User Group.
    #[new(default)]
    pub include_count: Option<bool>,
    /// Include the list of users for each User Group.
    #[new(default)]
    pub include_users: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub usergroups: Option<Vec<Usergroup>>,
}

/// Update an existing User Group
///
/// Wraps https://api.slack.com/methods/usergroups.update

#[derive(Clone, Debug, Serialize, new)]
pub struct UpdateRequest<'a> {
    /// The encoded ID of the User Group to update.
    pub usergroup: ::UsergroupId,
    /// A name for the User Group. Must be unique among User Groups.
    #[new(default)]
    pub name: Option<&'a str>,
    /// A mention handle. Must be unique among channels, users and User Groups.
    #[new(default)]
    pub handle: Option<&'a str>,
    /// A short description of the User Group.
    #[new(default)]
    pub description: Option<&'a str>,
    /// A comma separated string of encoded channel IDs for which the User Group uses as a default.
    #[new(default)]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub channels: &'a [::UserId],
    /// Include the number of users in the User Group.
    #[new(default)]
    pub include_count: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateResponse {
    ok: bool,
    pub usergroup: Option<Usergroup>,
}
