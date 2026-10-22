//! Merchant billing: the tenant's merchant connection, its customer bindings and
//! usage events. Tenant only; start from [`Client::merchant_connection`],
//! [`Client::merchant_bindings`] and [`Client::merchant_usage_events`].
//!
//! ```compile_fail
//! # fn example(client: blazing_agents::UserClient) {
//! client.merchant_connection();
//! # }
//! ```
//!
//! Documentation: <https://docs.blazingagents.com/platform/monetization>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, Raw, impl_request};
use crate::client::{Client, Tenant};
use crate::error::Result;
use crate::ids::{AgentId, MerchantConnectionId, MerchantUsageEventId, SessionId, TurnId};
use crate::op::TenantOp;
use crate::pagination::{Cursor, Paginated};
use crate::providers::ProviderType;
use crate::types::Timestamp;

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
    /// Gets the merchant connection, or `None` when the tenant has none.
    pub fn get(&self) -> Call<GetMerchantConnection> {
        Call::tenant(
            self.client,
            TenantOp::GetMerchantConnection,
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
        Call::tenant(
            self.client,
            TenantOp::CreateMerchantConnection,
            CreateMerchantConnection {
                provider,
                environment,
                credential: credential.into(),
                guard: None,
            },
        )
    }

    /// Changes the merchant connection. Fields you do not set keep their values.
    pub fn update(&self) -> Call<UpdateMerchantConnection> {
        Call::tenant(
            self.client,
            TenantOp::UpdateMerchantConnection,
            UpdateMerchantConnection::default(),
        )
    }

    /// Retires the merchant connection. While a turn admitted on it is running
    /// or one of its usage events is `pending`, the request fails with
    /// `merchant_connection_in_use`; retry once they finish. Retiring discards
    /// its `failed` events and deletes its credential.
    pub fn retire(&self) -> Call<RetireMerchantConnection> {
        Call::tenant(
            self.client,
            TenantOp::RetireMerchantConnection,
            RetireMerchantConnection,
        )
    }
}

impl MerchantBindings<'_> {
    /// Lists bindings between end users and merchant customers.
    pub fn list(&self) -> Call<ListMerchantBindings> {
        Call::tenant(
            self.client,
            TenantOp::ListMerchantBindings,
            ListMerchantBindings::default(),
        )
    }

    /// Binds an end user to a merchant customer, replacing any existing
    /// binding.
    pub fn put(
        &self,
        user_id: impl AsRef<str>,
        customer_id: impl Into<String>,
    ) -> Call<SetMerchantBinding> {
        Call::tenant(
            self.client,
            TenantOp::SetMerchantBinding {
                user_id: user_id.as_ref(),
            },
            SetMerchantBinding {
                customer_id: customer_id.into(),
            },
        )
    }

    /// Removes an end user's binding.
    pub fn delete(&self, user_id: impl AsRef<str>) -> Call<DeleteMerchantBinding> {
        Call::tenant(
            self.client,
            TenantOp::DeleteMerchantBinding {
                user_id: user_id.as_ref(),
            },
            DeleteMerchantBinding,
        )
    }
}

impl MerchantUsageEvents<'_> {
    /// Lists usage events.
    pub fn list(&self) -> Call<ListMerchantUsageEvents> {
        Call::tenant(
            self.client,
            TenantOp::ListMerchantUsageEvents,
            ListMerchantUsageEvents::default(),
        )
    }

    /// Gets one usage event.
    pub fn get(&self, event_id: impl Into<MerchantUsageEventId>) -> Call<GetMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::tenant(
            self.client,
            TenantOp::GetMerchantUsageEvent {
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
        Call::tenant(
            self.client,
            TenantOp::DiscardMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            DiscardMerchantUsageEvent,
        )
    }

    /// Retries a failed usage event.
    pub fn retry(
        &self,
        event_id: impl Into<MerchantUsageEventId>,
    ) -> Call<RetryMerchantUsageEvent> {
        let event_id = event_id.into();
        Call::tenant(
            self.client,
            TenantOp::RetryMerchantUsageEvent {
                event_id: event_id.as_str(),
            },
            RetryMerchantUsageEvent,
        )
    }

    /// Summarizes usage events over the last days.
    pub fn summary(&self) -> Call<GetMerchantUsageSummary> {
        Call::tenant(
            self.client,
            TenantOp::GetMerchantUsageSummary,
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

crate::open_enum! {
    /// Where a merchant usage event stands.
    pub enum MerchantUsageEventStatus {
        /// `pending`
        Pending = "pending",
        /// `accepted`
        Accepted = "accepted",
        /// `failed`
        Failed = "failed",
        /// `incomplete`
        Incomplete = "incomplete",
        /// `discarded`
        Discarded = "discarded",
        /// `expired`
        Expired = "expired",
    }
}

crate::open_enum! {
    /// How the turn behind a usage event ended.
    pub enum MerchantTurnStatus {
        /// `succeeded`
        Succeeded = "succeeded",
        /// `cancelled`
        Cancelled = "cancelled",
        /// `failed`
        Failed = "failed",
    }
}

crate::open_enum! {
    /// What the tenant should do next about a usage event.
    pub enum MerchantNextAction {
        /// `none`
        None = "none",
        /// `wait`
        Wait = "wait",
        /// `retry`
        Retry = "retry",
        /// `investigate`
        Investigate = "investigate",
        /// `discard`
        Discard = "discard",
    }
}

/// Request state of [`MerchantConnectionResource::get`].
#[derive(Clone, Debug)]
pub struct GetMerchantConnection;
impl_request!(GetMerchantConnection => Option<MerchantConnection>; encode = none;
    decode = (decode_optional_connection));

/// Request state of [`MerchantConnectionResource::create`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMerchantConnection {
    provider: MerchantProvider,
    environment: MerchantEnvironment,
    credential: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    guard: Option<MerchantGuard>,
}
impl_request!(CreateMerchantConnection => MerchantConnection; encode = json;
    decode = (decode_connection);
    sensitive = (|request: &Self| vec![request.credential.clone()]));

impl Call<CreateMerchantConnection> {
    /// A pre-turn plan or balance check with the provider. Off when left out.
    pub fn guard(self, guard: MerchantGuard) -> Self {
        self.map(|request| request.guard = Some(guard))
    }
}

/// Request state of [`MerchantConnectionResource::update`].
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMerchantConnection {
    #[serde(skip_serializing_if = "Option::is_none")]
    credential: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guard: Option<MerchantGuard>,
}
impl_request!(UpdateMerchantConnection => MerchantConnection; encode = json;
    decode = (decode_connection);
    sensitive = (|request: &Self| request.credential.iter().cloned().collect()));

impl Call<UpdateMerchantConnection> {
    /// A replacement credential for the same merchant account.
    pub fn credential(self, credential: impl Into<String>) -> Self {
        self.map(|request| request.credential = Some(credential.into()))
    }

    /// The complete guard configuration, replacing the current one.
    pub fn guard(self, guard: MerchantGuard) -> Self {
        self.map(|request| request.guard = Some(guard))
    }
}

/// Request state of [`MerchantConnectionResource::retire`].
#[derive(Clone, Debug)]
pub struct RetireMerchantConnection;
impl_request!(RetireMerchantConnection => (); encode = none; decode = none);

/// Request state of [`MerchantBindings::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMerchantBindings {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListMerchantBindings => BindingsPage; encode = query; decode = json; paginate);

impl Call<ListMerchantBindings> {
    /// Only the binding of this end user.
    pub fn user_id(self, user_id: impl Into<String>) -> Self {
        self.map(|request| request.user_id = Some(user_id.into()))
    }

    /// Bindings per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`MerchantBindings::put`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMerchantBinding {
    customer_id: String,
}
impl_request!(SetMerchantBinding => MerchantBinding; encode = json; decode = (decode_binding));

/// Request state of [`MerchantBindings::delete`].
#[derive(Clone, Debug)]
pub struct DeleteMerchantBinding;
impl_request!(DeleteMerchantBinding => (); encode = none; decode = none);

/// Request state of [`MerchantUsageEvents::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMerchantUsageEvents {
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<MerchantUsageEventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListMerchantUsageEvents => UsageEventsPage; encode = query; decode = json; paginate);

impl Call<ListMerchantUsageEvents> {
    /// Only events with this delivery status.
    pub fn status(self, status: MerchantUsageEventStatus) -> Self {
        self.map(|request| request.status = Some(status))
    }

    /// Events per page, 1 to 200. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`MerchantUsageEvents::get`].
#[derive(Clone, Debug)]
pub struct GetMerchantUsageEvent;
impl_request!(GetMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = (decode_event));

/// Request state of [`MerchantUsageEvents::discard`].
#[derive(Clone, Debug)]
pub struct DiscardMerchantUsageEvent;
impl_request!(DiscardMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = (decode_event));

/// Request state of [`MerchantUsageEvents::retry`].
#[derive(Clone, Debug)]
pub struct RetryMerchantUsageEvent;
impl_request!(RetryMerchantUsageEvent => MerchantUsageEvent; encode = none; decode = (decode_event));

/// Request state of [`MerchantUsageEvents::summary`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMerchantUsageSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    days: Option<u8>,
}
impl_request!(GetMerchantUsageSummary => MerchantUsageSummary; encode = query; decode = (decode_summary));

impl Call<GetMerchantUsageSummary> {
    /// UTC days in the `daily` series, ending today, 1 to 90. Defaults to 14.
    pub fn days(self, days: u8) -> Self {
        self.map(|request| request.days = Some(days))
    }
}

#[derive(Deserialize)]
struct ConnectionEnvelope {
    connection: MerchantConnection,
}

#[derive(Deserialize)]
struct OptionalConnectionEnvelope {
    connection: Option<MerchantConnection>,
}

#[derive(Deserialize)]
struct BindingEnvelope {
    binding: MerchantBinding,
}

#[derive(Deserialize)]
struct EventEnvelope {
    event: MerchantUsageEvent,
}

#[derive(Deserialize)]
struct SummaryEnvelope {
    summary: MerchantUsageSummary,
}

async fn decode_connection(raw: Raw) -> Result<MerchantConnection> {
    Ok(raw.json::<ConnectionEnvelope>().await?.connection)
}

async fn decode_optional_connection(raw: Raw) -> Result<Option<MerchantConnection>> {
    Ok(raw.json::<OptionalConnectionEnvelope>().await?.connection)
}

async fn decode_binding(raw: Raw) -> Result<MerchantBinding> {
    raw.json::<BindingEnvelope>()
        .await
        .map(|envelope| envelope.binding)
}

async fn decode_event(raw: Raw) -> Result<MerchantUsageEvent> {
    raw.json::<EventEnvelope>()
        .await
        .map(|envelope| envelope.event)
}

async fn decode_summary(raw: Raw) -> Result<MerchantUsageSummary> {
    raw.json::<SummaryEnvelope>()
        .await
        .map(|envelope| envelope.summary)
}

/// The merchant connection. Its credential is never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantConnection {
    /// The connection's ID (`mch_…`).
    pub id: MerchantConnectionId,
    /// The merchant of record platform.
    pub provider: MerchantProvider,
    /// The provider environment.
    pub environment: MerchantEnvironment,
    /// The account the credential belongs to at the provider.
    pub merchant_account_id: String,
    /// The last characters of the credential, for display.
    pub key_fragment: String,
    /// The pre-turn plan or balance check.
    pub guard: MerchantGuard,
    /// The configuration's version, incremented on each change.
    pub config_version: u64,
    /// When the connection was created.
    pub created_at: Timestamp,
    /// When the connection last changed.
    pub updated_at: Timestamp,
}

/// A pre-turn plan or balance check with the merchant provider, and the input
/// that sets one. `meter_id` is always sent, `null` when unset.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantGuard {
    /// Whether the guard checks each end user before a turn starts.
    pub enabled: bool,
    /// The provider product IDs a plan must hold, up to 100.
    pub product_ids: Vec<String>,
    /// The provider meter that meters usage, when the provider needs one.
    pub meter_id: Option<String>,
}

impl MerchantGuard {
    /// A guard that checks each end user (`true`) or is off (`false`).
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            product_ids: Vec::new(),
            meter_id: None,
        }
    }

    /// The provider product IDs a plan must hold, up to 100.
    pub fn product_ids(mut self, product_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.product_ids = product_ids.into_iter().map(Into::into).collect();
        self
    }

    /// The provider meter that meters usage.
    pub fn meter_id(mut self, meter_id: impl Into<String>) -> Self {
        self.meter_id = Some(meter_id.into());
        self
    }
}

/// A binding between an end user and a merchant customer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantBinding {
    /// The end user's ID.
    pub user_id: String,
    /// The customer ID in the merchant provider.
    pub customer_id: String,
    /// When the binding was created.
    pub created_at: Timestamp,
    /// When the binding last changed.
    pub updated_at: Timestamp,
}

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
pub struct MerchantUsageEvent {
    /// The event's ID (`mev_…`).
    pub id: MerchantUsageEventId,
    /// The turn the event bills.
    pub turn_id: TurnId,
    /// The merchant connection the event goes through (`mch_…`).
    pub connection_id: MerchantConnectionId,
    /// The merchant of record platform.
    pub provider: MerchantProvider,
    /// The end user the event bills.
    pub user_id: String,
    /// The bound provider customer, when one is bound.
    pub customer_id: Option<String>,
    /// The agent that ran the turn.
    pub agent_id: AgentId,
    /// The session the turn belongs to, when it has one.
    pub session_id: Option<SessionId>,
    /// The model the turn ran.
    pub model: String,
    /// The model's vendor.
    pub model_provider: ProviderType,
    /// Billed input tokens.
    pub input_tokens: u64,
    /// Billed output tokens.
    pub output_tokens: u64,
    /// Billed tokens in total.
    pub total_tokens: u64,
    /// How the turn ended.
    pub turn_status: MerchantTurnStatus,
    /// When the turn started.
    pub started_at: Timestamp,
    /// When the usage occurred.
    pub occurred_at: Timestamp,
    /// Where the event's delivery stands.
    pub status: MerchantUsageEventStatus,
    /// The provider's record of the delivery, in its own shape.
    pub payload: Option<serde_json::Map<String, serde_json::Value>>,
    /// How many deliveries have been tried.
    pub attempt_count: u64,
    /// The delivery workflow's generation.
    pub attempt_generation: u64,
    /// When the last delivery attempt ran.
    pub last_attempt_at: Option<Timestamp>,
    /// Why the last attempt failed, when it did.
    pub last_error_code: Option<String>,
    /// When the provider accepted the event.
    pub accepted_at: Option<Timestamp>,
    /// What the tenant should do next about the event.
    pub next_action: MerchantNextAction,
}

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
pub struct MerchantUsageSummary {
    /// All-time counts by delivery status.
    pub counts: MerchantUsageCounts,
    /// When the provider last accepted an event.
    pub last_accepted_at: Option<Timestamp>,
    /// When the oldest still-pending event's usage occurred.
    pub oldest_pending_occurred_at: Option<Timestamp>,
    /// Per-day delivery totals for the requested window.
    pub daily: Vec<MerchantUsageDay>,
}

/// All-time counts of merchant usage events by delivery status.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantUsageCounts {
    /// Events waiting to be delivered.
    pub pending: u64,
    /// Events the provider accepted.
    pub accepted: u64,
    /// Events whose delivery failed.
    pub failed: u64,
    /// Events that could not be built.
    pub incomplete: u64,
    /// Events that missed Dodo's one-hour window.
    pub expired: u64,
    /// Events never billed: the tenant discarded them, or their end user had
    /// no customer binding.
    pub discarded: u64,
}

/// One UTC day of merchant usage deliveries.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct MerchantUsageDay {
    /// The day, `YYYY-MM-DD` UTC.
    pub day: String,
    /// Events the provider accepted that day.
    pub accepted_events: u64,
    /// Tokens in the accepted events.
    pub accepted_tokens: u64,
    /// Events from that day not yet delivered.
    pub pending_events: u64,
}
