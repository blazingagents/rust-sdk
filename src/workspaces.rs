//! Private file systems and shells your agents keep between sessions. Start
//! from [`Client::workspaces`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/workspaces>.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::call::{Call, Raw, impl_request};
use crate::client::{Client, Scope};
use crate::ids::{TenantId, WorkspaceId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};
use crate::patch::Patch;
use crate::types::{Metadata, Timestamp};

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
    /// Lists workspaces, newest first. The workspace reserved for the
    /// platform-managed agent is never listed.
    pub fn list(&self) -> Call<ListWorkspaces> {
        Call::new(self.client, Op::ListWorkspaces, ListWorkspaces::default())
    }

    /// Creates a workspace. It starts no compute until an agent first reads,
    /// writes, or runs something in it.
    ///
    /// ```no_run
    /// # async fn example(client: blazing_agents::Client) -> blazing_agents::Result<()> {
    /// use blazing_agents::workspaces::NetworkPolicy;
    /// let workspace = client
    ///     .workspaces()
    ///     .create()
    ///     .name("Release files")
    ///     .network_policy(NetworkPolicy::allowlist(["registry.npmjs.org"]))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(&self) -> Call<CreateWorkspace> {
        Call::new(self.client, Op::CreateWorkspace, CreateWorkspace::default())
    }

    /// Gets one workspace without starting it.
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

    /// Changes a workspace's name, metadata, or network policy. Send at least
    /// one field; fields you do not set keep their values, and `metadata` and
    /// `network_policy` replace their current values completely.
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

    /// Permanently deletes a workspace and all its files. Move attached agents
    /// to another workspace first: while any agent uses it the request fails
    /// with `workspace_in_use`. Returns [`WorkspaceDeletion::Pending`] when the
    /// deletion was accepted and cleanup continues in the background, or
    /// [`WorkspaceDeletion::Completed`] once the workspace is gone.
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
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListWorkspaces => Page<Workspace>; encode = query; decode = json; paginate);

impl Call<ListWorkspaces> {
    /// Only workspaces of this end user. An empty string selects tenant-level
    /// workspaces.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Workspaces per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`Workspaces::create`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspace {
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<WorkspaceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_policy: Option<NetworkPolicy>,
}
impl_request!(CreateWorkspace => Workspace; encode = json; decode = json);

impl Call<CreateWorkspace> {
    /// The workspace tier, fixed at creation. [`WorkspaceTier::Core`] (the
    /// default) loses its files when the workspace stops;
    /// [`WorkspaceTier::Plus`] keeps them while it is stopped.
    pub fn tier(self, tier: WorkspaceTier) -> Self {
        self.map(|request| request.tier = Some(tier))
    }

    /// A display name, up to 80 characters. Leave it unset for an unnamed
    /// workspace.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// The end user who owns the workspace. Fixed after creation; an empty
    /// string (the default) means a tenant-level workspace.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Your own data, returned unchanged.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// Which hosts the workspace can reach. Defaults to
    /// [`NetworkPolicy::Unrestricted`].
    pub fn network_policy(self, network_policy: NetworkPolicy) -> Self {
        self.map(|request| request.network_policy = Some(network_policy))
    }
}

/// Request state of [`Workspaces::get`].
#[derive(Clone, Debug)]
pub struct GetWorkspace;
impl_request!(GetWorkspace => Workspace; encode = none; decode = json);

/// Request state of [`Workspaces::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspace {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    name: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_policy: Option<NetworkPolicy>,
}
impl_request!(UpdateWorkspace => Workspace; encode = json; decode = json);

impl Call<UpdateWorkspace> {
    /// A new display name, up to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Patch::Set(name.into()))
    }

    /// Removes the display name.
    pub fn clear_name(self) -> Self {
        self.map(|request| request.name = Patch::Clear)
    }

    /// Replaces the workspace's metadata completely.
    pub fn metadata(self, metadata: Metadata) -> Self {
        self.map(|request| request.metadata = Some(metadata))
    }

    /// Replaces the network policy. If it switches between `unrestricted` and
    /// a restricted mode, a running workspace saves its files and stops; it
    /// applies the new policy the next time an agent uses it.
    pub fn network_policy(self, network_policy: NetworkPolicy) -> Self {
        self.map(|request| request.network_policy = Some(network_policy))
    }
}

/// How a workspace deletion ended. Returned by [`Workspaces::delete`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorkspaceDeletion {
    /// The workspace and its files are gone.
    Completed,
    /// The deletion was accepted (202) and cleanup continues in the background.
    Pending,
}

/// Request state of [`Workspaces::delete`].
#[derive(Clone, Debug)]
pub struct DeleteWorkspace;
impl_request!(DeleteWorkspace => WorkspaceDeletion; encode = none; decode = (delete_outcome));

/// The outcome comes from the status alone; the body is empty.
async fn delete_outcome(raw: Raw) -> crate::error::Result<WorkspaceDeletion> {
    Ok(if raw.meta.status == 202 {
        WorkspaceDeletion::Pending
    } else {
        WorkspaceDeletion::Completed
    })
}

crate::open_enum! {
    /// The workspace tier, fixed at creation.
    pub enum WorkspaceTier {
        /// `core`: files are lost when the workspace stops.
        Core = "core",
        /// `plus`: files are kept when the workspace stops while idle.
        Plus = "plus",
    }
}

/// Which hosts a workspace can reach. A `mode` this version of the SDK does
/// not know decodes as [`NetworkPolicy::Unrecognized`] instead of failing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetworkPolicy {
    /// Every host is reachable (`{"mode": "unrestricted"}`).
    Unrestricted,
    /// Only the listed hosts are reachable
    /// (`{"mode": "allowlist", "allowedHosts": [...]}`).
    #[non_exhaustive]
    Allowlist {
        /// The hosts the workspace may reach.
        allowed_hosts: Vec<String>,
    },
    /// No host is reachable (`{"mode": "offline"}`).
    Offline,
    /// A mode this version of the SDK does not know, kept exactly as the
    /// server sent it.
    Unrecognized(UnrecognizedNetworkPolicy),
}

impl NetworkPolicy {
    /// Only `hosts` are reachable. Each is a host name such as
    /// `registry.npmjs.org`.
    pub fn allowlist(hosts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Allowlist {
            allowed_hosts: hosts.into_iter().map(Into::into).collect(),
        }
    }
}

/// A network policy mode this version of the SDK does not know, kept exactly
/// as the server sent it so sending it back — for example in
/// [`Workspaces::update`] — round-trips it unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnrecognizedNetworkPolicy {
    raw: serde_json::Map<String, serde_json::Value>,
}

impl UnrecognizedNetworkPolicy {
    /// The `mode` string the server sent.
    pub fn mode(&self) -> &str {
        self.raw
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    }

    /// The whole network policy object as received.
    pub fn as_json(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.raw
    }
}

/// The `mode`-tagged wire shape of a known [`NetworkPolicy`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkPolicyWire {
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_hosts: Option<Vec<String>>,
}

impl Serialize for NetworkPolicy {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (mode, allowed_hosts) = match self {
            Self::Unrestricted => ("unrestricted", None),
            Self::Allowlist { allowed_hosts } => ("allowlist", Some(allowed_hosts.clone())),
            Self::Offline => ("offline", None),
            Self::Unrecognized(other) => return other.raw.serialize(serializer),
        };
        NetworkPolicyWire {
            mode: mode.to_owned(),
            allowed_hosts,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NetworkPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::types::decode_tagged(
            deserializer,
            "mode",
            |mode, raw| match mode {
                "unrestricted" => Some(Ok(Self::Unrestricted)),
                "offline" => Some(Ok(Self::Offline)),
                "allowlist" => Some(
                    raw.get("allowedHosts")
                        .cloned()
                        .ok_or_else(|| serde::de::Error::missing_field("allowedHosts"))
                        .and_then(|hosts| {
                            serde_json::from_value::<Vec<String>>(hosts)
                                .map(|allowed_hosts| Self::Allowlist { allowed_hosts })
                        }),
                ),
                _ => None,
            },
            |raw| Self::Unrecognized(UnrecognizedNetworkPolicy { raw }),
        )
    }
}

/// A workspace: a private file system agents keep between sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// The workspace's ID.
    pub id: WorkspaceId,
    /// The tenant that owns the workspace.
    pub tenant_id: TenantId,
    /// The workspace's tier.
    pub tier: WorkspaceTier,
    /// The display name, if any.
    pub name: Option<String>,
    /// The end user, or an empty string for a tenant-level workspace.
    pub user_id: String,
    /// Your own data.
    pub metadata: Metadata,
    /// Which hosts the workspace can reach.
    pub network_policy: NetworkPolicy,
    /// When the workspace was created.
    pub created_at: Timestamp,
    /// When the workspace last changed.
    pub updated_at: Timestamp,
}
