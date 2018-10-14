//! Functionality for sending requests to Slack.

pub trait SlackSender {
    type Error: ::std::error::Error;

    fn send_structured<T: ::serde::Serialize>(
        &self,
        method_url: &str,
        params: &T,
    ) -> Result<String, Self::Error>;
}

#[cfg(any(feature = "reqwest", test))]
impl SlackSender for ::reqwest::Client {
    type Error = ::reqwest::Error;
    /// Make an API call to Slack. Takes a struct that describes the request params
    fn send_structured<T: ::serde::Serialize>(
        &self,
        method_url: &str,
        params: &T,
    ) -> Result<String, ::reqwest::Error> {
        let mut url_text = method_url.to_string();
        if let Ok(s) = ::serde_urlencoded::to_string(params) {
            url_text += &s;
        } else {
            // TODO: Log the error
        }
        let url =
            ::reqwest::Url::parse(&url_text).expect("Internal error, failed to parse Slack URL");
        self.get(url).send()?.text()
    }
}

macro_rules! api_call {
    ($name:ident, $strname:expr, $reqty:ty => $okty:ty) => {
        pub fn $name<C: ::http::SlackSender>(
            client: &C,
            token: &str,
            request: &$reqty,
        ) -> Result<$okty, ::http::Error<C>> {
            api_call_internal!(client, token, $strname, request, $okty)
        }
    };
    ($name:ident, $strname:expr, => $okty:ty) => {
        pub fn $name<C: ::http::SlackSender>(
            client: &C,
            token: &str,
        ) -> Result<$okty, ::http::Error<C>> {
            api_call_internal!(client, token, $strname, &"", $okty)
        }
    };
    ($name:ident, $strname:expr, $reqty:ty => ) => {
        pub fn $name<C: ::http::SlackSender>(
            client: &C,
            token: &str,
            request: &$reqty,
        ) -> Result<(), ::http::Error<C>> {
            #[allow(dead_code)] // But isn't serde using the field?
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct SimpleOk {
                ok: bool,
            }

            api_call_internal!(client, token, $strname, request, SimpleOk).map(|_| ())
        }
    };
    ($name:ident, $strname:expr) => {
        pub fn $name<C: ::http::SlackSender>(
            client: &C,
            token: &str,
        ) -> Result<(), ::http::Error<C>> {
            #[allow(dead_code)] // But isn't serde using the field?
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct SimpleOk {
                ok: bool,
            }

            api_call_internal!(client, token, $strname, &"", SimpleOk).map(|_| ())
        }
    };
}

macro_rules! api_call_internal {
    ($client:expr, $token:expr, $strname:expr, $request:expr, $okty:ty) => {{
        use http::Error;
        #[derive(Deserialize)]
        struct IsError {
            ok: bool,
            error: Option<String>,
        }

        let url = format!("https://slack.com/api/{}?token={}&", $strname, $token);
        let bytes = $client
            .send_structured(&url, $request)
            .map_err(Error::Client)?;

        let is_error = ::serde_json::from_str::<IsError>(&bytes);
        match is_error {
            // Complete failure, can't do anything with the bytes
            Err(e) => Err(Error::CannotParse(e, bytes)),
            // Slack sent us an error
            Ok(IsError { ok: false, error }) => Err(Error::Slack(error.unwrap_or_default())),
            // Slack sent us an success result
            Ok(IsError { ok: true, .. }) => match ::serde_json::from_str::<$okty>(&bytes) {
                Ok(r) => Ok(r),
                Err(e) => Err(Error::CannotParse(e, bytes)),
            },
        }
    }};
}

pub enum Error<C: SlackSender> {
    Slack(String),
    CannotParse(::serde_json::error::Error, String),
    Client(C::Error),
}

impl<C: SlackSender> ::std::fmt::Debug for Error<C> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::Slack(reason) => write!(f, "{}", reason),
            Error::CannotParse(e, json) => {
                let v: ::serde_json::Value = ::serde_json::from_str(&json).unwrap();
                write!(f, "{}\n{}", e, ::serde_json::to_string_pretty(&v).unwrap())
            }
            Error::Client(..) => write!(f, "The requests client failed"),
        }
    }
}

impl<C: SlackSender> ::std::fmt::Display for Error<C> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            Error::Slack(reason) => write!(f, "{}", reason),
            Error::CannotParse(e, json) => {
                let v: ::serde_json::Value = ::serde_json::from_str(&json).unwrap();
                write!(f, "{}\n{}", e, ::serde_json::to_string_pretty(&v).unwrap())
            }
            Error::Client(..) => write!(f, "The requests client failed"),
        }
    }
}

impl<C: SlackSender> ::std::error::Error for Error<C> {
    fn description(&self) -> &str {
        match self {
            Error::Slack(ref reason) => reason,
            Error::CannotParse(..) => "Could not parse as specified result type",
            Error::Client(..) => "The requests client failed",
        }
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        match self {
            Error::Slack(_) => None,
            Error::CannotParse(ref cause, _) => Some(cause),
            Error::Client(ref cause) => Some(cause),
        }
    }
}

pub mod api;
pub mod auth;
pub mod bots;
pub mod channels;
pub mod chat;
pub mod conversations;
pub mod dnd;
pub mod emoji;
pub mod files;
pub mod groups;
pub mod im;
pub mod mpim;
pub mod oauth;
pub mod pins;
pub mod reactions;
pub mod reminders;
pub mod rtm;
pub mod search;
pub mod stars;
pub mod team;
pub mod team_profile;
pub mod usergroups;
pub mod usergroups_users;
pub mod users;
pub mod users_profile;
