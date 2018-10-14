use rtm::Usergroup;

/// List all users in a User Group
///
/// Wraps https://api.slack.com/methods/usergroups.users.list

api_call!(list, "usergroups.users.list", ListRequest => ListResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct ListRequest {
    /// The encoded ID of the User Group to update.
    pub usergroup: ::UsergroupId,
    /// Allow results that involve disabled User Groups.
    #[new(default)]
    pub include_disabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListResponse {
    ok: bool,
    pub users: Option<Vec<::UserId>>,
}

/// Update the list of users for a User Group
///
/// Wraps https://api.slack.com/methods/usergroups.users.update

api_call!(update, "usergroups.users.update", UpdateRequest => UpdateResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct UpdateRequest<'a> {
    /// The encoded ID of the User Group to update.
    pub usergroup: ::UsergroupId,
    /// A comma separated string of encoded user IDs that represent the entire list of users for the User Group.
    #[new(default)]
    #[serde(serialize_with = "::serialize_comma_separated")]
    pub users: &'a [::UserId],
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
