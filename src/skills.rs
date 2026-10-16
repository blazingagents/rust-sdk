//! Skills of one agent. Start from [`Client::agent`] and
//! [`AgentHandle::skills`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, Payload, impl_request};
use crate::client::{AgentHandle, Client, Scope};
use crate::error::Result;
use crate::files::{Download, Upload};
use crate::ids::{AgentId, SkillId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};

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
    /// Creates a skill from one `SKILL.md` file.
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

    /// Creates a skill from an archive.
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

    /// Lists the agent's skills.
    pub fn list(&self) -> Call<ListSkills> {
        Call::new(
            self.client,
            Op::ListSkills {
                agent_id: self.agent_id.as_str(),
            },
            ListSkills::default(),
        )
    }

    /// Gets one skill.
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

    /// Deletes a skill.
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

    /// Streams one file of a skill.
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

    /// Creates or replaces one file of a skill.
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

    /// Deletes one file of a skill.
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

    /// Copies a skill to other agents.
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
impl_request!(CreateSkill => Skill; encode = json; decode = json);

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
            .part("file", self.archive.into_part()?);
        Ok(Payload::default().multipart(form))
    }
}
impl_request!(UploadSkill => Skill; encode = (Self::into_payload); decode = json);

/// Request state of [`Skills::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSkills {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListSkills => Page<Skill>; encode = query; decode = json; paginate);

/// Request state of [`Skills::get`].
#[derive(Clone, Debug)]
pub struct GetSkill;
impl_request!(GetSkill => Skill; encode = none; decode = json);

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
impl_request!(PutSkillFile => Skill; encode = (Self::into_payload); decode = json);

/// Request state of [`Skills::delete_file`].
#[derive(Clone, Debug, Serialize)]
pub struct DeleteSkillFile {
    path: String,
}
impl_request!(DeleteSkillFile => Skill; encode = query; decode = json);

/// Request state of [`Skills::copy`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySkill {
    agent_ids: Vec<AgentId>,
}
impl_request!(CopySkill => SkillCopyResultList; encode = json; decode = json);

/// A skill.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Skill {}

/// The outcome of copying a skill to each destination agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SkillCopyResultList {}
