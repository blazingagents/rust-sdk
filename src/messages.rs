//! Chat input: an AI SDK `UIMessage` with role `user`. Build one in code or
//! deserialize the browser's JSON; either way it holds at least one part.

use serde::{Deserialize, Serialize};

/// A user message to send with [`Client::chat`](crate::Client::chat) or
/// [`Sessions::submit_input`](crate::sessions::Sessions::submit_input).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WireMessage", into = "WireMessage")]
pub struct UserMessage {
    id: String,
    first: UserPart,
    rest: Vec<UserPart>,
    metadata: Option<serde_json::Value>,
}

impl UserMessage {
    /// A text message with a random UUID as its ID. Keep the message and resend
    /// it to retry without a `message_id_conflict`.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(
            uuid::Uuid::new_v4().to_string(),
            UserPart::Text { text: text.into() },
        )
    }

    /// A message with the given ID and first part.
    pub fn new(id: impl Into<String>, first: UserPart) -> Self {
        Self {
            id: id.into(),
            first,
            rest: Vec::new(),
            metadata: None,
        }
    }

    /// Appends a part.
    #[must_use]
    pub fn part(mut self, part: UserPart) -> Self {
        self.rest.push(part);
        self
    }

    /// Sets the message's metadata.
    #[must_use]
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// The message ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The parts, in order. There is always at least one.
    pub fn parts(&self) -> impl Iterator<Item = &UserPart> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

/// One part of a [`UserMessage`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum UserPart {
    /// Text.
    Text {
        /// The text.
        text: String,
    },
    /// A file, such as an image, as a `data:` URL or a fetchable URL.
    File {
        /// The MIME type.
        media_type: String,
        /// The URL.
        url: String,
    },
    /// A part type this SDK version does not know. Only deserialization creates
    /// it, and it is sent unchanged.
    Unrecognized(RawPart),
}

/// A part the SDK does not know, kept as its JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct RawPart(serde_json::Value);

impl RawPart {
    /// The part's JSON.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum KnownPart {
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    File {
        media_type: String,
        url: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WirePart {
    Known(KnownPart),
    Other(serde_json::Value),
}

impl From<UserPart> for WirePart {
    fn from(part: UserPart) -> Self {
        match part {
            UserPart::Text { text } => WirePart::Known(KnownPart::Text { text }),
            UserPart::File { media_type, url } => {
                WirePart::Known(KnownPart::File { media_type, url })
            }
            UserPart::Unrecognized(RawPart(value)) => WirePart::Other(value),
        }
    }
}

impl From<WirePart> for UserPart {
    fn from(part: WirePart) -> Self {
        match part {
            WirePart::Known(KnownPart::Text { text }) => UserPart::Text { text },
            WirePart::Known(KnownPart::File { media_type, url }) => {
                UserPart::File { media_type, url }
            }
            WirePart::Other(value) => UserPart::Unrecognized(RawPart(value)),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct WireMessage {
    id: String,
    role: String,
    parts: Vec<WirePart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

/// JSON that is not a user message with at least one part.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InvalidUserMessage {
    /// `role` is not `user`.
    #[error("message role must be \"user\"")]
    Role,
    /// `id` is empty.
    #[error("message id must not be empty")]
    EmptyId,
    /// `parts` is empty.
    #[error("message must have at least one part")]
    NoParts,
}

impl TryFrom<WireMessage> for UserMessage {
    type Error = InvalidUserMessage;

    fn try_from(wire: WireMessage) -> Result<Self, Self::Error> {
        if wire.role != "user" {
            return Err(InvalidUserMessage::Role);
        }
        if wire.id.is_empty() {
            return Err(InvalidUserMessage::EmptyId);
        }
        let mut parts = wire.parts.into_iter().map(UserPart::from);
        let first = parts.next().ok_or(InvalidUserMessage::NoParts)?;
        Ok(Self {
            id: wire.id,
            first,
            rest: parts.collect(),
            metadata: wire.metadata,
        })
    }
}

impl From<UserMessage> for WireMessage {
    fn from(message: UserMessage) -> Self {
        Self {
            id: message.id,
            role: "user".to_owned(),
            parts: std::iter::once(message.first)
                .chain(message.rest)
                .map(WirePart::from)
                .collect(),
            metadata: message.metadata,
        }
    }
}
