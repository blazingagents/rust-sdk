//! Tenant settings and the tenant-wide spending limit. Tenant only; start from
//! [`Client::tenant`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::op::Op;
use crate::spending_limits::{SpendingLimitInput, SpendingLimitStatus};

/// Tenant operations. Returned by [`Client::tenant`].
#[derive(Clone, Copy, Debug)]
pub struct TenantResource<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Tenant operations.
    pub fn tenant(&self) -> TenantResource<'_> {
        TenantResource { client: self }
    }
}

impl TenantResource<'_> {
    /// Gets the tenant's settings.
    pub fn get(&self) -> Call<GetTenantSettings> {
        Call::new(self.client, Op::GetTenantSettings, GetTenantSettings)
    }

    /// Changes the tenant's settings. Fields you do not set keep their values.
    pub fn update(&self) -> Call<UpdateTenantSettings> {
        Call::new(
            self.client,
            Op::UpdateTenantSettings,
            UpdateTenantSettings::default(),
        )
    }

    /// Gets the tenant-wide model spending limit and its current period.
    pub fn get_spending_limit(&self) -> Call<GetTenantSpendingLimit> {
        Call::new(
            self.client,
            Op::GetTenantSpendingLimit,
            GetTenantSpendingLimit,
        )
    }

    /// Sets the tenant-wide model spending limit, or removes it with `None`.
    pub fn update_spending_limit(
        &self,
        limit: Option<SpendingLimitInput>,
    ) -> Call<UpdateTenantSpendingLimit> {
        Call::new(
            self.client,
            Op::UpdateTenantSpendingLimit,
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
pub struct UpdateTenantSettings {}
impl_request!(UpdateTenantSettings => TenantSettings; encode = json; decode = json);

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
pub struct TenantSettings {}
