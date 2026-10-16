//! Skills of one agent. Start from [`Client::agent`] and
//! [`AgentHandle::skills`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/skills>.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::call::{Call, Payload, impl_request};
use crate::client::{AgentHandle, Client, Scope};
use crate::error::{ErrorCode, Result};
use crate::files::{Download, Upload};
use crate::ids::{AgentId, SkillId, TenantId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::types::Timestamp;

/// Skill operations of one agent. Returned by [`AgentHandle::skills`].
#[derive(Clone, Copy, Debug)]
pub struct Skills<'a, S: Scope> {
    client: &'a Client<S>,
    agent_id: &'a AgentId,
}

impl<S: Scope> AgentHandle<'_, S> {
    /// Skill operations of this agent.
    pub fn skills(&self) -> Skills<'_, S> {
        Skills {
            client: self.client,
            agent_id: &self.agent_id,
        }
    }
}

impl<S: Scope> Skills<'_, S> {
    /// Creates a skill from its `SKILL.md`. `path` must be `SKILL.md`, and
    /// `content` must start with YAML frontmatter that sets `name` and
    /// `description`.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// let skill = client
    ///     .agent("ag_4kP9sT2vXq7LmN3a")
    ///     .skills()
    ///     .create(
    ///         "SKILL.md",
    ///         "---\nname: refund-policy\ndescription: Answer refund questions.\n---\n\nQuote the matching rule.\n",
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(&self, path: impl Into<String>, content: impl Into<String>) -> Call<CreateSkill> {
        Call::new(
            self.client,
            Op::CreateSkill {
                agent_id: self.agent_id.as_str(),
            },
            CreateSkill {
                path: path.into(),
                content: content.into(),
            },
        )
    }

    /// Creates a skill from an archive that contains `SKILL.md` at its root.
    /// The file part's name defaults to `skill.<archive type>`, such as
    /// `skill.tar.gz`; set [`Upload::file_name`] to send another.
    pub fn upload(&self, archive_type: ArchiveType, archive: Upload) -> Call<UploadSkill> {
        Call::new(
            self.client,
            Op::UploadSkill {
                agent_id: self.agent_id.as_str(),
            },
            UploadSkill {
                archive_type,
                archive,
            },
        )
    }

    /// Lists the agent's skills, newest first.
    pub fn list(&self) -> Call<ListSkills> {
        Call::new(
            self.client,
            Op::ListSkills {
                agent_id: self.agent_id.as_str(),
            },
            ListSkills::default(),
        )
    }

    /// Gets one skill, with its files.
    pub fn get(&self, skill_id: impl Into<SkillId>) -> Call<GetSkill> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::GetSkill {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            GetSkill,
        )
    }

    /// Permanently deletes a skill and all its files.
    pub fn delete(&self, skill_id: impl Into<SkillId>) -> Call<DeleteSkill> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::DeleteSkill {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            DeleteSkill,
        )
    }

    /// Streams one file of a skill. `path` is relative to the skill's root,
    /// such as `policy.md` or `scripts/refund.sh`.
    pub fn read_file(
        &self,
        skill_id: impl Into<SkillId>,
        path: impl Into<String>,
    ) -> Call<GetSkillFile> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::GetSkillFile {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            GetSkillFile { path: path.into() },
        )
    }

    /// Creates or replaces the file at `path` and returns the updated skill.
    /// Replacing `SKILL.md` rereads its frontmatter, so the skill's `name`,
    /// `description` and `metadata` follow it.
    pub fn replace_file(
        &self,
        skill_id: impl Into<SkillId>,
        path: impl Into<String>,
        content: Upload,
    ) -> Call<PutSkillFile> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::PutSkillFile {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            PutSkillFile {
                path: path.into(),
                content,
            },
        )
    }

    /// Deletes the file at `path` and returns the updated skill. Deleting a
    /// file that does not exist succeeds. `SKILL.md` cannot be deleted;
    /// delete the skill instead.
    pub fn delete_file(
        &self,
        skill_id: impl Into<SkillId>,
        path: impl Into<String>,
    ) -> Call<DeleteSkillFile> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::DeleteSkillFile {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            DeleteSkillFile { path: path.into() },
        )
    }

    /// Copies the skill, with all its files, to each agent in `agent_ids`: 1
    /// to 30 IDs, each listed once. Each copy is a new skill with its own ID.
    /// The results come back in request order; a failed destination yields a
    /// [`SkillCopyResult::Failed`] result, not an error.
    pub fn copy(
        &self,
        skill_id: impl Into<SkillId>,
        agent_ids: impl IntoIterator<Item = impl Into<AgentId>>,
    ) -> Call<CopySkill> {
        let skill_id = skill_id.into();
        Call::new(
            self.client,
            Op::CopySkill {
                agent_id: self.agent_id.as_str(),
                skill_id: skill_id.as_str(),
            },
            CopySkill {
                agent_ids: agent_ids.into_iter().map(Into::into).collect(),
            },
        )
    }
}

crate::open_enum! {
    /// The format of an uploaded skill archive.
    pub enum ArchiveType {
        /// `zip`
        Zip = "zip",
        /// `tar`
        Tar = "tar",
        /// `tar.gz`
        TarGz = "tar.gz",
    }
}

/// Request state of [`Skills::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkill {
    path: String,
    content: String,
}
impl_request!(CreateSkill => SkillDetail; encode = json; decode = json);

/// Request state of [`Skills::upload`].
#[derive(Debug)]
pub struct UploadSkill {
    archive_type: ArchiveType,
    archive: Upload,
}

impl UploadSkill {
    fn into_payload(self) -> Result<Payload> {
        let form = reqwest::multipart::Form::new()
            .text("type", self.archive_type.as_str().to_owned())
            .part(
                "file",
                self.archive
                    .default_file_name(format!("skill.{}", self.archive_type.as_str()))
                    .into_part()?,
            );
        Ok(Payload::default().multipart(form))
    }
}
impl_request!(UploadSkill => SkillDetail; encode = (Self::into_payload); decode = json);

/// Request state of [`Skills::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSkills {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListSkills => Page<Skill>; encode = query; decode = json; paginate);

impl Call<ListSkills> {
    /// Skills per page, 1 to 100. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Skills::get`].
#[derive(Clone, Debug)]
pub struct GetSkill;
impl_request!(GetSkill => SkillDetail; encode = none; decode = json);

/// Request state of [`Skills::delete`].
#[derive(Clone, Debug)]
pub struct DeleteSkill;
impl_request!(DeleteSkill => (); encode = none; decode = none);

/// Request state of [`Skills::read_file`].
#[derive(Clone, Debug, Serialize)]
pub struct GetSkillFile {
    path: String,
}
impl_request!(GetSkillFile => Download; encode = query; decode = download);

/// Request state of [`Skills::replace_file`].
#[derive(Debug)]
pub struct PutSkillFile {
    path: String,
    content: Upload,
}

impl PutSkillFile {
    fn into_payload(self) -> Result<Payload> {
        Ok(Payload {
            query: vec![("path".to_owned(), self.path)],
            ..Payload::default()
        }
        .octet_stream(self.content.into_body()))
    }
}
impl_request!(PutSkillFile => SkillDetail; encode = (Self::into_payload); decode = json);

/// Request state of [`Skills::delete_file`].
#[derive(Clone, Debug, Serialize)]
pub struct DeleteSkillFile {
    path: String,
}
impl_request!(DeleteSkillFile => SkillDetail; encode = query; decode = json);

/// Request state of [`Skills::copy`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySkill {
    agent_ids: Vec<AgentId>,
}
impl_request!(CopySkill => Vec<SkillCopyResult>; encode = json; decode = json);

/// A skill in a list page: its summary fields without its files.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// The skill's ID.
    pub id: SkillId,
    /// The tenant that owns the skill.
    pub tenant_id: TenantId,
    /// The agent that owns the skill.
    pub agent_id: AgentId,
    /// The skill's name from its `SKILL.md` frontmatter.
    pub name: String,
    /// What the skill does, from its `SKILL.md` frontmatter.
    pub description: String,
    /// The `metadata` map of the `SKILL.md` frontmatter, if it sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    /// When the skill was created.
    pub created_at: Timestamp,
    /// When the skill last changed.
    pub updated_at: Timestamp,
}

/// A skill with its files.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    /// The skill's ID.
    pub id: SkillId,
    /// The tenant that owns the skill.
    pub tenant_id: TenantId,
    /// The agent that owns the skill.
    pub agent_id: AgentId,
    /// The skill's name from its `SKILL.md` frontmatter.
    pub name: String,
    /// What the skill does, from its `SKILL.md` frontmatter.
    pub description: String,
    /// The `metadata` map of the `SKILL.md` frontmatter, if it sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
    /// When the skill was created.
    pub created_at: Timestamp,
    /// When the skill last changed.
    pub updated_at: Timestamp,
    /// The skill's files.
    pub files: Vec<SkillFile>,
}

/// One file of a skill.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SkillFile {
    /// The path relative to the skill's root.
    pub path: String,
    /// The file's size in bytes.
    pub size_bytes: u64,
}

/// The outcome of copying a skill to one destination agent.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum SkillCopyResult {
    /// The copy succeeded.
    #[non_exhaustive]
    Created {
        /// The destination agent's ID.
        #[serde(rename = "agentId")]
        agent_id: AgentId,
        /// The new skill, with its files.
        skill: SkillDetail,
    },
    /// The copy failed.
    #[non_exhaustive]
    Failed {
        /// The destination agent's ID.
        #[serde(rename = "agentId")]
        agent_id: AgentId,
        /// Why the copy failed.
        error: SkillCopyError,
    },
    /// A status this version of the SDK does not know; the raw object,
    /// re-serialized unchanged.
    #[serde(untagged)]
    Unrecognized(serde_json::Value),
}

impl<'de> Deserialize<'de> for SkillCopyResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// The known wire shapes; a malformed one is a decode error, not
        /// `Unrecognized`.
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "lowercase")]
        enum Known {
            Created {
                #[serde(rename = "agentId")]
                agent_id: AgentId,
                skill: SkillDetail,
            },
            Failed {
                #[serde(rename = "agentId")]
                agent_id: AgentId,
                error: SkillCopyError,
            },
        }

        crate::types::decode_tagged(
            deserializer,
            "status",
            |tag, raw| match tag {
                "created" | "failed" => Some(Known::deserialize(raw).map(|known| match known {
                    Known::Created { agent_id, skill } => Self::Created { agent_id, skill },
                    Known::Failed { agent_id, error } => Self::Failed { agent_id, error },
                })),
                _ => None,
            },
            |raw| Self::Unrecognized(serde_json::Value::Object(raw)),
        )
    }
}

/// Why copying a skill to one destination failed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SkillCopyError {
    /// The error's code, such as `skill_name_conflict` or
    /// `skill_limit_reached`.
    pub code: ErrorCode,
    /// The error's message.
    pub message: String,
    /// Structured details, if the server sent any.
    pub details: Option<serde_json::Value>,
}
