extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate derive_new;

pub mod http;
pub mod rtm;

mod id;
pub use self::id::*;
mod timestamp;
pub use self::timestamp::Timestamp;

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
