//! MCP connections. Tenant only; start from [`Client::mcp_connections`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::McpConnectionId;
use crate::op::Op;

/// MCP connection operations. Returned by [`Client::mcp_connections`].
#[derive(Clone, Copy, Debug)]
pub struct McpConnections<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// MCP connection operations.
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

    /// Replaces an MCP connection's URL and credentials.
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

/// The URL and credentials of a new MCP connection: one variant per `authType`.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(untagged)]
pub enum NewMcpConnection {}

/// The URL and credentials to reconnect with: one variant per `authType`.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(untagged)]
pub enum McpReconnect {}

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
impl_request!(CreateMcpConnection => McpConnection; encode = json; decode = credential_safe_json);

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
impl_request!(ReconnectMcpConnection => McpConnectionReconnectResult; encode = json; decode = credential_safe_json);

/// An MCP connection. Its secrets are never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnection {}

/// The tenant's MCP connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionList {}

/// The outcome of testing an MCP connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionTest {}

/// The outcome of reconnecting an MCP connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpConnectionReconnectResult {}
