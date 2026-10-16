//! Stored messages as the API returns them: the AI SDK `UIMessage` shape used by
//! session transcripts and task run messages.

use serde::{Deserialize, Serialize};

/// One stored message of a transcript.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UiMessage {}

/// One part of a stored message.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UiPart {}
