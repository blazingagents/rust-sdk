//! Memories of one agent. Start from [`Client::memories`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/memory>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, Raw, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, MemoryId, TenantId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::types::Timestamp;

/// Memory operations. Returned by [`Client::memories`].
#[derive(Clone, Copy, Debug)]
pub struct Memories<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Memory operations.
    pub fn memories(&self) -> Memories<'_, S> {
        Memories { client: self }
    }
}

impl<S: Scope> Memories<'_, S> {
    /// Lists an agent's memories.
    pub fn list(&self, agent_id: impl Into<AgentId>) -> Call<ListMemories> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::ListMemories {
                agent_id: agent_id.as_str(),
            },
            ListMemories::default(),
        )
    }

    /// Stores a memory. `text` is non-empty and at most 10 KiB, and an agent
    /// holds at most 500 memories: creating one when the agent is full can
    /// remove the memory used least recently.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// let memory = client
    ///     .memories()
    ///     .create("ag_4kP9sT2vXq7LmN3a", "Prefers release notes under five lines.")
    ///     .user_id("customer_123")
    ///     .await?;
    /// println!("{}", memory.id);
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(
        &self,
        agent_id: impl Into<AgentId>,
        text: impl Into<String>,
    ) -> Call<CreateMemory> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::CreateMemory {
                agent_id: agent_id.as_str(),
            },
            CreateMemory {
                text: text.into(),
                user_id: None,
            },
        )
    }

    /// Gets one memory. Does not change
    /// [`last_accessed_at`](Memory::last_accessed_at).
    pub fn get(
        &self,
        agent_id: impl Into<AgentId>,
        memory_id: impl Into<MemoryId>,
    ) -> Call<GetMemory> {
        let agent_id = agent_id.into();
        let memory_id = memory_id.into();
        Call::new(
            self.client,
            Op::GetMemory {
                agent_id: agent_id.as_str(),
                memory_id: memory_id.as_str(),
            },
            GetMemory,
        )
    }

    /// Replaces a memory's whole text. Updates `updated_at` and
    /// [`last_accessed_at`](Memory::last_accessed_at).
    pub fn update(
        &self,
        agent_id: impl Into<AgentId>,
        memory_id: impl Into<MemoryId>,
        text: impl Into<String>,
    ) -> Call<UpdateMemory> {
        let agent_id = agent_id.into();
        let memory_id = memory_id.into();
        Call::new(
            self.client,
            Op::UpdateMemory {
                agent_id: agent_id.as_str(),
                memory_id: memory_id.as_str(),
            },
            UpdateMemory { text: text.into() },
        )
    }

    /// Permanently deletes a memory.
    pub fn delete(
        &self,
        agent_id: impl Into<AgentId>,
        memory_id: impl Into<MemoryId>,
    ) -> Call<DeleteMemory> {
        let agent_id = agent_id.into();
        let memory_id = memory_id.into();
        Call::new(
            self.client,
            Op::DeleteMemory {
                agent_id: agent_id.as_str(),
                memory_id: memory_id.as_str(),
            },
            DeleteMemory,
        )
    }
}

/// Request state of [`Memories::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMemories {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListMemories => Page<Memory>; encode = query; decode = json; paginate);

impl Call<ListMemories> {
    /// Only memories with exactly this `user_id`. An empty string selects
    /// general memories; leave it out for all memories.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Only memories whose text matches these words. An empty string is
    /// ignored.
    pub fn search(self, search: impl Into<String>) -> Self {
        self.map(|request| {
            let search = search.into();
            request.search = (!search.is_empty()).then_some(search);
        })
    }

    /// Memories per page, 1 to 100. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Memories::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMemory {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
}
impl_request!(CreateMemory => Memory; encode = json; decode = (unwrap_memory));

impl Call<CreateMemory> {
    /// The end user the memory belongs to. Fixed after creation; an empty or
    /// omitted value creates a general memory, which unscoped turns see. A turn
    /// made through `for_user` sees only its end user's memories.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }
}

/// Request state of [`Memories::get`].
#[derive(Clone, Debug)]
pub struct GetMemory;
impl_request!(GetMemory => Memory; encode = none; decode = (unwrap_memory));

/// Request state of [`Memories::update`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemory {
    text: String,
}
impl_request!(UpdateMemory => Memory; encode = json; decode = (unwrap_memory));

/// Request state of [`Memories::delete`].
#[derive(Clone, Debug)]
pub struct DeleteMemory;
impl_request!(DeleteMemory => (); encode = none; decode = none);

/// A memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    /// The memory's ID.
    pub id: MemoryId,
    /// The tenant that owns the memory.
    pub tenant_id: TenantId,
    /// The agent the memory belongs to.
    pub agent_id: AgentId,
    /// The end user the memory belongs to, or an empty string for a general
    /// memory, which only unscoped turns see.
    pub user_id: String,
    /// The remembered text.
    pub text: String,
    /// When the memory was created.
    pub created_at: Timestamp,
    /// When the memory last changed.
    pub updated_at: Timestamp,
    /// When the memory was last used. Reading it with [`Memories::get`] or
    /// [`Memories::list`] does not count.
    pub last_accessed_at: Timestamp,
}

// Create, get and update wrap the memory in `{"memory": ...}`; list items do not.
#[derive(Deserialize)]
struct MemoryEnvelope {
    memory: Memory,
}

async fn unwrap_memory(raw: Raw) -> crate::Result<Memory> {
    raw.json::<MemoryEnvelope>()
        .await
        .map(|envelope| envelope.memory)
}
