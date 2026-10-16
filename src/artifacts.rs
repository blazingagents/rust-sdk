//! Fixed copies of the files your agents chose to publish, such as reports or
//! exports. Start from [`Client::artifacts`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/artifacts>.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{AgentId, ArtifactId, SessionId, TenantId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::types::{Metadata, Timestamp};

/// Artifact operations. Returned by [`Client::artifacts`].
#[derive(Clone, Copy, Debug)]
pub struct Artifacts<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Artifact operations.
    pub fn artifacts(&self) -> Artifacts<'_, S> {
        Artifacts { client: self }
    }
}

impl<S: Scope> Artifacts<'_, S> {
    /// Lists artifacts, newest first, 50 per page.
    pub fn list(&self) -> Call<ListArtifacts> {
        Call::new(self.client, Op::ListArtifacts, ListArtifacts::default())
    }

    /// Gets one artifact's metadata without its contents. Create a download
    /// URL to fetch the file.
    pub fn get(&self, artifact_id: impl Into<ArtifactId>) -> Call<GetArtifact> {
        let artifact_id = artifact_id.into();
        Call::new(
            self.client,
            Op::GetArtifact {
                artifact_id: artifact_id.as_str(),
            },
            GetArtifact,
        )
    }

    /// Creates a link that downloads the artifact's file for five minutes.
    /// Fetch it without an API key; anyone with the link can download the
    /// file, so keep it out of logs.
    pub fn create_download_url(
        &self,
        artifact_id: impl Into<ArtifactId>,
    ) -> Call<CreateArtifactDownloadUrl> {
        let artifact_id = artifact_id.into();
        Call::new(
            self.client,
            Op::CreateArtifactDownloadUrl {
                artifact_id: artifact_id.as_str(),
            },
            CreateArtifactDownloadUrl,
        )
    }

    /// Permanently deletes an artifact and its file. The source file in the
    /// workspace is not touched.
    pub fn delete(&self, artifact_id: impl Into<ArtifactId>) -> Call<DeleteArtifact> {
        let artifact_id = artifact_id.into();
        Call::new(
            self.client,
            Op::DeleteArtifact {
                artifact_id: artifact_id.as_str(),
            },
            DeleteArtifact,
        )
    }
}

/// Request state of [`Artifacts::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListArtifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListArtifacts => Page<Artifact>; encode = query; decode = json; paginate);

impl Call<ListArtifacts> {
    /// Only artifacts published by this agent.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Some(agent_id.into()))
    }

    /// Only artifacts published in this session.
    pub fn session_id(self, session_id: impl Into<SessionId>) -> Self {
        self.map(|request| request.session_id = Some(session_id.into()))
    }
}

/// Request state of [`Artifacts::get`].
#[derive(Clone, Debug)]
pub struct GetArtifact;
impl_request!(GetArtifact => Artifact; encode = none; decode = json);

/// Request state of [`Artifacts::create_download_url`].
#[derive(Clone, Debug)]
pub struct CreateArtifactDownloadUrl;
impl_request!(CreateArtifactDownloadUrl => ArtifactDownloadUrl; encode = none; decode = json);

/// Request state of [`Artifacts::delete`].
#[derive(Clone, Debug)]
pub struct DeleteArtifact;
impl_request!(DeleteArtifact => (); encode = none; decode = none);

/// An artifact: a fixed copy of a file an agent published.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// The artifact's ID.
    pub artifact_id: ArtifactId,
    /// The agent that published the artifact.
    pub agent_id: AgentId,
    /// The tenant that owns the artifact.
    pub tenant_id: TenantId,
    /// The session the artifact was published in.
    pub session_id: SessionId,
    /// The file name.
    pub filename: String,
    /// The MIME type.
    pub media_type: String,
    /// The size in bytes.
    pub size_bytes: u64,
    /// The end user, or an empty string for a tenant-level artifact.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// When the artifact was published.
    pub created_at: Timestamp,
    /// When the artifact last changed.
    pub updated_at: Timestamp,
}

/// A short-lived download URL. Anyone with it can download the file until it
/// expires, so keep it out of logs: `Debug` redacts it.
#[derive(Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDownloadUrl {
    /// The download URL, usable without an API key.
    pub url: String,
    /// When the URL stops working.
    pub expires_at: Timestamp,
}

impl fmt::Debug for ArtifactDownloadUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArtifactDownloadUrl")
            .field("url", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
