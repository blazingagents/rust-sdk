//! Artifacts produced in sessions. Start from [`Client::artifacts`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::ArtifactId;
use crate::op::Op;
use crate::pagination::{Cursor, Page};

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
    /// Lists artifacts.
    pub fn list(&self) -> Call<ListArtifacts> {
        Call::new(self.client, Op::ListArtifacts, ListArtifacts::default())
    }

    /// Gets one artifact.
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

    /// Creates a short-lived download URL for an artifact.
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

    /// Deletes an artifact.
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
    cursor: Option<Cursor>,
}
impl_request!(ListArtifacts => Page<Artifact>; encode = query; decode = json; paginate);

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

/// An artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Artifact {}

/// A short-lived download URL.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDownloadUrl {}
