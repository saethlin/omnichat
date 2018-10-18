use serde::de::{self, Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, Serializer};
use std::fmt;

#[derive(Clone, Copy, Debug, Default)]
pub struct Timestamp {
    pub microseconds: i64,
}

struct TimestampVisitor;
impl<'de> Visitor<'de> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a Unix-style timestamp, as a u32 or string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Timestamp, E>
    where
        E: de::Error,
    {
        if value.len() <= 17 {
            // Split at the decimal point
            let dot_location = value
                .find('.')
                .ok_or_else(|| Error::custom("Got a string without a ."))?;
            let (seconds_str, micros_str) = value.split_at(dot_location);
            let seconds = seconds_str
                .parse::<i64>()
                .map_err(|_| Error::custom(format!("Cannot parse {} as a number", seconds_str)))?;
            let microseconds = micros_str[1..]
                .parse::<i64>()
                .map_err(|_| Error::custom(format!("Cannot parse {} as a number", micros_str)))?;
            Ok(Timestamp {
                microseconds: seconds * 1_000_000 + microseconds,
            })
        } else {
            Err(E::custom(format!(
                "Timestamps must be string or number with 16 decimal places, got {}",
                value
            )))
        }
    }

    fn visit_u64<E>(self, value: u64) -> Result<Timestamp, E>
    where
        E: de::Error,
    {
        Ok(Timestamp {
            microseconds: value as i64 * 1_000_000,
        })
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Timestamp, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TimestampVisitor)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl ::std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}.{:06}",
            self.microseconds / 1_000_000,
            self.microseconds % 1_000_000
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn ser_de() {
        let the_json = "\"1234.567890\"";

        let ts: Timestamp = ::serde_json::from_str(the_json).unwrap();
        let as_str = ts.to_string();

        assert_eq!(&as_str, "1234.567890");
    }
}
