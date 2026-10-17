//! Tenant settings and the tenant-wide spending limit. Tenant only; start from
//! [`Client::tenant`].
//!
//! Documentation: <https://docs.blazingagents.com/platform/usage-and-quotas>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::op::TenantOp;
use crate::patch::Patch;
use crate::spending_limits::{SpendingLimitInput, SpendingLimitStatus};
use crate::types::Timestamp;

/// Tenant operations. Returned by [`Client::tenant`].
#[derive(Clone, Copy, Debug)]
pub struct TenantResource<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Tenant operations.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.tenant();
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::Client) {
    /// let tenant = client.tenant();
    /// # }
    /// ```
    pub fn tenant(&self) -> TenantResource<'_> {
        TenantResource { client: self }
    }
}

impl TenantResource<'_> {
    /// Gets the tenant's settings.
    pub fn get(&self) -> Call<GetTenantSettings> {
        Call::tenant(self.client, TenantOp::GetTenantSettings, GetTenantSettings)
    }

    /// Changes the tenant's settings. Fields you do not set keep their values.
    /// Send at least one field.
    pub fn update(&self) -> Call<UpdateTenantSettings> {
        Call::tenant(
            self.client,
            TenantOp::UpdateTenantSettings,
            UpdateTenantSettings::default(),
        )
    }

    /// Gets the tenant-wide model spending limit and its current period.
    pub fn get_spending_limit(&self) -> Call<GetTenantSpendingLimit> {
        Call::tenant(
            self.client,
            TenantOp::GetTenantSpendingLimit,
            GetTenantSpendingLimit,
        )
    }

    /// Sets the tenant-wide model spending limit, or removes it with `None`.
    pub fn update_spending_limit(
        &self,
        limit: Option<SpendingLimitInput>,
    ) -> Call<UpdateTenantSpendingLimit> {
        Call::tenant(
            self.client,
            TenantOp::UpdateTenantSpendingLimit,
            UpdateTenantSpendingLimit {
                spending_limit: limit,
            },
        )
    }
}

/// Request state of [`TenantResource::get`].
#[derive(Clone, Debug)]
pub struct GetTenantSettings;
impl_request!(GetTenantSettings => TenantSettings; encode = none; decode = json);

/// Request state of [`TenantResource::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTenantSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monetization_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    quota: Patch<QuotaInput>,
}
impl_request!(UpdateTenantSettings => TenantSettings; encode = json; decode = json);

impl Call<UpdateTenantSettings> {
    /// A new display name, 1 to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// Bills usage to your end users through your merchant account. Turning it
    /// off drops usage events your merchant has not yet accepted.
    pub fn monetization_enabled(self, enabled: bool) -> Self {
        self.map(|request| request.monetization_enabled = Some(enabled))
    }

    /// Replaces the whole monthly quota.
    pub fn quota(self, quota: QuotaInput) -> Self {
        self.map(|request| request.quota = Patch::Set(quota))
    }

    /// Removes the monthly quota.
    pub fn clear_quota(self) -> Self {
        self.map(|request| request.quota = Patch::Clear)
    }
}

/// Request state of [`TenantResource::get_spending_limit`].
#[derive(Clone, Debug)]
pub struct GetTenantSpendingLimit;
impl_request!(GetTenantSpendingLimit => SpendingLimitStatus; encode = none; decode = json);

/// Request state of [`TenantResource::update_spending_limit`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTenantSpendingLimit {
    spending_limit: Option<SpendingLimitInput>,
}
impl_request!(UpdateTenantSpendingLimit => SpendingLimitStatus; encode = json; decode = json);

/// The tenant's settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TenantSettings {
    /// The display name.
    pub name: String,
    /// The monthly quota, or `None` when none is set.
    pub quota: Option<Quota>,
    /// A scheduled deletion, or `None` when none is pending.
    pub deletion: Option<TenantDeletion>,
    /// Whether usage is billed to your end users.
    pub monetization_enabled: bool,
}

/// The tenant's monthly quota.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    /// The total input and output tokens per month, or `None` for unlimited.
    pub monthly_token_limit: Option<u64>,
    /// The requests per month, or `None` for unlimited.
    pub monthly_request_limit: Option<u64>,
    /// The day of each month the quota resets, 1 to 28.
    pub reset_day: u8,
}

/// A scheduled tenant deletion.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TenantDeletion {
    /// When the tenant is deleted.
    pub deletes_at: Timestamp,
    /// When the deletion was requested.
    pub requested_at: Timestamp,
}

/// A monthly quota to set on the tenant. Every field is always sent; a limit
/// you do not set goes out as `null`, which means unlimited.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaInput {
    monthly_token_limit: Option<u64>,
    monthly_request_limit: Option<u64>,
    reset_day: u8,
}

impl QuotaInput {
    /// A quota resetting on day `reset_day` of each month, 1 to 28. Set at
    /// least one limit.
    pub fn new(reset_day: u8) -> Self {
        Self {
            monthly_token_limit: None,
            monthly_request_limit: None,
            reset_day,
        }
    }

    /// The total input and output tokens per month.
    #[must_use]
    pub fn monthly_token_limit(mut self, limit: u64) -> Self {
        self.monthly_token_limit = Some(limit);
        self
    }

    /// The requests per month.
    #[must_use]
    pub fn monthly_request_limit(mut self, limit: u64) -> Self {
        self.monthly_request_limit = Some(limit);
        self
    }
}
