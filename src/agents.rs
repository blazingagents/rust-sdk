//! Agents. Start from [`Client::agents`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/agents>.

use serde::{Deserialize, Serialize};

use crate::agent_tools::BuiltinToolName;
use crate::call::{Call, Payload, impl_request};
use crate::client::{Client, Scope, Tenant};
use crate::error::Result;
use crate::files::Upload;
use crate::ids::{AgentId, McpConnectionId, ProviderId, TenantId, WorkspaceId};
use crate::op::{Op, TenantOp};
use crate::pagination::{Cursor, Page};
use crate::patch::Patch;
use crate::spending_limits::{SpendingLimitInput, SpendingLimitStatus};
use crate::types::{Metadata, Timestamp};
use crate::workspaces::WorkspaceTier;

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
    /// Lists agents, newest first. Without filters it returns every agent.
    pub fn list(&self) -> Call<ListAgents> {
        Call::new(self.client, Op::ListAgents, ListAgents::default())
    }

    /// Creates an agent. `name` is 1 to 80 characters. With no other input the
    /// agent starts `active` on the server defaults: a new Core workspace, no
    /// model, every built-in tool off and `full` approval.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// let agent = client
    ///     .agents()
    ///     .create("Support writer")
    ///     .model("prv_7Tn4Kd9QwE2sLx5R", "openai/gpt-6-luna")
    ///     .instructions("Answer clearly and briefly.")
    ///     .await?;
    /// assert_eq!(agent.status, blazing_agents::agents::AgentStatus::Active);
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(&self, name: impl Into<String>) -> Call<CreateAgent> {
        Call::new(
            self.client,
            Op::CreateAgent,
            CreateAgent {
                name: name.into(),
                model: None,
                thinking_level: None,
                provider_id: None,
                workspace_id: None,
                workspace_tier: None,
                auto_compaction: None,
                compaction_reserve_tokens: None,
                memory_injection_enabled: None,
                tools: None,
                approval_in_chat: None,
                approval_in_tasks: None,
                instructions: None,
                user_id: None,
                metadata: None,
                mcp_connection_ids: None,
            },
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

    /// Changes an agent. Send at least one field. Fields you do not set keep
    /// their values, and arrays, `metadata` and approval policies replace the
    /// current value. New sessions and task runs pick up the change; existing
    /// sessions keep their saved configuration.
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

    /// Permanently deletes an agent, its sessions, tasks, memories, skills and
    /// linked prompts. `include_artifacts` chooses whether its artifacts are
    /// deleted too. Its workspace, provider and MCP connections stay. While it
    /// has a running turn or a queued or running task run the request fails
    /// with `agent_busy`; stop that work or wait for it, then delete again.
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

    /// Stops an agent from accepting new turns; turns already running finish.
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

    /// Sets the agent's avatar image: a PNG, JPEG or WebP of 512 KiB or less.
    /// The part's Content-Type comes from the file name's extension (or
    /// [`Upload::content_type`]), so [`Upload::file`] works directly and
    /// bytes need a [`file_name`](Upload::file_name) such as `avatar.png`.
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

    /// Lists the MCP connections attached to the agent and what the agent
    /// forwards to each one.
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

    /// Changes what the agent forwards to one attached MCP connection. Fields
    /// you do not set keep their values.
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
}

impl Agents<'_, Tenant> {
    /// Gets the agent's model spending limit and its current period.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.agents().get_spending_limit("ag_4kP9sT2vXq7LmN3a");
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::Client) {
    /// let limit = client.agents().get_spending_limit("ag_4kP9sT2vXq7LmN3a");
    /// # }
    /// ```
    pub fn get_spending_limit(&self, agent_id: impl Into<AgentId>) -> Call<GetAgentSpendingLimit> {
        let agent_id = agent_id.into();
        Call::tenant(
            self.client,
            TenantOp::GetAgentSpendingLimit {
                agent_id: agent_id.as_str(),
            },
            GetAgentSpendingLimit,
        )
    }

    /// Sets the agent's model spending limit, or removes it with `None`.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.agents().update_spending_limit("ag_4kP9sT2vXq7LmN3a", None);
    /// # }
    /// ```
    pub fn update_spending_limit(
        &self,
        agent_id: impl Into<AgentId>,
        limit: Option<SpendingLimitInput>,
    ) -> Call<UpdateAgentSpendingLimit> {
        let agent_id = agent_id.into();
        Call::tenant(
            self.client,
            TenantOp::UpdateAgentSpendingLimit {
                agent_id: agent_id.as_str(),
            },
            UpdateAgentSpendingLimit {
                spending_limit: limit,
            },
        )
    }
}

crate::open_enum! {
    /// Whether an agent accepts new turns.
    pub enum AgentStatus {
        /// `active`
        Active = "active",
        /// `disabled`; turns already running finish.
        Disabled = "disabled",
    }
}

crate::open_enum! {
    /// A built-in agent tool. Each tool is enabled individually through
    /// `tools`, and an approval override names one through
    /// [`ToolReference::builtin`].
    pub enum AgentTool {
        /// `read` — read a file from the workspace.
        Read = "read",
        /// `write` — create or overwrite a file in the workspace.
        Write = "write",
        /// `edit` — replace exact text in a workspace file.
        Edit = "edit",
        /// `grep` — search workspace file contents with a regular expression.
        Grep = "grep",
        /// `glob` — find workspace files by path pattern.
        Glob = "glob",
        /// `bash` — run shell commands in the workspace.
        Bash = "bash",
        /// `publish_artifacts` — publish workspace files as downloadable
        /// artifacts.
        PublishArtifacts = "publish_artifacts",
        /// `write_todos` — plan and track multi-step work with a todo list.
        WriteTodos = "write_todos",
        /// `save_memory` — save a durable note.
        SaveMemory = "save_memory",
        /// `get_memory` — read one saved note.
        GetMemory = "get_memory",
        /// `search_memories` — search saved notes.
        SearchMemories = "search_memories",
        /// `update_memory` — update a saved note.
        UpdateMemory = "update_memory",
        /// `delete_memory` — delete a saved note.
        DeleteMemory = "delete_memory",
        /// `web_search` — search the public web; each search consumes usage
        /// credit.
        WebSearch = "web_search",
    }
}

crate::open_enum! {
    /// What happens to a tool call under an [`ApprovalPolicy`].
    pub enum ApprovalDecision {
        /// `full` — the call runs without approval.
        Full = "full",
        /// `deny` — the call is blocked.
        Deny = "deny",
        /// `manual` — the call waits for a person to approve or deny it.
        Manual = "manual",
        /// `auto` — the agent's model allows the call, denies it, or asks a
        /// person; if the review fails, the call is blocked.
        Auto = "auto",
    }
}

/// Request state of [`Agents::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgents {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<WorkspaceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListAgents => Page<Agent>; encode = query; decode = json; paginate);

impl Call<ListAgents> {
    /// Only agents of this end user. An empty string selects tenant-level
    /// agents; leaving it unset returns all agents.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Only agents that use this workspace.
    pub fn workspace_id(self, workspace_id: impl Into<WorkspaceId>) -> Self {
        self.map(|request| request.workspace_id = Some(workspace_id.into()))
    }

    /// Agents per page, 1 to 100. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Agents::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgent {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_id: Option<ProviderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<WorkspaceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_tier: Option<WorkspaceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_compaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_injection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AgentTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_in_chat: Option<ApprovalPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_in_tasks: Option<ApprovalPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mcp_connection_ids: Option<Vec<McpConnectionId>>,
}
impl_request!(CreateAgent => Agent; encode = json; decode = json);

impl Call<CreateAgent> {
    /// The model the agent runs: the provider whose key runs it and the
    /// provider's own model ID, such as `openai/gpt-6-luna`. Send the two
    /// together; an agent without a model can be saved but cannot answer.
    pub fn model(self, provider_id: impl Into<ProviderId>, model: impl Into<String>) -> Self {
        self.map(|request| {
            request.provider_id = Some(provider_id.into());
            request.model = Some(model.into());
        })
    }

    /// How much reasoning to request from the model, such as `high`. Needs a
    /// provider and model, and must be a level the model supports.
    pub fn thinking_level(self, thinking_level: impl Into<String>) -> Self {
        self.map(|request| request.thinking_level = Some(thinking_level.into()))
    }

    /// An existing workspace the agent shares instead of getting a new one.
    /// Mutually exclusive with [`workspace_tier`](Self::workspace_tier); the
    /// setter called last wins.
    pub fn workspace_id(self, workspace_id: impl Into<WorkspaceId>) -> Self {
        self.map(|request| {
            request.workspace_id = Some(workspace_id.into());
            request.workspace_tier = None;
        })
    }

    /// The tier of the workspace created for the agent, defaulting to Core.
    /// Mutually exclusive with [`workspace_id`](Self::workspace_id); the
    /// setter called last wins.
    pub fn workspace_tier(self, workspace_tier: WorkspaceTier) -> Self {
        self.map(|request| {
            request.workspace_tier = Some(workspace_tier);
            request.workspace_id = None;
        })
    }

    /// Summarize older context automatically when a conversation nears the
    /// model's context limit. Defaults to `true`.
    pub fn auto_compaction(self, auto_compaction: bool) -> Self {
        self.map(|request| request.auto_compaction = Some(auto_compaction))
    }

    /// Tokens to keep free for the model's reply when deciding whether to
    /// compact. Defaults to 16384.
    pub fn compaction_reserve_tokens(self, compaction_reserve_tokens: u64) -> Self {
        self.map(|request| request.compaction_reserve_tokens = Some(compaction_reserve_tokens))
    }

    /// Add the agent's saved memories to every turn automatically. Defaults to
    /// `false`.
    pub fn memory_injection_enabled(self, memory_injection_enabled: bool) -> Self {
        self.map(|request| request.memory_injection_enabled = Some(memory_injection_enabled))
    }

    /// The built-in tools the agent can use. Each is off until listed.
    /// Defaults to none.
    pub fn tools(self, tools: impl IntoIterator<Item = AgentTool>) -> Self {
        self.map(|request| request.tools = Some(tools.into_iter().collect()))
    }

    /// Which tool calls need approval in sessions and stateless generation.
    /// Defaults to [`ApprovalPolicy::default`].
    pub fn approval_in_chat(self, approval_in_chat: ApprovalPolicy) -> Self {
        self.map(|request| request.approval_in_chat = Some(approval_in_chat))
    }

    /// Which tool calls need approval in task runs. Defaults to
    /// [`ApprovalPolicy::default`].
    pub fn approval_in_tasks(self, approval_in_tasks: ApprovalPolicy) -> Self {
        self.map(|request| request.approval_in_tasks = Some(approval_in_tasks))
    }

    /// Instructions the agent follows on every turn, up to 3000 characters.
    pub fn instructions(self, instructions: impl Into<String>) -> Self {
        self.map(|request| request.instructions = Some(instructions.into()))
    }

    /// Your end user's ID, used for attribution. An empty string (the default)
    /// means a tenant-level agent. It cannot change after creation.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, returned unchanged.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// The MCP connections attached to the agent, up to 10.
    pub fn mcp_connection_ids(
        self,
        ids: impl IntoIterator<Item = impl Into<McpConnectionId>>,
    ) -> Self {
        self.map(|request| {
            request.mcp_connection_ids = Some(ids.into_iter().map(Into::into).collect())
        })
    }
}

/// Request state of [`Agents::get`].
#[derive(Clone, Debug)]
pub struct GetAgent;
impl_request!(GetAgent => Agent; encode = none; decode = json);

/// Request state of [`Agents::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgent {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    provider_id: Patch<ProviderId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    model: Patch<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    thinking_level: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<WorkspaceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_compaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_injection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AgentTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_in_chat: Option<ApprovalPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_in_tasks: Option<ApprovalPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mcp_connection_ids: Option<Vec<McpConnectionId>>,
}
impl_request!(UpdateAgent => Agent; encode = json; decode = json);

impl Call<UpdateAgent> {
    /// A new display name, 1 to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// Moves the agent to a new model: the provider whose key runs it and the
    /// provider's own model ID, sent together.
    pub fn model(self, provider_id: impl Into<ProviderId>, model: impl Into<String>) -> Self {
        self.map(|request| {
            request.provider_id = Patch::Set(provider_id.into());
            request.model = Patch::Set(model.into());
        })
    }

    /// A new model on the agent's current provider, which is kept.
    pub fn model_on_current_provider(self, model: impl Into<String>) -> Self {
        self.map(|request| request.model = Patch::Set(model.into()))
    }

    /// Removes the provider, model and thinking level together. The server
    /// rejects removing the provider while a thinking level remains.
    pub fn clear_model(self) -> Self {
        self.map(|request| {
            request.provider_id = Patch::Clear;
            request.model = Patch::Clear;
            request.thinking_level = Patch::Clear;
        })
    }

    /// How much reasoning to request from the model, such as `high`. Needs a
    /// provider and model, and must be a level the model supports.
    pub fn thinking_level(self, thinking_level: impl Into<String>) -> Self {
        self.map(|request| request.thinking_level = Patch::Set(thinking_level.into()))
    }

    /// Removes the thinking level; the model decides how much to reason.
    pub fn clear_thinking_level(self) -> Self {
        self.map(|request| request.thinking_level = Patch::Clear)
    }

    /// Moves the agent to another workspace.
    pub fn workspace_id(self, workspace_id: impl Into<WorkspaceId>) -> Self {
        self.map(|request| request.workspace_id = Some(workspace_id.into()))
    }

    /// Summarize older context automatically when a conversation nears the
    /// model's context limit.
    pub fn auto_compaction(self, auto_compaction: bool) -> Self {
        self.map(|request| request.auto_compaction = Some(auto_compaction))
    }

    /// Tokens to keep free for the model's reply when deciding whether to
    /// compact.
    pub fn compaction_reserve_tokens(self, compaction_reserve_tokens: u64) -> Self {
        self.map(|request| request.compaction_reserve_tokens = Some(compaction_reserve_tokens))
    }

    /// Add the agent's saved memories to every turn automatically.
    pub fn memory_injection_enabled(self, memory_injection_enabled: bool) -> Self {
        self.map(|request| request.memory_injection_enabled = Some(memory_injection_enabled))
    }

    /// The built-in tools the agent can use. Replaces the current list.
    pub fn tools(self, tools: impl IntoIterator<Item = AgentTool>) -> Self {
        self.map(|request| request.tools = Some(tools.into_iter().collect()))
    }

    /// Which tool calls need approval in sessions and stateless generation.
    /// Replaces the current policy.
    pub fn approval_in_chat(self, approval_in_chat: ApprovalPolicy) -> Self {
        self.map(|request| request.approval_in_chat = Some(approval_in_chat))
    }

    /// Which tool calls need approval in task runs. Replaces the current
    /// policy.
    pub fn approval_in_tasks(self, approval_in_tasks: ApprovalPolicy) -> Self {
        self.map(|request| request.approval_in_tasks = Some(approval_in_tasks))
    }

    /// New instructions the agent follows on every turn, up to 3000
    /// characters.
    pub fn instructions(self, instructions: impl Into<String>) -> Self {
        self.map(|request| request.instructions = Some(instructions.into()))
    }

    /// Replaces the agent's metadata completely.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// The MCP connections attached to the agent, up to 10. Replaces the
    /// current list.
    pub fn mcp_connection_ids(
        self,
        ids: impl IntoIterator<Item = impl Into<McpConnectionId>>,
    ) -> Self {
        self.map(|request| {
            request.mcp_connection_ids = Some(ids.into_iter().map(Into::into).collect())
        })
    }
}

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
pub struct UpdateAgentMcpAttachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    forward_user_id: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    forwarded_metadata_keys: Option<Vec<String>>,
}
impl_request!(UpdateAgentMcpAttachment => McpAttachment; encode = json; decode = json);

impl Call<UpdateAgentMcpAttachment> {
    /// Send the session's `userId` to the MCP server with every tool call.
    pub fn forward_user_id(self, forward_user_id: bool) -> Self {
        self.map(|request| request.forward_user_id = Some(forward_user_id))
    }

    /// Session metadata keys whose values are sent to the MCP server with
    /// every tool call, up to 32 keys of 1 to 64 characters. Replaces the
    /// current list.
    pub fn forwarded_metadata_keys(
        self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.map(|request| {
            request.forwarded_metadata_keys = Some(keys.into_iter().map(Into::into).collect())
        })
    }
}

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

/// Which tool calls need approval: a default decision plus per-tool
/// overrides. A matching override wins over the default.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPolicy {
    /// The decision for calls no override matches.
    pub default: ApprovalDecision,
    /// Per-tool decisions. Each tool can appear once.
    #[serde(default)]
    pub overrides: Vec<ApprovalOverride>,
}

impl ApprovalPolicy {
    /// A policy that applies `default` to every tool call.
    pub fn new(default: ApprovalDecision) -> Self {
        Self {
            default,
            overrides: Vec::new(),
        }
    }

    /// Adds a per-tool decision that wins over the policy's default.
    #[must_use]
    pub fn with_override(mut self, tool: ToolReference, decision: ApprovalDecision) -> Self {
        self.overrides.push(ApprovalOverride::new(tool, decision));
        self
    }
}

impl Default for ApprovalPolicy {
    /// The server default: `full` with no overrides.
    fn default() -> Self {
        Self::new(ApprovalDecision::Full)
    }
}

/// One per-tool decision in an [`ApprovalPolicy`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ApprovalOverride {
    /// The tool the decision applies to.
    pub tool: ToolReference,
    /// The decision for matching calls.
    pub decision: ApprovalDecision,
}

impl ApprovalOverride {
    /// An override applying `decision` to calls of `tool`.
    pub fn new(tool: ToolReference, decision: ApprovalDecision) -> Self {
        Self { tool, decision }
    }
}

/// The tool an approval override or a tool approval refers to: a built-in
/// tool by name, one tool of an attached MCP connection, or, in tool
/// approvals only, a backend function.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolReference {
    /// A built-in tool by name.
    #[non_exhaustive]
    Builtin {
        /// The tool's name.
        name: BuiltinToolName,
    },
    /// One tool of an MCP connection attached to the agent.
    #[non_exhaustive]
    Mcp {
        /// The MCP connection's ID.
        #[serde(rename = "connectionId")]
        connection_id: McpConnectionId,
        /// The tool's name on the MCP server.
        name: String,
    },
    /// A function a backend supplied for one chat request. Only tool
    /// approvals name one; approval overrides cannot.
    #[non_exhaustive]
    Function {
        /// The function's name.
        name: String,
    },
    /// A tool kind this version of the SDK does not know; the raw object,
    /// re-serialized unchanged.
    #[serde(untagged)]
    Unrecognized(serde_json::Value),
}

impl ToolReference {
    /// A built-in tool by name.
    pub fn builtin(name: impl Into<BuiltinToolName>) -> Self {
        Self::Builtin { name: name.into() }
    }

    /// One tool of an attached MCP connection: the connection's ID and the
    /// tool's name on its server.
    pub fn mcp(connection_id: impl Into<McpConnectionId>, name: impl Into<String>) -> Self {
        Self::Mcp {
            connection_id: connection_id.into(),
            name: name.into(),
        }
    }
}

impl<'de> Deserialize<'de> for ToolReference {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// The known wire shapes; a malformed one is a decode error, not
        /// `Unrecognized`.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "lowercase")]
        enum Known {
            Builtin {
                name: BuiltinToolName,
            },
            Mcp {
                #[serde(rename = "connectionId")]
                connection_id: McpConnectionId,
                name: String,
            },
            Function {
                name: String,
            },
        }

        crate::types::decode_tagged(
            deserializer,
            "type",
            |tag, raw| match tag {
                "builtin" | "mcp" | "function" => {
                    Some(Known::deserialize(raw).map(|known| match known {
                        Known::Builtin { name } => Self::Builtin { name },
                        Known::Mcp {
                            connection_id,
                            name,
                        } => Self::Mcp {
                            connection_id,
                            name,
                        },
                        Known::Function { name } => Self::Function { name },
                    }))
                }
                _ => None,
            },
            |raw| Self::Unrecognized(serde_json::Value::Object(raw)),
        )
    }
}

/// An agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    /// The agent's ID.
    pub id: AgentId,
    /// The tenant that owns the agent.
    pub tenant_id: TenantId,
    /// The display name.
    pub name: String,
    /// The provider's own model ID, such as `openai/gpt-6-luna`, if the agent
    /// has a model.
    pub model: Option<String>,
    /// The requested reasoning level, such as `high`, if set.
    pub thinking_level: Option<String>,
    /// The provider whose key runs the model, if the agent has one.
    pub provider_id: Option<ProviderId>,
    /// The workspace the agent reads and writes files in.
    pub workspace_id: WorkspaceId,
    /// Whether older context is summarized automatically when a conversation
    /// nears the model's context limit.
    pub auto_compaction: bool,
    /// Tokens kept free for the model's reply when deciding whether to
    /// compact.
    pub compaction_reserve_tokens: u64,
    /// Whether the agent's saved memories are added to every turn.
    pub memory_injection_enabled: bool,
    /// The built-in tools the agent can use.
    pub tools: Vec<AgentTool>,
    /// Which tool calls need approval in sessions and stateless generation.
    #[serde(default)]
    pub approval_in_chat: ApprovalPolicy,
    /// Which tool calls need approval in task runs.
    #[serde(default)]
    pub approval_in_tasks: ApprovalPolicy,
    /// The instructions the agent follows on every turn.
    pub instructions: String,
    /// The end user the agent belongs to, or an empty string for a
    /// tenant-level agent.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// The MCP connections attached to the agent.
    pub mcp_connection_ids: Vec<McpConnectionId>,
    /// A short-lived signed URL of the avatar image, if the agent has one.
    pub avatar_url: Option<String>,
    /// When the agent was created.
    pub created_at: Timestamp,
    /// When the agent last changed.
    pub updated_at: Timestamp,
    /// Whether the agent accepts new turns.
    pub status: AgentStatus,
}

/// One MCP connection attached to an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpAttachment {
    /// The attached MCP connection's ID.
    pub mcp_connection_id: McpConnectionId,
    /// Whether the session's `userId` is sent to the MCP server with every
    /// tool call.
    pub forward_user_id: bool,
    /// The session metadata keys whose values are sent to the MCP server with
    /// every tool call.
    pub forwarded_metadata_keys: Vec<String>,
    /// When the attachment was created.
    pub created_at: Timestamp,
    /// When the attachment last changed.
    pub updated_at: Timestamp,
}

/// The MCP connections attached to an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct McpAttachmentList {
    /// The attachments, one per attached connection.
    pub mcp_attachments: Vec<McpAttachment>,
}

/// The agent configuration a session or task run saved when it started.
/// Later changes to the agent do not change it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// The display name.
    pub name: String,
    /// The provider's own model ID, such as `openai/gpt-6-luna`, if the agent
    /// had a model.
    pub model: Option<String>,
    /// The requested reasoning level, such as `high`, if set.
    pub thinking_level: Option<String>,
    /// The provider whose key ran the model, if the agent had one.
    pub provider_id: Option<ProviderId>,
    /// Whether older context was summarized automatically when a conversation
    /// neared the model's context limit.
    pub auto_compaction: bool,
    /// Tokens kept free for the model's reply when deciding whether to
    /// compact.
    pub compaction_reserve_tokens: u64,
    /// Whether the agent's saved memories were added to every turn.
    pub memory_injection_enabled: bool,
    /// The built-in tools the agent could use.
    pub tools: Vec<AgentTool>,
    /// Which tool calls needed approval in sessions and stateless generation.
    #[serde(default)]
    pub approval_in_chat: ApprovalPolicy,
    /// Which tool calls needed approval in task runs.
    #[serde(default)]
    pub approval_in_tasks: ApprovalPolicy,
    /// The instructions the agent followed on every turn.
    pub instructions: String,
    /// Your own data.
    pub metadata: Metadata,
    /// The MCP connections attached to the agent.
    pub mcp_connection_ids: Vec<McpConnectionId>,
}
