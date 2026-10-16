//! MCP connections. Tenant only; start from [`Client::mcp_connections`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/tools/mcp-tools>.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::McpConnectionId;
use crate::op::Op;
use crate::types::Timestamp;

/// MCP connection operations. Returned by [`Client::mcp_connections`].
#[derive(Clone, Copy, Debug)]
pub struct McpConnections<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// MCP connection operations.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.mcp_connections();
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::Client) {
    /// let mcp_connections = client.mcp_connections();
    /// # }
    /// ```
    pub fn mcp_connections(&self) -> McpConnections<'_> {
        McpConnections { client: self }
    }
}

impl McpConnections<'_> {
    /// Lists MCP connections.
    pub fn list(&self) -> Call<ListMcpConnections> {
        Call::new(self.client, Op::ListMcpConnections, ListMcpConnections)
    }

    /// Creates an MCP connection. Secrets are never returned.
    pub fn create(&self, connection: NewMcpConnection) -> Call<CreateMcpConnection> {
        Call::new(
            self.client,
            Op::CreateMcpConnection,
            CreateMcpConnection { connection },
        )
    }

    /// Gets one MCP connection.
    pub fn get(&self, mcp_connection_id: impl Into<McpConnectionId>) -> Call<GetMcpConnection> {
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::GetMcpConnection {
                id: mcp_connection_id.as_str(),
            },
            GetMcpConnection,
        )
    }

    /// Renames an MCP connection.
    pub fn update(
        &self,
        mcp_connection_id: impl Into<McpConnectionId>,
        name: impl Into<String>,
    ) -> Call<UpdateMcpConnection> {
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::UpdateMcpConnection {
                id: mcp_connection_id.as_str(),
            },
            UpdateMcpConnection { name: name.into() },
        )
    }

    /// Deletes an MCP connection.
    pub fn delete(
        &self,
        mcp_connection_id: impl Into<McpConnectionId>,
    ) -> Call<DeleteMcpConnection> {
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::DeleteMcpConnection {
                id: mcp_connection_id.as_str(),
            },
            DeleteMcpConnection,
        )
    }

    /// Tests an MCP connection against its server.
    pub fn test(&self, mcp_connection_id: impl Into<McpConnectionId>) -> Call<TestMcpConnection> {
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::TestMcpConnection {
                id: mcp_connection_id.as_str(),
            },
            TestMcpConnection,
        )
    }

    /// Replaces an MCP connection's URL and credentials. Secrets are never
    /// returned.
    pub fn reconnect(
        &self,
        mcp_connection_id: impl Into<McpConnectionId>,
        auth: McpReconnect,
    ) -> Call<ReconnectMcpConnection> {
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::ReconnectMcpConnection {
                id: mcp_connection_id.as_str(),
            },
            ReconnectMcpConnection { auth },
        )
    }
}

crate::open_enum! {
    /// How Blazing Agents authenticates to the MCP server.
    pub enum McpAuthType {
        /// `none`
        None = "none",
        /// `bearer`
        Bearer = "bearer",
        /// `oauth_authorization_code`
        OAuthAuthorizationCode = "oauth_authorization_code",
        /// `oauth_client_credentials`
        OAuthClientCredentials = "oauth_client_credentials",
    }
}

crate::open_enum! {
    /// Whether an MCP connection is usable.
    pub enum McpConnectionStatus {
        /// `connected`
        Connected = "connected",
        /// `needs_auth`
        NeedsAuth = "needs_auth",
        /// `error`
        Error = "error",
    }
}

crate::open_enum! {
    /// Why an MCP connection last failed to authenticate or be reached.
    pub enum McpConnectionErrorCode {
        /// `MCP_CONNECTION_AUTHENTICATION_FAILED`
        AuthenticationFailed = "MCP_CONNECTION_AUTHENTICATION_FAILED",
        /// `MCP_CONNECTION_INVALID`
        Invalid = "MCP_CONNECTION_INVALID",
        /// `MCP_CONNECTION_UNREACHABLE`
        Unreachable = "MCP_CONNECTION_UNREACHABLE",
        /// `MCP_CONNECTION_DISCOVERY_FAILED`
        DiscoveryFailed = "MCP_CONNECTION_DISCOVERY_FAILED",
    }
}

/// The URL and credentials of a new MCP connection: one variant per `authType`.
/// Fields that do not apply to a variant cannot be sent with it.
#[derive(Clone, Serialize)]
#[non_exhaustive]
#[serde(tag = "authType", rename_all_fields = "camelCase")]
pub enum NewMcpConnection {
    /// No authentication.
    #[serde(rename = "none")]
    None {
        /// The display name, unique within your tenant.
        name: String,
        /// The streamable HTTP URL of the MCP server.
        url: String,
    },
    /// An `Authorization: Bearer <token>` header.
    #[serde(rename = "bearer")]
    Bearer {
        /// The display name, unique within your tenant.
        name: String,
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The bearer token. Never returned.
        bearer_token: String,
    },
    /// OAuth with the authorization code flow. Send `client_id` and
    /// `client_secret` together or neither. The connection is saved as
    /// `needs_auth` until an admin signs in through the dashboard.
    #[serde(rename = "oauth_authorization_code")]
    OAuthAuthorizationCode {
        /// The display name, unique within your tenant.
        name: String,
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The OAuth client ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
        /// The OAuth client secret. Never returned.
        #[serde(skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
        /// The OAuth scope to request.
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
    /// OAuth with the client credentials flow.
    #[serde(rename = "oauth_client_credentials")]
    OAuthClientCredentials {
        /// The display name, unique within your tenant.
        name: String,
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The OAuth client ID.
        client_id: String,
        /// The OAuth client secret. Never returned.
        client_secret: String,
        /// The OAuth scope to request.
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
}

impl NewMcpConnection {
    fn secrets(&self) -> Vec<String> {
        match self {
            Self::None { .. } => Vec::new(),
            Self::Bearer { bearer_token, .. } => vec![bearer_token.clone()],
            Self::OAuthAuthorizationCode { client_secret, .. } => {
                client_secret.clone().into_iter().collect()
            }
            Self::OAuthClientCredentials { client_secret, .. } => {
                vec![client_secret.clone()]
            }
        }
    }
}

impl fmt::Debug for NewMcpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None { name, url } => f
                .debug_struct("None")
                .field("name", name)
                .field("url", url)
                .finish(),
            Self::Bearer { name, url, .. } => f
                .debug_struct("Bearer")
                .field("name", name)
                .field("url", url)
                .field("bearer_token", &"[redacted]")
                .finish(),
            Self::OAuthAuthorizationCode {
                name,
                url,
                client_id,
                client_secret,
                scope,
            } => f
                .debug_struct("OAuthAuthorizationCode")
                .field("name", name)
                .field("url", url)
                .field("client_id", client_id)
                .field(
                    "client_secret",
                    &client_secret.as_ref().map(|_| "[redacted]"),
                )
                .field("scope", scope)
                .finish(),
            Self::OAuthClientCredentials {
                name,
                url,
                client_id,
                scope,
                ..
            } => f
                .debug_struct("OAuthClientCredentials")
                .field("name", name)
                .field("url", url)
                .field("client_id", client_id)
                .field("client_secret", &"[redacted]")
                .field("scope", scope)
                .finish(),
        }
    }
}

/// The URL and credentials to reconnect with: one variant per `authType`.
#[derive(Clone, Serialize)]
#[non_exhaustive]
#[serde(tag = "authType", rename_all_fields = "camelCase")]
pub enum McpReconnect {
    /// No authentication.
    #[serde(rename = "none")]
    None {
        /// The streamable HTTP URL of the MCP server.
        url: String,
    },
    /// An `Authorization: Bearer <token>` header.
    #[serde(rename = "bearer")]
    Bearer {
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The bearer token. Never returned.
        bearer_token: String,
    },
    /// OAuth with the authorization code flow. Send `client_id` and
    /// `client_secret` together or neither. The connection stays `needs_auth`
    /// until an admin signs in through the dashboard.
    #[serde(rename = "oauth_authorization_code")]
    OAuthAuthorizationCode {
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The OAuth client ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
        /// The OAuth client secret. Never returned.
        #[serde(skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
        /// The OAuth scope to request.
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
    /// OAuth with the client credentials flow.
    #[serde(rename = "oauth_client_credentials")]
    OAuthClientCredentials {
        /// The streamable HTTP URL of the MCP server.
        url: String,
        /// The OAuth client ID.
        client_id: String,
        /// The OAuth client secret. Never returned.
        client_secret: String,
        /// The OAuth scope to request.
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
}

impl McpReconnect {
    fn secrets(&self) -> Vec<String> {
        match self {
            Self::None { .. } => Vec::new(),
            Self::Bearer { bearer_token, .. } => vec![bearer_token.clone()],
            Self::OAuthAuthorizationCode { client_secret, .. } => {
                client_secret.clone().into_iter().collect()
            }
            Self::OAuthClientCredentials { client_secret, .. } => {
                vec![client_secret.clone()]
            }
        }
    }
}

impl fmt::Debug for McpReconnect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None { url } => f.debug_struct("None").field("url", url).finish(),
            Self::Bearer { url, .. } => f
                .debug_struct("Bearer")
                .field("url", url)
                .field("bearer_token", &"[redacted]")
                .finish(),
            Self::OAuthAuthorizationCode {
                url,
                client_id,
                client_secret,
                scope,
            } => f
                .debug_struct("OAuthAuthorizationCode")
                .field("url", url)
                .field("client_id", client_id)
                .field(
                    "client_secret",
                    &client_secret.as_ref().map(|_| "[redacted]"),
                )
                .field("scope", scope)
                .finish(),
            Self::OAuthClientCredentials {
                url,
                client_id,
                scope,
                ..
            } => f
                .debug_struct("OAuthClientCredentials")
                .field("url", url)
                .field("client_id", client_id)
                .field("client_secret", &"[redacted]")
                .field("scope", scope)
                .finish(),
        }
    }
}

/// Request state of [`McpConnections::list`].
#[derive(Clone, Debug)]
pub struct ListMcpConnections;
impl_request!(ListMcpConnections => McpConnectionList; encode = none; decode = credential_safe_json);

/// Request state of [`McpConnections::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct CreateMcpConnection {
    connection: NewMcpConnection,
}
impl_request!(CreateMcpConnection => McpConnection; encode = json; decode = credential_safe_json;
    sensitive = (|request: &Self| request.connection.secrets()));

/// Request state of [`McpConnections::get`].
#[derive(Clone, Debug)]
pub struct GetMcpConnection;
impl_request!(GetMcpConnection => McpConnection; encode = none; decode = credential_safe_json);

/// Request state of [`McpConnections::update`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpConnection {
    name: String,
}
impl_request!(UpdateMcpConnection => McpConnection; encode = json; decode = credential_safe_json);

/// Request state of [`McpConnections::delete`].
#[derive(Clone, Debug)]
pub struct DeleteMcpConnection;
impl_request!(DeleteMcpConnection => (); encode = none; decode = none);

/// Request state of [`McpConnections::test`].
#[derive(Clone, Debug)]
pub struct TestMcpConnection;
impl_request!(TestMcpConnection => McpConnectionTest; encode = none; decode = credential_safe_json);

/// Request state of [`McpConnections::reconnect`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ReconnectMcpConnection {
    auth: McpReconnect,
}
impl_request!(ReconnectMcpConnection => McpConnectionReconnectResult; encode = json; decode = credential_safe_json;
    sensitive = (|request: &Self| request.auth.secrets()));

/// An MCP connection. Its secrets are never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnection {
    /// The connection's ID.
    pub id: McpConnectionId,
    /// The display name.
    pub name: String,
    /// The MCP server URL.
    pub url: String,
    /// How Blazing Agents authenticates to the server.
    pub auth_type: McpAuthType,
    /// Whether the connection is usable.
    pub status: McpConnectionStatus,
    /// The last characters of the stored credential, if any.
    pub credential_fragment: Option<String>,
    /// Why the connection last failed, if it did.
    pub last_auth_error_code: Option<McpConnectionErrorCode>,
    /// The OAuth issuer discovered on the server, if any.
    pub oauth_issuer: Option<String>,
    /// The OAuth resource discovered on the server, if any.
    pub oauth_resource: Option<String>,
    /// When the current access token expires, if any.
    pub token_expires_at: Option<Timestamp>,
    /// When the connection was created.
    pub created_at: Timestamp,
    /// When the connection last changed.
    pub updated_at: Timestamp,
}

/// The tenant's MCP connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionList {
    /// Every MCP connection.
    pub mcp_connections: Vec<McpConnection>,
}

/// The outcome of testing an MCP connection. With `ok` false the `error`
/// field explains the failure; with `ok` true the other fields describe the
/// server.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionTest {
    /// Whether the connection works.
    pub ok: bool,
    /// The round-trip time.
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// The server identity it reported.
    #[serde(default)]
    pub server: Option<McpServerInfo>,
    /// How many tools the server offers.
    #[serde(default)]
    pub tool_count: Option<u64>,
    /// The names of the tools the server offers.
    #[serde(default)]
    pub tool_names: Option<Vec<String>>,
    /// Why the test failed.
    #[serde(default)]
    pub error: Option<McpConnectionTestError>,
}

/// The identity an MCP server reports.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    /// The server's name.
    pub name: String,
    /// The server's version.
    pub version: String,
}

/// Why an MCP connection test failed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionTestError {
    /// The failure kind.
    pub code: McpConnectionErrorCode,
    /// A human-readable explanation.
    pub message: String,
}

/// The outcome of reconnecting an MCP connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionReconnectResult {
    /// `connected`, or `needs_auth` when an admin must sign in through the
    /// dashboard.
    pub status: McpConnectionStatus,
    /// The updated connection.
    pub connection: McpConnection,
}
