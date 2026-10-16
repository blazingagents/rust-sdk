//! Typed resource IDs. A `&str` converts into any ID type, so literals stay short,
//! while an ID taken from a response cannot be passed where another kind belongs.

use std::fmt;
use std::str::FromStr;

use crate::error::ConfigError;

macro_rules! id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// The ID as a string.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// The ID as an owned string.
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&$name> for $name {
            fn from(value: &$name) -> Self {
                value.clone()
            }
        }

        impl From<&String> for $name {
            fn from(value: &String) -> Self {
                Self(value.clone())
            }
        }
    };
}

id!(
    /// An agent ID (`ag_…`).
    AgentId
);
id!(
    /// A session ID (`ss_…`).
    SessionId
);
id!(
    /// A turn ID (`turn_…`).
    TurnId
);
id!(
    /// A skill ID (`skill_…`).
    SkillId
);
id!(
    /// A memory ID (`mem_…`).
    MemoryId
);
id!(
    /// A prompt ID (`prompt_…`).
    PromptId
);
id!(
    /// A task ID (`tk_…`).
    TaskId
);
id!(
    /// A task run ID (`tr_…`).
    TaskRunId
);
id!(
    /// A workspace ID (`ws_…`).
    WorkspaceId
);
id!(
    /// An artifact ID (`at_…`).
    ArtifactId
);
id!(
    /// A provider ID (`prv_…`).
    ProviderId
);
id!(
    /// An MCP connection ID (`mcp_…`).
    McpConnectionId
);
id!(
    /// A chat connection ID (`cc_…`).
    ChatConnectionId
);
id!(
    /// A chat delivery ID (`cd_…`).
    ChatDeliveryId
);
id!(
    /// A merchant usage event ID (`mev_…`).
    MerchantUsageEventId
);
id!(
    /// A merchant connection ID (`mch_…`).
    MerchantConnectionId
);
id!(
    /// A backend function call ID (`fc_…`).
    FunctionCallId
);
id!(
    /// A tool approval ID (`apr_…`).
    ApprovalId
);
id!(
    /// A tenant ID (`ten_…`).
    TenantId
);

/// An end user's ID for [`Client::for_user`](crate::Client::for_user): 1 to 256
/// printable ASCII characters without a leading or trailing space.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    /// Validates `value` as a user ID.
    pub fn new(value: impl Into<String>) -> Result<Self, ConfigError> {
        let value = value.into();
        let printable_ascii = value.bytes().all(|byte| (0x20..=0x7E).contains(&byte));
        let valid = !value.is_empty()
            && value.len() <= 256
            && printable_ascii
            && !value.starts_with(' ')
            && !value.ends_with(' ');
        if valid {
            Ok(Self(value))
        } else {
            Err(ConfigError::InvalidUserId)
        }
    }

    /// The ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for UserId {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}
