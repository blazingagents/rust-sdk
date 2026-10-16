//! Agents. Start from [`Client::agents`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, Payload, impl_request};
use crate::client::{Client, Scope};
use crate::error::Result;
use crate::files::Upload;
use crate::ids::{AgentId, McpConnectionId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::spending_limits::{SpendingLimitInput, SpendingLimitStatus};

/// Agent operations. Returned by [`Client::agents`].
#[derive(Clone, Copy, Debug)]
pub struct Agents<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Agent operations.
    pub fn agents(&self) -> Agents<'_, S> {
        Agents { client: self }
    }
}

impl<S: Scope> Agents<'_, S> {
    /// Lists agents, newest first.
    pub fn list(&self) -> Call<ListAgents> {
        Call::new(self.client, Op::ListAgents, ListAgents::default())
    }

    /// Creates an agent.
    pub fn create(&self, name: impl Into<String>) -> Call<CreateAgent> {
        Call::new(
            self.client,
            Op::CreateAgent,
            CreateAgent { name: name.into() },
        )
    }

    /// Gets one agent.
    pub fn get(&self, agent_id: impl Into<AgentId>) -> Call<GetAgent> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::GetAgent {
                agent_id: agent_id.as_str(),
            },
            GetAgent,
        )
    }

    /// Changes an agent. Fields you do not set keep their values.
    pub fn update(&self, agent_id: impl Into<AgentId>) -> Call<UpdateAgent> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::UpdateAgent {
                agent_id: agent_id.as_str(),
            },
            UpdateAgent::default(),
        )
    }

    /// Deletes an agent and, with `include_artifacts`, its artifacts.
    pub fn delete(
        &self,
        agent_id: impl Into<AgentId>,
        include_artifacts: bool,
    ) -> Call<DeleteAgent> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::DeleteAgent {
                agent_id: agent_id.as_str(),
            },
            DeleteAgent { include_artifacts },
        )
    }

    /// Stops an agent from accepting turns.
    pub fn disable(&self, agent_id: impl Into<AgentId>) -> Call<DisableAgent> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::DisableAgent {
                agent_id: agent_id.as_str(),
            },
            DisableAgent,
        )
    }

    /// Lets a disabled agent accept turns again.
    pub fn enable(&self, agent_id: impl Into<AgentId>) -> Call<EnableAgent> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::EnableAgent {
                agent_id: agent_id.as_str(),
            },
            EnableAgent,
        )
    }

    /// Sets the agent's avatar image.
    pub fn upload_avatar(
        &self,
        agent_id: impl Into<AgentId>,
        avatar: Upload,
    ) -> Call<UploadAgentAvatar> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::UploadAgentAvatar {
                agent_id: agent_id.as_str(),
            },
            UploadAgentAvatar { avatar },
        )
    }

    /// Removes the agent's avatar image.
    pub fn remove_avatar(&self, agent_id: impl Into<AgentId>) -> Call<DeleteAgentAvatar> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::DeleteAgentAvatar {
                agent_id: agent_id.as_str(),
            },
            DeleteAgentAvatar,
        )
    }

    /// Lists the agent's MCP connection attachments.
    pub fn list_mcp_attachments(
        &self,
        agent_id: impl Into<AgentId>,
    ) -> Call<ListAgentMcpAttachments> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::ListAgentMcpAttachments {
                agent_id: agent_id.as_str(),
            },
            ListAgentMcpAttachments,
        )
    }

    /// Changes how the agent forwards user context to one MCP connection.
    pub fn update_mcp_attachment(
        &self,
        agent_id: impl Into<AgentId>,
        mcp_connection_id: impl Into<McpConnectionId>,
    ) -> Call<UpdateAgentMcpAttachment> {
        let agent_id = agent_id.into();
        let mcp_connection_id = mcp_connection_id.into();
        Call::new(
            self.client,
            Op::UpdateAgentMcpAttachment {
                agent_id: agent_id.as_str(),
                mcp_connection_id: mcp_connection_id.as_str(),
            },
            UpdateAgentMcpAttachment::default(),
        )
    }

    /// Gets the agent's model spending limit and its current period.
    pub fn get_spending_limit(&self, agent_id: impl Into<AgentId>) -> Call<GetAgentSpendingLimit> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::GetAgentSpendingLimit {
                agent_id: agent_id.as_str(),
            },
            GetAgentSpendingLimit,
        )
    }

    /// Sets the agent's model spending limit, or removes it with `None`.
    pub fn update_spending_limit(
        &self,
        agent_id: impl Into<AgentId>,
        limit: Option<SpendingLimitInput>,
    ) -> Call<UpdateAgentSpendingLimit> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::UpdateAgentSpendingLimit {
                agent_id: agent_id.as_str(),
            },
            UpdateAgentSpendingLimit {
                spending_limit: limit,
            },
        )
    }
}

/// Request state of [`Agents::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgents {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListAgents => Page<Agent>; encode = query; decode = json; paginate);

/// Request state of [`Agents::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgent {
    name: String,
}
impl_request!(CreateAgent => Agent; encode = json; decode = json);

/// Request state of [`Agents::get`].
#[derive(Clone, Debug)]
pub struct GetAgent;
impl_request!(GetAgent => Agent; encode = none; decode = json);

/// Request state of [`Agents::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgent {}
impl_request!(UpdateAgent => Agent; encode = json; decode = json);

/// Request state of [`Agents::delete`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAgent {
    include_artifacts: bool,
}
impl_request!(DeleteAgent => (); encode = query; decode = none);

/// Request state of [`Agents::disable`].
#[derive(Clone, Debug)]
pub struct DisableAgent;
impl_request!(DisableAgent => Agent; encode = none; decode = json);

/// Request state of [`Agents::enable`].
#[derive(Clone, Debug)]
pub struct EnableAgent;
impl_request!(EnableAgent => Agent; encode = none; decode = json);

/// Request state of [`Agents::upload_avatar`].
#[derive(Debug)]
pub struct UploadAgentAvatar {
    avatar: Upload,
}

impl UploadAgentAvatar {
    fn into_payload(self) -> Result<Payload> {
        let form = reqwest::multipart::Form::new().part("file", self.avatar.into_part()?);
        Ok(Payload::default().multipart(form))
    }
}
impl_request!(UploadAgentAvatar => Agent; encode = (Self::into_payload); decode = json);

/// Request state of [`Agents::remove_avatar`].
#[derive(Clone, Debug)]
pub struct DeleteAgentAvatar;
impl_request!(DeleteAgentAvatar => Agent; encode = none; decode = json);

/// Request state of [`Agents::list_mcp_attachments`].
#[derive(Clone, Debug)]
pub struct ListAgentMcpAttachments;
impl_request!(ListAgentMcpAttachments => McpAttachmentList; encode = none; decode = json);

/// Request state of [`Agents::update_mcp_attachment`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentMcpAttachment {}
impl_request!(UpdateAgentMcpAttachment => McpAttachment; encode = json; decode = json);

/// Request state of [`Agents::get_spending_limit`].
#[derive(Clone, Debug)]
pub struct GetAgentSpendingLimit;
impl_request!(GetAgentSpendingLimit => SpendingLimitStatus; encode = none; decode = json);

/// Request state of [`Agents::update_spending_limit`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentSpendingLimit {
    spending_limit: Option<SpendingLimitInput>,
}
impl_request!(UpdateAgentSpendingLimit => SpendingLimitStatus; encode = json; decode = json);

/// An agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Agent {}

/// One MCP connection attached to an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpAttachment {}

/// The MCP connections attached to an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpAttachmentList {}
