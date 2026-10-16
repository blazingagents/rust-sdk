//! Merchant billing: the tenant's merchant connection, its customer bindings and
//! usage events. Tenant only; start from [`Client::merchant_connection`],
//! [`Client::merchant_bindings`] and [`Client::merchant_usage_events`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::MerchantUsageEventId;
use crate::op::Op;
use crate::pagination::{Cursor, Paginated};

/// Merchant connection operations. Returned by [`Client::merchant_connection`].
#[derive(Clone, Copy, Debug)]
pub struct MerchantConnectionResource<'a> {
    client: &'a Client<Tenant>,
}

/// Merchant binding operations. Returned by [`Client::merchant_bindings`].
#[derive(Clone, Copy, Debug)]
pub struct MerchantBindings<'a> {
    client: &'a Client<Tenant>,
}

/// Merchant usage event operations. Returned by [`Client::merchant_usage_events`].
#[derive(Clone, Copy, Debug)]
pub struct MerchantUsageEvents<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Merchant connection operations.
    pub fn merchant_connection(&self) -> MerchantConnectionResource<'_> {
        MerchantConnectionResource { client: self }
    }

    /// Merchant binding operations.
    pub fn merchant_bindings(&self) -> MerchantBindings<'_> {
        MerchantBindings { client: self }
    }

    /// Merchant usage event operations.
    pub fn merchant_usage_events(&self) -> MerchantUsageEvents<'_> {
        MerchantUsageEvents { client: self }
    }
}

impl MerchantConnectionResource<'_> {
    /// Gets the merchant connection.
    pub fn get(&self) -> Call<GetMerchantConnection> {
        Call::new(
            self.client,
            Op::GetMerchantConnection,
            GetMerchantConnection,
        )
    }

    /// Creates the merchant connection. The credential is never returned.
    pub fn create(
        &self,
        provider: MerchantProvider,
        environment: MerchantEnvironment,
        credential: impl Into<String>,
    ) -> Call<CreateMerchantConnection> {
        Call::new(
            self.client,
            Op::CreateMerchantConnection,
            CreateMerchantConnection {
                provider,
                environment,
                credential: credential.into(),
            },
        )
    }

    /// Changes the merchant connection. Fields you do not set keep their values.
    pub fn update(&self) -> Call<UpdateMerchantConnection> {
        Call::new(
            self.client,
            Op::UpdateMerchantConnection,
            UpdateMerchantConnection::default(),
        )
    }

    /// Retires the merchant connection.
    pub fn retire(&self) -> Call<RetireMerchantConnection> {
        Call::new(
            self.client,
            Op::RetireMerchantConnection,
            RetireMerchantConnection,
        )
    }
}

impl MerchantBindings<'_> {
    /// Lists bindings between end users and merchant customers.
    pub fn list(&self) -> Call<ListMerchantBindings> {
        Call::new(
            self.client,
            Op::ListMerchantBindings,
            ListMerchantBindings::default(),
        )
    }

    /// Binds an end user to a merchant customer.
    pub fn set(
        &self,
        user_id: impl AsRef<str>,
        customer_id: impl Into<String>,
    ) -> Call<SetMerchantBinding> {
        Call::new(
            self.client,
            Op::SetMerchantBinding {
                user_id: user_id.as_ref(),
            },
            SetMerchantBinding {
                customer_id: customer_id.into(),
            },
        )
    }

    /// Removes an end user's binding.
    pub fn delete(&self, user_id: impl AsRef<str>) -> Call<DeleteMerchantBinding> {
        Call::new(
            self.client,
            Op::DeleteMerchantBinding {
                user_id: user_id.as_ref(),
            },
            DeleteMerchantBinding,
        )
    }
}

impl MerchantUsageEvents<'_> {
    /// Lists usage events.
    pub fn list(&self) -> Call<ListMerchantUsageEvents> {
        Call::new(
            self.client,
            Op::ListMerchantUsageEvents,
            ListMerchantUsageEvents::default(),
        )
    }

    /// Gets one usage event.
    pub fn get(&self, event_id: impl Into<MerchantUsageEventId>) -> Call<GetMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::new(
            self.client,
            Op::GetMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            GetMerchantUsageEvent,
        )
    }

    /// Discards a usage event so it is never billed.
    pub fn discard(
        &self,
        event_id: impl Into<MerchantUsageEventId>,
    ) -> Call<DiscardMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::new(
            self.client,
            Op::DiscardMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            DiscardMerchantUsageEvent,
        )
    }

    /// Releases a held usage event for billing.
    pub fn release(
        &self,
        event_id: impl Into<MerchantUsageEventId>,
    ) -> Call<ReleaseMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::new(
            self.client,
            Op::ReleaseMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            ReleaseMerchantUsageEvent,
        )
    }

    /// Retries a failed usage event.
    pub fn retry(
        &self,
        event_id: impl Into<MerchantUsageEventId>,
    ) -> Call<RetryMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::new(
            self.client,
            Op::RetryMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            RetryMerchantUsageEvent,
        )
    }

    /// Summarizes usage events over the last days.
    pub fn summary(&self) -> Call<GetMerchantUsageSummary> {
        Call::new(
            self.client,
            Op::GetMerchantUsageSummary,
            GetMerchantUsageSummary::default(),
        )
    }
}

crate::open_enum! {
    /// The merchant of record platform.
    pub enum MerchantProvider {
        /// `polar`
        Polar = "polar",
        /// `dodo`
        Dodo = "dodo",
    }
}

crate::open_enum! {
    /// The merchant platform environment.
    pub enum MerchantEnvironment {
        /// `sandbox`
        Sandbox = "sandbox",
        /// `live`
        Live = "live",
    }
}

/// Request state of [`MerchantConnectionResource::get`].
#[derive(Clone, Debug)]
pub struct GetMerchantConnection;
impl_request!(GetMerchantConnection => MerchantConnection; encode = none; decode = json);

/// Request state of [`MerchantConnectionResource::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMerchantConnection {
    provider: MerchantProvider,
    environment: MerchantEnvironment,
    credential: String,
}
impl_request!(CreateMerchantConnection => MerchantConnection; encode = json; decode = json;
    sensitive = (|request: &Self| vec![request.credential.clone()]));

/// Request state of [`MerchantConnectionResource::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMerchantConnection {}
impl_request!(UpdateMerchantConnection => MerchantConnection; encode = json; decode = json);

/// Request state of [`MerchantConnectionResource::retire`].
#[derive(Clone, Debug)]
pub struct RetireMerchantConnection;
impl_request!(RetireMerchantConnection => (); encode = none; decode = none);

/// Request state of [`MerchantBindings::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMerchantBindings {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListMerchantBindings => BindingsPage; encode = query; decode = json; paginate);

/// Request state of [`MerchantBindings::set`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMerchantBinding {
    customer_id: String,
}
impl_request!(SetMerchantBinding => MerchantBinding; encode = json; decode = json);

/// Request state of [`MerchantBindings::delete`].
#[derive(Clone, Debug)]
pub struct DeleteMerchantBinding;
impl_request!(DeleteMerchantBinding => (); encode = none; decode = none);

/// Request state of [`MerchantUsageEvents::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMerchantUsageEvents {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListMerchantUsageEvents => UsageEventsPage; encode = query; decode = json; paginate);

/// Request state of [`MerchantUsageEvents::get`].
#[derive(Clone, Debug)]
pub struct GetMerchantUsageEvent;
impl_request!(GetMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = json);

/// Request state of [`MerchantUsageEvents::discard`].
#[derive(Clone, Debug)]
pub struct DiscardMerchantUsageEvent;
impl_request!(DiscardMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = json);

/// Request state of [`MerchantUsageEvents::release`].
#[derive(Clone, Debug)]
pub struct ReleaseMerchantUsageEvent;
impl_request!(ReleaseMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = json);

/// Request state of [`MerchantUsageEvents::retry`].
#[derive(Clone, Debug)]
pub struct RetryMerchantUsageEvent;
impl_request!(RetryMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = json);

/// Request state of [`MerchantUsageEvents::summary`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMerchantUsageSummary {}
impl_request!(GetMerchantUsageSummary => MerchantUsageSummary; encode = query; decode = json);

/// The merchant connection. Its credential is never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantConnection {}

/// A binding between an end user and a merchant customer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantBinding {}

/// A page of merchant bindings.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct BindingsPage {
    /// The bindings on this page.
    pub bindings: Vec<MerchantBinding>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
}

impl Paginated for BindingsPage {
    type Item = MerchantBinding;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<MerchantBinding> {
        self.bindings
    }
}

/// A merchant usage event.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantUsageEvent {}

/// A page of merchant usage events.
#[derive(Clone, Debug, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct UsageEventsPage {
    /// The events on this page.
    pub events: Vec<MerchantUsageEvent>,
    /// The cursor of the next page, or `None` on the last page.
    pub next_cursor: Option<Cursor>,
}

impl Paginated for UsageEventsPage {
    type Item = MerchantUsageEvent;

    fn next_cursor(&self) -> Option<&Cursor> {
        self.next_cursor.as_ref()
    }

    fn into_items(self) -> Vec<MerchantUsageEvent> {
        self.events
    }
}

/// A summary of merchant usage events.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantUsageSummary {}
