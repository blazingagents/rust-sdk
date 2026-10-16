//! Value types shared by resource models.

use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An RFC 3339 instant. Ordering compares instants, and `Display` and
/// `Serialize` write RFC 3339 in UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(jiff::Timestamp);

impl Timestamp {
    /// The instant as a `SystemTime`.
    pub fn to_system_time(self) -> SystemTime {
        SystemTime::from(self.0)
    }

    /// Milliseconds since the Unix epoch.
    pub fn as_unix_millis(self) -> i64 {
        self.0.as_millisecond()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// An RFC 3339 string that is not a timestamp.
#[derive(Debug, thiserror::Error)]
#[error("invalid RFC 3339 timestamp: {0}")]
pub struct InvalidTimestamp(String);

impl FromStr for Timestamp {
    type Err = InvalidTimestamp;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse()
            .map(Self)
            .map_err(|_| InvalidTimestamp(value.to_owned()))
    }
}

impl TryFrom<SystemTime> for Timestamp {
    type Error = InvalidTimestamp;

    fn try_from(value: SystemTime) -> Result<Self, Self::Error> {
        jiff::Timestamp::try_from(value)
            .map(Self)
            .map_err(|error| InvalidTimestamp(error.to_string()))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A JSON object of caller-owned data, returned unchanged by the API.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata(serde_json::Map<String, serde_json::Value>);

impl Metadata {
    /// An empty object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces one key.
    #[must_use]
    pub fn insert(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    /// One value by key.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.get(key)
    }

    /// The underlying JSON object.
    pub fn as_map(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.0
    }

    /// The underlying JSON object.
    pub fn into_map(self) -> serde_json::Map<String, serde_json::Value> {
        self.0
    }
}

impl<K: Into<String>, V: Into<serde_json::Value>> FromIterator<(K, V)> for Metadata {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        )
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for Metadata {
    fn from(map: serde_json::Map<String, serde_json::Value>) -> Self {
        Self(map)
    }
}

/// A JSON value that is not an object.
#[derive(Debug, thiserror::Error)]
#[error("metadata must be a JSON object")]
pub struct NotAnObject;

impl TryFrom<serde_json::Value> for Metadata {
    type Error = NotAnObject;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        match value {
            serde_json::Value::Object(map) => Ok(Self(map)),
            _ => Err(NotAnObject),
        }
    }
}

/// Decodes a JSON object whose `tag` field selects the variant. `decode_known`
/// returns `None` for a tag it does not handle, which becomes `unrecognized`
/// with the whole object; a `Some(Err(..))` from a handled tag is a decode
/// error. A value that is not an object, or whose tag is missing or not a
/// string, is an error.
pub(crate) fn decode_tagged<'de, D, T>(
    deserializer: D,
    tag: &'static str,
    decode_known: impl FnOnce(
        &str,
        &serde_json::Map<String, serde_json::Value>,
    ) -> Option<Result<T, serde_json::Error>>,
    unrecognized: impl FnOnce(serde_json::Map<String, serde_json::Value>) -> T,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
    let Some(tag_value) = raw.get(tag).and_then(serde_json::Value::as_str) else {
        return Err(serde::de::Error::custom(format!(
            "`{tag}` must be a string"
        )));
    };
    match decode_known(tag_value, &raw) {
        Some(result) => result.map_err(serde::de::Error::custom),
        None => Ok(unrecognized(raw)),
    }
}

/// A wire value that no known variant of an open enum matches. Only decoding and
/// parsing create it, so it never holds a value a known variant would match.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unrecognized(Box<str>);

impl Unrecognized {
    pub(crate) fn new(value: &str) -> Self {
        Self(value.into())
    }

    /// The wire value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Unrecognized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
