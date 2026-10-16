//! Saved message templates with `{{variable}}` placeholders. Start from
//! [`Client::prompts`]. Run a prompt by passing a
//! [`SavedPrompt`](crate::generation::SavedPrompt) to [`Client::chat`],
//! [`Client::completion`] or [`Client::object`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/prompts>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, PromptId, TenantId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::patch::Patch;
use crate::types::{Metadata, Timestamp};

/// Prompt operations. Returned by [`Client::prompts`].
#[derive(Clone, Copy, Debug)]
pub struct Prompts<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Prompt operations.
    pub fn prompts(&self) -> Prompts<'_, S> {
        Prompts { client: self }
    }
}

impl<S: Scope> Prompts<'_, S> {
    /// Saves a prompt. `name` is 1 to 80 characters and prompts can share one.
    /// `template` is up to 10,240 characters with up to 10 `{{variable}}`
    /// placeholders; the created prompt lists them in
    /// [`variables`](Prompt::variables).
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// let prompt = client
    ///     .prompts()
    ///     .create("Release summary", "Summarize {{version}} for {{audience}}.")
    ///     .agent_id("ag_4kP9sT2vXq7LmN3a")
    ///     .await?;
    /// assert_eq!(prompt.variables, ["version", "audience"]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(
        &self,
        name: impl Into<String>,
        template: impl Into<String>,
    ) -> Call<CreatePrompt> {
        Call::new(
            self.client,
            Op::CreatePrompt,
            CreatePrompt {
                name: name.into(),
                template: template.into(),
                agent_id: None,
                user_id: None,
                metadata: None,
            },
        )
    }

    /// Lists prompts, newest first. Without filters it returns every prompt.
    pub fn list(&self) -> Call<ListPrompts> {
        Call::new(self.client, Op::ListPrompts, ListPrompts::default())
    }

    /// Gets one prompt.
    pub fn get(&self, prompt_id: impl Into<PromptId>) -> Call<GetPrompt> {
        let prompt_id = prompt_id.into();
        Call::new(
            self.client,
            Op::GetPrompt {
                prompt_id: prompt_id.as_str(),
            },
            GetPrompt,
        )
    }

    /// Changes a prompt in place. Fields you do not set keep their values, and
    /// a new template recomputes [`variables`](Prompt::variables). Send at least
    /// one field.
    pub fn update(&self, prompt_id: impl Into<PromptId>) -> Call<UpdatePrompt> {
        let prompt_id = prompt_id.into();
        Call::new(
            self.client,
            Op::UpdatePrompt {
                prompt_id: prompt_id.as_str(),
            },
            UpdatePrompt::default(),
        )
    }

    /// Permanently deletes a prompt. Transcripts keep the text it produced.
    pub fn delete(&self, prompt_id: impl Into<PromptId>) -> Call<DeletePrompt> {
        let prompt_id = prompt_id.into();
        Call::new(
            self.client,
            Op::DeletePrompt {
                prompt_id: prompt_id.as_str(),
            },
            DeletePrompt,
        )
    }
}

/// Request state of [`Prompts::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePrompt {
    name: String,
    template: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}
impl_request!(CreatePrompt => Prompt; encode = json; decode = json);

impl Call<CreatePrompt> {
    /// Links the prompt to an agent. Deleting that agent deletes the prompt.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Some(agent_id.into()))
    }

    /// The end user the prompt belongs to. Fixed after creation; an empty
    /// string (the default) means a tenant-level prompt.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, returned unchanged.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }
}

/// Request state of [`Prompts::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPrompts {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListPrompts => Page<Prompt>; encode = query; decode = json; paginate);

impl Call<ListPrompts> {
    /// Only prompts of this end user. An empty string selects tenant-level
    /// prompts.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Only prompts linked to this agent.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Some(agent_id.into()))
    }

    /// Prompts per page, 1 to 100. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Prompts::get`].
#[derive(Clone, Debug)]
pub struct GetPrompt;
impl_request!(GetPrompt => Prompt; encode = none; decode = json);

/// Request state of [`Prompts::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePrompt {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    agent_id: Patch<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}
impl_request!(UpdatePrompt => Prompt; encode = json; decode = json);

impl Call<UpdatePrompt> {
    /// Links the prompt to an agent.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Patch::Set(agent_id.into()))
    }

    /// Removes the agent link.
    pub fn clear_agent_id(self) -> Self {
        self.map(|request| request.agent_id = Patch::Clear)
    }

    /// A new display name, 1 to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// A new template. [`variables`](Prompt::variables) is recomputed from it.
    pub fn template(self, template: impl Into<String>) -> Self {
        self.map(|request| request.template = Some(template.into()))
    }

    /// Replaces the prompt's metadata completely.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }
}

/// Request state of [`Prompts::delete`].
#[derive(Clone, Debug)]
pub struct DeletePrompt;
impl_request!(DeletePrompt => (); encode = none; decode = none);

/// A saved prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// The prompt's ID.
    pub id: PromptId,
    /// The tenant that owns the prompt.
    pub tenant_id: TenantId,
    /// The linked agent, if any.
    pub agent_id: Option<AgentId>,
    /// The display name.
    pub name: String,
    /// The template text.
    pub template: String,
    /// The placeholder names, each once, in first-seen order.
    pub variables: Vec<String>,
    /// The end user, or an empty string for a tenant-level prompt.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the prompt was created.
    pub created_at: Timestamp,
    /// When the prompt last changed.
    pub updated_at: Timestamp,
}
