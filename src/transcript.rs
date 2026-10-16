//! Stored messages as the API returns them: the AI SDK `UIMessage` shape used by
//! session transcripts and task run messages.
//!
//! Documentation: <https://docs.blazingagents.com/platform/sessions-and-turns>.

use serde::{Deserialize, Serialize};

crate::open_enum! {
    /// A stored message's role.
    pub enum MessageRole {
        /// `system`
        System = "system",
        /// `user`
        User = "user",
        /// `assistant`
        Assistant = "assistant",
    }
}

/// One stored message of a transcript.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UiMessage {
    /// The message's ID.
    pub id: String,
    /// The message's role.
    pub role: MessageRole,
    /// `true` when the message is a saved assistant reply that
    /// [`Sessions::fork`](crate::sessions::Sessions::fork) accepts.
    pub branchable: bool,
    /// The message's parts, in order.
    pub parts: Vec<UiPart>,
    /// Data stored with the message, if any.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// One part of a stored message.
///
/// Parts are deliberately untyped: the AI SDK keeps adding part types, and a
/// typed enum would make each addition a breaking change. [`kind`](UiPart::kind)
/// is the wire `type` and [`fields`](UiPart::fields) keeps everything else.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UiPart {
    /// The part's type, such as `"text"`, `"reasoning"` or `"tool-bash"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The part's remaining fields.
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl UiPart {
    /// The part's text, when it is a `"text"` or `"reasoning"` part.
    pub fn text(&self) -> Option<&str> {
        if self.kind == "text" || self.kind == "reasoning" {
            self.fields.get("text")?.as_str()
        } else {
            None
        }
    }
}
