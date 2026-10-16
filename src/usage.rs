//! Usage reports. Start from [`Client::usage`]. The overview and per-agent
//! reports are tenant only.

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Scope, Tenant};
use crate::ids::{AgentId, SessionId};
use crate::op::Op;

/// Usage operations. Returned by [`Client::usage`].
#[derive(Clone, Copy, Debug)]
pub struct UsageResource<'a, S: Scope> {
    client: &'a Client<S>,
}

impl<S: Scope> Client<S> {
    /// Usage operations.
    pub fn usage(&self) -> UsageResource<'_, S> {
        UsageResource { client: self }
    }
}

impl<S: Scope> UsageResource<'_, S> {
    /// Gets usage over a period, grouped by day by default.
    pub fn get(&self) -> Call<GetUsage> {
        Call::new(self.client, Op::GetUsage, GetUsage::default())
    }

    /// Gets the usage of specific sessions.
    pub fn sessions(
        &self,
        session_ids: impl IntoIterator<Item = impl Into<SessionId>>,
    ) -> Call<GetSessionUsage> {
        Call::new(
            self.client,
            Op::GetSessionUsage,
            GetSessionUsage {
                session_ids: session_ids.into_iter().map(Into::into).collect(),
            },
        )
    }
}

impl UsageResource<'_, Tenant> {
    /// Gets the tenant's usage overview.
    pub fn overview(&self) -> Call<GetUsageOverview> {
        Call::new(
            self.client,
            Op::GetUsageOverview,
            GetUsageOverview::default(),
        )
    }

    /// Gets one agent's usage over a period.
    pub fn get_for_agent(&self, agent_id: impl Into<AgentId>) -> Call<GetAgentUsage> {
        let agent_id = agent_id.into();
        Call::new(
            self.client,
            Op::GetAgentUsage {
                agent_id: agent_id.as_str(),
            },
            GetAgentUsage::default(),
        )
    }
}

/// Request state of [`UsageResource::get`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUsage {}
impl_request!(GetUsage => Usage; encode = query; decode = json);

/// Request state of [`UsageResource::sessions`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionUsage {
    session_ids: Vec<SessionId>,
}
impl_request!(GetSessionUsage => SessionUsage; encode = json; decode = json);

/// Request state of [`UsageResource::overview`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUsageOverview {}
impl_request!(GetUsageOverview => UsageOverview; encode = query; decode = json);

/// Request state of [`UsageResource::get_for_agent`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAgentUsage {}
impl_request!(GetAgentUsage => Usage; encode = query; decode = json);

/// Usage over a period, in buckets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Usage {}

/// The tenant's usage overview.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UsageOverview {}

/// The usage of specific sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {}
