//! Tenant settings and the tenant-wide spending limit. Tenant only; start from
//! [`Client::tenant`].
//!
//! Documentation: <https://docs.blazingagents.com/platform/usage-and-spending-limits>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::op::TenantOp;
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
    /// A scheduled deletion, or `None` when none is pending.
    pub deletion: Option<TenantDeletion>,
    /// Whether usage is billed to your end users.
    pub monetization_enabled: bool,
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
