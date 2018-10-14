use rtm::Reminder;

/// Creates a reminder.
///
/// Wraps https://api.slack.com/methods/reminders.add

api_call!(add, "reminders.add", AddRequest => AddResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct AddRequest<'a> {
    /// The content of the reminder
    pub text: &'a str,
    /// When this reminder should happen: the Unix timestamp (up to five years from now), the number of seconds until the reminder (if within 24 hours), or a natural language description (Ex. "in 15 minutes," or "every Thursday")
    pub time: u32,
    /// The user who will receive the reminder. If no user is specified, the reminder will go to user who created it.
    #[new(default)]
    pub user: Option<::UserId>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddResponse {
    ok: bool,
    pub reminder: Option<Reminder>,
}

/// Marks a reminder as complete.
///
/// Wraps https://api.slack.com/methods/reminders.complete

api_call!(complete, "reminders.complete", CompleteRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct CompleteRequest {
    /// The ID of the reminder to be marked as complete
    pub reminder: ::ReminderId,
}

/// Deletes a reminder.
///
/// Wraps https://api.slack.com/methods/reminders.delete

api_call!(delete, "reminders.delete", DeleteRequest =>);

#[derive(Clone, Debug, Serialize, new)]
pub struct DeleteRequest {
    /// The ID of the reminder
    pub reminder: ::ReminderId,
}

/// Gets information about a reminder.
///
/// Wraps https://api.slack.com/methods/reminders.info

api_call!(info, "reminders.info", InfoRequest => InfoResponse);

#[derive(Clone, Debug, Serialize, new)]
pub struct InfoRequest {
    /// The ID of the reminder
    pub reminder: ::ReminderId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoResponse {
    ok: bool,
    pub reminder: Option<Reminder>,
}

/// Lists all reminders created by or for a given user.
///
/// Wraps https://api.slack.com/methods/reminders.list
// TODO: Docs say "created by or for a given user", but also do not mention how to indicate said
// user
api_call!(list, "reminders.list", => ListResponse);

#[derive(Clone, Debug, Deserialize)]
pub struct ListResponse {
    ok: bool,
    pub reminders: Option<Vec<Reminder>>,
}
