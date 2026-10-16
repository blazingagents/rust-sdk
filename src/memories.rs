//! Memories of one agent. Start from [`Client::memories`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, MemoryId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};

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

    /// Stores a memory.
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
            CreateMemory { text: text.into() },
        )
    }

    /// Gets one memory.
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

    /// Replaces a memory's text.
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

    /// Deletes a memory.
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
    cursor: Option<Cursor>,
}
impl_request!(ListMemories => Page<Memory>; encode = query; decode = json; paginate);

/// Request state of [`Memories::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMemory {
    text: String,
}
impl_request!(CreateMemory => Memory; encode = json; decode = json);

/// Request state of [`Memories::get`].
#[derive(Clone, Debug)]
pub struct GetMemory;
impl_request!(GetMemory => Memory; encode = none; decode = json);

/// Request state of [`Memories::update`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemory {
    text: String,
}
impl_request!(UpdateMemory => Memory; encode = json; decode = json);

/// Request state of [`Memories::delete`].
#[derive(Clone, Debug)]
pub struct DeleteMemory;
impl_request!(DeleteMemory => (); encode = none; decode = none);

/// A memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Memory {}
