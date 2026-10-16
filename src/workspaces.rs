//! Workspaces. Start from [`Client::workspaces`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope};
use crate::ids::WorkspaceId;
use crate::op::Op;
use crate::pagination::{Cursor, Page};

/// Workspace operations. Returned by [`Client::workspaces`].
#[derive(Clone, Copy, Debug)]
pub struct Workspaces<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Workspace operations.
    pub fn workspaces(&self) -> Workspaces<'_, S> {
        Workspaces { client: self }
    }
}

impl<S: Scope> Workspaces<'_, S> {
    /// Lists workspaces.
    pub fn list(&self) -> Call<ListWorkspaces> {
        Call::new(self.client, Op::ListWorkspaces, ListWorkspaces::default())
    }

    /// Creates a workspace.
    pub fn create(&self) -> Call<CreateWorkspace> {
        Call::new(self.client, Op::CreateWorkspace, CreateWorkspace::default())
    }

    /// Gets one workspace.
    pub fn get(&self, workspace_id: impl Into<WorkspaceId>) -> Call<GetWorkspace> {
        let workspace_id = workspace_id.into();
        Call::new(
            self.client,
            Op::GetWorkspace {
                workspace_id: workspace_id.as_str(),
            },
            GetWorkspace,
        )
    }

    /// Changes a workspace. Fields you do not set keep their values.
    pub fn update(&self, workspace_id: impl Into<WorkspaceId>) -> Call<UpdateWorkspace> {
        let workspace_id = workspace_id.into();
        Call::new(
            self.client,
            Op::UpdateWorkspace {
                workspace_id: workspace_id.as_str(),
            },
            UpdateWorkspace::default(),
        )
    }

    /// Deletes a workspace.
    pub fn delete(&self, workspace_id: impl Into<WorkspaceId>) -> Call<DeleteWorkspace> {
        let workspace_id = workspace_id.into();
        Call::new(
            self.client,
            Op::DeleteWorkspace {
                workspace_id: workspace_id.as_str(),
            },
            DeleteWorkspace,
        )
    }
}

/// Request state of [`Workspaces::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkspaces {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListWorkspaces => Page<Workspace>; encode = query; decode = json; paginate);

/// Request state of [`Workspaces::create`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspace {}
impl_request!(CreateWorkspace => Workspace; encode = json; decode = json);

/// Request state of [`Workspaces::get`].
#[derive(Clone, Debug)]
pub struct GetWorkspace;
impl_request!(GetWorkspace => Workspace; encode = none; decode = json);

/// Request state of [`Workspaces::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspace {}
impl_request!(UpdateWorkspace => Workspace; encode = json; decode = json);

/// Request state of [`Workspaces::delete`].
#[derive(Clone, Debug)]
pub struct DeleteWorkspace;
impl_request!(DeleteWorkspace => (); encode = none; decode = none);

/// A workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Workspace {}
