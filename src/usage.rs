//! Usage reports. Start from [`Client::usage`]. The overview is tenant only.
//!
//! Documentation: <https://docs.blazingagents.com/platform/usage-and-quotas>.

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

    /// Gets the usage of specific sessions, 1 to 100 of them.
    pub fn sessions(
        &self,
        session_ids: impl IntoIterator<Item = impl Into<SessionId>>,
    ) -> Call<GetSessionUsage> {
        Call::new(
            self.client,
            Op::GetSessionUsage,
            GetSessionUsage {
                session_ids: session_ids.into_iter().map(Into::into).collect(),
                from: None,
                to: None,
            },
        )
    }

    /// Gets one agent's usage over a period.
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::UserClient) {
    /// let usage = client.usage().get_for_agent("ag_4kP9sT2vXq7LmN3a");
    /// # }
    /// ```
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

impl UsageResource<'_, Tenant> {
    /// Gets the tenant's usage overview.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.usage().overview();
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::Client) {
    /// let overview = client.usage().overview();
    /// # }
    /// ```
    pub fn overview(&self) -> Call<GetUsageOverview> {
        Call::new(
            self.client,
            Op::GetUsageOverview,
            GetUsageOverview::default(),
        )
    }
}

crate::open_enum! {
    /// How usage buckets are grouped.
    pub enum UsageGroupBy {
        /// `day`
        Day = "day",
        /// `agent`
        Agent = "agent",
        /// `model`
        Model = "model",
        /// `session`
        Session = "session",
        /// `user`
        User = "user",
    }
}

/// Request state of [`UsageResource::get`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_by: Option<UsageGroupBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(GetUsage => Usage; encode = query; decode = json);

impl Call<GetUsage> {
    /// The UTC `YYYY-MM-DD` days to cover, `from` first and `to` last; `to` can
    /// be at most 31 days after `from`. Omitted, the period is the last 30
    /// days ending today.
    pub fn range(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.map(|request| {
            request.from = Some(from.into());
            request.to = Some(to.into());
        })
    }

    /// Only usage by this agent.
    pub fn agent_id(self, agent_id: impl Into<AgentId>) -> Self {
        self.map(|request| request.agent_id = Some(agent_id.into()))
    }

    /// Only usage in this session. An empty string selects calls made without
    /// a session.
    pub fn session_id(self, session_id: impl Into<SessionId>) -> Self {
        self.map(|request| request.session_id = Some(session_id.into()))
    }

    /// Only usage for this end user. An empty string selects tenant-level
    /// usage.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// How to group the buckets. `session` returns the top sessions by total
    /// tokens; the others return every group. Defaults to `day`.
    pub fn group_by(self, group_by: UsageGroupBy) -> Self {
        self.map(|request| request.group_by = Some(group_by))
    }

    /// Sessions to return with `group_by` [`UsageGroupBy::Session`], 1 to 200.
    /// Other groupings ignore it. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`UsageResource::sessions`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionUsage {
    session_ids: Vec<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
}
impl_request!(GetSessionUsage => SessionUsage; encode = json; decode = json);

impl Call<GetSessionUsage> {
    /// The UTC `YYYY-MM-DD` days to cover, `from` first and `to` last; `to` can
    /// be at most 31 days after `from`. Omitted, the period is the last 30
    /// days ending today.
    pub fn range(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.map(|request| {
            request.from = Some(from.into());
            request.to = Some(to.into());
        })
    }
}

/// Request state of [`UsageResource::overview`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUsageOverview {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(GetUsageOverview => UsageOverview; encode = query; decode = json);

impl Call<GetUsageOverview> {
    /// The UTC `YYYY-MM-DD` days to cover, `from` first and `to` last; `to` can
    /// be at most 31 days after `from`. Omitted, the period is the last 30
    /// days ending today.
    pub fn range(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.map(|request| {
            request.from = Some(from.into());
            request.to = Some(to.into());
        })
    }

    /// Rows per ranked breakdown (`by_agent`, `by_user`, `by_model`), 1 to 20.
    /// Defaults to 5.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`UsageResource::get_for_agent`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAgentUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<SessionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_by: Option<UsageGroupBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(GetAgentUsage => Usage; encode = query; decode = json);

impl Call<GetAgentUsage> {
    /// The UTC `YYYY-MM-DD` days to cover, `from` first and `to` last; `to` can
    /// be at most 31 days after `from`. Omitted, the period is the last 30
    /// days ending today.
    pub fn range(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.map(|request| {
            request.from = Some(from.into());
            request.to = Some(to.into());
        })
    }

    /// Only usage in this session. An empty string selects calls made without
    /// a session.
    pub fn session_id(self, session_id: impl Into<SessionId>) -> Self {
        self.map(|request| request.session_id = Some(session_id.into()))
    }

    /// Only usage for this end user. An empty string selects tenant-level
    /// usage.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// How to group the buckets. `session` returns the top sessions by total
    /// tokens; the others return every group. Defaults to `day`.
    pub fn group_by(self, group_by: UsageGroupBy) -> Self {
        self.map(|request| request.group_by = Some(group_by))
    }

    /// Sessions to return with `group_by` [`UsageGroupBy::Session`], 1 to 200.
    /// Other groupings ignore it. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Usage over a period, in buckets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// The usage, one bucket per group.
    pub buckets: Vec<UsageBucket>,
    /// The usage across all buckets.
    pub totals: UsageTotals,
}

/// One group's usage.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UsageBucket {
    /// The UTC `YYYY-MM-DD` day, when grouped by day.
    pub day: Option<String>,
    /// The agent, when grouped by agent.
    pub agent_id: Option<AgentId>,
    /// The session, when grouped by session.
    pub session_id: Option<SessionId>,
    /// The end user, when grouped by user. An empty string is tenant-level
    /// usage.
    pub user_id: Option<String>,
    /// The provider, when grouped by model.
    pub provider: Option<String>,
    /// The model, when grouped by model.
    pub model: Option<String>,
    /// Input tokens used.
    pub input_tokens: u64,
    /// Output tokens used.
    pub output_tokens: u64,
    /// Requests made.
    pub request_count: u64,
    /// Milliseconds spent in calls.
    pub duration_ms: u64,
}

/// Usage across all buckets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    /// Input tokens used.
    pub input_tokens: u64,
    /// Output tokens used.
    pub output_tokens: u64,
    /// Requests made.
    pub request_count: u64,
    /// Milliseconds spent in calls.
    pub duration_ms: u64,
}

/// The tenant's usage overview.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UsageOverview {
    /// The usage across the whole period.
    pub totals: UsageTotals,
    /// The usage per day.
    pub daily: Vec<UsageBucket>,
    /// The agents with the most usage.
    pub by_agent: Vec<UsageBucket>,
    /// The end users with the most usage.
    pub by_user: Vec<UsageBucket>,
    /// The models with the most usage.
    pub by_model: Vec<UsageBucket>,
    /// How many agents were used in the period.
    pub active_agent_count: u64,
}

/// The usage of specific sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {
    /// One entry per requested session.
    pub data: Vec<SessionUsageEntry>,
}

/// One session's usage.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SessionUsageEntry {
    /// The session.
    pub session_id: SessionId,
    /// The session's usage.
    pub totals: UsageTotals,
}
