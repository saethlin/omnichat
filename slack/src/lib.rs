// Copyright 2015-2016 the slack-rs authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Low-level, direct interface for the [Slack Web
//! API](https://api.slack.com/methods).
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate chrono;
#[cfg(any(feature = "reqwest", test))]
extern crate reqwest;
extern crate serde_json;
#[cfg(any(feature = "reqwest", test))]
extern crate serde_urlencoded;
extern crate uuid;
#[macro_use]
extern crate derive_new;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

pub mod http;
pub mod rtm;

mod timestamp;
pub use timestamp::Timestamp;

mod id;
pub use id::*;

fn serialize_comma_separated<T, S>(items: &[T], serializer: S) -> Result<S::Ok, S::Error>
where
    S: ::serde::Serializer,
    T: ::serde::Serialize + ::std::fmt::Display,
{
    use std::fmt::Write;

    let mut output = String::with_capacity(items.len() * (ID_LENGTH + 1));
    for item in items {
        let _ = write!(output, "{},", item);
    }
    output.pop(); // Remove last comma, does nothing if output is empty

    // Create a string that we can then serialize
    serializer.serialize_str(&output)
}
