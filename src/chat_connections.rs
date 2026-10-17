//! Chat platform connections (Slack, Telegram) and their deliveries. Tenant only;
//! start from [`Client::chat_connections`] and [`Client::chat_deliveries`].
//!
//! ```compile_fail
//! # fn example(client: blazing_agents::UserClient) {
//! client.chat_connections();
//! # }
//! ```
//!
//! Documentation: <https://docs.blazingagents.com/platform/chat-integrations>.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::{AgentId, ApprovalId, ChatConnectionId, ChatDeliveryId, SessionId, TenantId};
use crate::op::TenantOp;
use crate::pagination::{Cursor, Page};
use crate::types::Timestamp;

/// Chat connection operations. Returned by [`Client::chat_connections`].
#[derive(Clone, Copy, Debug)]
pub struct ChatConnections<'a> {
    client: &'a Client<Tenant>,
}

/// Tenant-wide chat delivery operations. Returned by [`Client::chat_deliveries`].
#[derive(Clone, Copy, Debug)]
pub struct ChatDeliveries<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Chat connection operations.
    pub fn chat_connections(&self) -> ChatConnections<'_> {
        ChatConnections { client: self }
    }

    /// Tenant-wide chat delivery operations.
    pub fn chat_deliveries(&self) -> ChatDeliveries<'_> {
        ChatDeliveries { client: self }
    }
}

impl ChatConnections<'_> {
    /// Lists chat connections.
    pub fn list(&self) -> Call<ListChatConnections> {
        Call::tenant(
            self.client,
            TenantOp::ListChatConnections,
            ListChatConnections,
        )
    }

    /// Connects an agent to a chat platform. Credentials are never returned.
    pub fn create(&self, connection: impl Into<NewChatConnection>) -> Call<CreateChatConnection> {
        Call::tenant(
            self.client,
            TenantOp::CreateChatConnection,
            CreateChatConnection {
                connection: connection.into(),
            },
        )
    }

    /// Gets one chat connection.
    pub fn get(&self, chat_connection_id: impl Into<ChatConnectionId>) -> Call<GetChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::GetChatConnection {
                id: chat_connection_id.as_str(),
            },
            GetChatConnection,
        )
    }

    /// Changes a chat connection's name or configuration. Set at least one
    /// field; the server rejects an empty update.
    pub fn update(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<RenameChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::RenameChatConnection {
                id: chat_connection_id.as_str(),
            },
            RenameChatConnection::default(),
        )
    }

    /// Deletes a chat connection.
    pub fn delete(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<DeleteChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::DeleteChatConnection {
                id: chat_connection_id.as_str(),
            },
            DeleteChatConnection,
        )
    }

    /// Resumes deliveries on a chat connection.
    pub fn enable(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<EnableChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::EnableChatConnection {
                id: chat_connection_id.as_str(),
            },
            EnableChatConnection,
        )
    }

    /// Pauses deliveries on a chat connection.
    pub fn disable(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<DisableChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::DisableChatConnection {
                id: chat_connection_id.as_str(),
            },
            DisableChatConnection,
        )
    }

    /// Checks a chat connection against its platform.
    pub fn check_health(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<CheckChatHealth> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::CheckChatHealth {
                id: chat_connection_id.as_str(),
            },
            CheckChatHealth,
        )
    }

    /// Replaces a chat connection's credentials.
    pub fn rotate_credentials(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
        credentials: ChatCredentials,
    ) -> Call<RotateChatCredentials> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::RotateChatCredentials {
                id: chat_connection_id.as_str(),
            },
            RotateChatCredentials { credentials },
        )
    }

    /// Lists one chat connection's deliveries.
    pub fn deliveries(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<ListChatDeliveries> {
        let chat_connection_id = chat_connection_id.into();
        Call::tenant(
            self.client,
            TenantOp::ListChatDeliveries {
                id: chat_connection_id.as_str(),
            },
            ListChatDeliveries::default(),
        )
    }

    /// Resends a failed or ambiguous delivery. `expected_attempt` is the
    /// delivery's current [`attempt`](ChatDelivery::attempt), 0 to 99, and the
    /// repair is refused if it has changed. Calling it confirms the sender of
    /// the previous attempt has stopped, and that you accept the message may
    /// appear twice because the platform may already have it.
    pub fn repair_delivery(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
        delivery_id: impl Into<ChatDeliveryId>,
        expected_attempt: u64,
    ) -> Call<RepairChatDelivery> {
        let chat_connection_id = chat_connection_id.into();
        let delivery_id = delivery_id.into();
        Call::tenant(
            self.client,
            TenantOp::RepairChatDelivery {
                id: chat_connection_id.as_str(),
                delivery_id: delivery_id.as_str(),
            },
            RepairChatDelivery {
                expected_attempt,
                previous_sender_stopped: true,
                accept_duplicate_risk: true,
            },
        )
    }
}

impl ChatDeliveries<'_> {
    /// Lists failed and ambiguous deliveries across chat connections.
    pub fn list(&self) -> Call<ListTenantChatDeliveries> {
        Call::tenant(
            self.client,
            TenantOp::ListTenantChatDeliveries,
            ListTenantChatDeliveries::default(),
        )
    }
}

crate::open_enum! {
    /// The chat platform a connection belongs to.
    pub enum ChatPlatform {
        /// `slack`
        Slack = "slack",
        /// `telegram`
        Telegram = "telegram",
    }
}

crate::open_enum! {
    /// What a chat delivery carries.
    pub enum ChatDeliveryKind {
        /// `reply`
        Reply = "reply",
        /// `card`
        Card = "card",
    }
}

crate::open_enum! {
    /// Where a chat delivery stands.
    pub enum ChatDeliveryStatus {
        /// `pending`
        Pending = "pending",
        /// `confirmed`
        Confirmed = "confirmed",
        /// `failed`
        Failed = "failed",
        /// `ambiguous`
        Ambiguous = "ambiguous",
    }
}

crate::open_enum! {
    /// The outcome of one health check.
    pub enum ChatHealthCheckStatus {
        /// `pass`
        Pass = "pass",
        /// `fail`
        Fail = "fail",
        /// `unknown`
        Unknown = "unknown",
    }
}

/// A status the tenant-wide delivery list can filter to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChatDeliveryListStatus {
    /// `failed`
    Failed,
    /// `ambiguous`
    Ambiguous,
}

impl ChatDeliveryListStatus {
    /// The wire value.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Ambiguous => "ambiguous",
        }
    }
}

/// A Slack app's bot token and signing secret. Both are write-only; the API
/// returns only [`credential_fragment`](ChatConnection::credential_fragment).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackCredentials {
    bot_token: String,
    signing_secret: String,
}

impl SlackCredentials {
    /// A Slack bot token and signing secret.
    pub fn new(bot_token: impl Into<String>, signing_secret: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            signing_secret: signing_secret.into(),
        }
    }

    fn secrets(&self) -> Vec<String> {
        vec![self.bot_token.clone(), self.signing_secret.clone()]
    }
}

impl fmt::Debug for SlackCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackCredentials")
            .field("bot_token", &"[REDACTED]")
            .field("signing_secret", &"[REDACTED]")
            .finish()
    }
}

/// A Telegram bot token from BotFather. Write-only; the API returns only
/// [`credential_fragment`](ChatConnection::credential_fragment).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelegramCredentials {
    bot_token: String,
}

impl TelegramCredentials {
    /// A Telegram bot token.
    pub fn new(bot_token: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
        }
    }

    fn secrets(&self) -> Vec<String> {
        vec![self.bot_token.clone()]
    }
}

impl fmt::Debug for TelegramCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelegramCredentials")
            .field("bot_token", &"[REDACTED]")
            .finish()
    }
}

/// A new chat connection: one variant per platform.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(tag = "platform", rename_all = "lowercase")]
pub enum NewChatConnection {
    /// A Slack connection.
    Slack(NewSlackConnection),
    /// A Telegram connection.
    Telegram(NewTelegramConnection),
}

impl NewChatConnection {
    /// A Slack connection for `agent_id` with the app's credentials.
    pub fn slack(
        agent_id: impl Into<AgentId>,
        name: impl Into<String>,
        credentials: SlackCredentials,
    ) -> Self {
        Self::Slack(NewSlackConnection::new(agent_id, name, credentials))
    }

    /// A Telegram connection for `agent_id` with the bot token.
    pub fn telegram(
        agent_id: impl Into<AgentId>,
        name: impl Into<String>,
        credentials: TelegramCredentials,
    ) -> Self {
        Self::Telegram(NewTelegramConnection::new(agent_id, name, credentials))
    }

    fn secrets(&self) -> Vec<String> {
        match self {
            Self::Slack(connection) => connection.credentials.secrets(),
            Self::Telegram(connection) => connection.credentials.secrets(),
        }
    }
}

impl From<NewSlackConnection> for NewChatConnection {
    fn from(connection: NewSlackConnection) -> Self {
        Self::Slack(connection)
    }
}

impl From<NewTelegramConnection> for NewChatConnection {
    fn from(connection: NewTelegramConnection) -> Self {
        Self::Telegram(connection)
    }
}

/// A new Slack chat connection.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct NewSlackConnection {
    agent_id: AgentId,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<NewSlackConfiguration>,
    credentials: SlackCredentials,
}

impl NewSlackConnection {
    /// A Slack connection for `agent_id`. `name` is 1 to 80 characters.
    pub fn new(
        agent_id: impl Into<AgentId>,
        name: impl Into<String>,
        credentials: SlackCredentials,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            enabled: None,
            configuration: None,
            credentials,
        }
    }

    /// Whether the connection accepts messages. Defaults to `true`.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Channel IDs health checks look at, up to 20. They do not restrict who
    /// can talk to the agent.
    pub fn channel_ids(mut self, channel_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.configuration
            .get_or_insert_with(Default::default)
            .channel_ids = Some(channel_ids.into_iter().map(Into::into).collect());
        self
    }
}

/// The optional settings of a new Slack connection.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NewSlackConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_ids: Option<Vec<String>>,
}

/// A new Telegram chat connection.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct NewTelegramConnection {
    agent_id: AgentId,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<NewTelegramConfiguration>,
    credentials: TelegramCredentials,
}

impl NewTelegramConnection {
    /// A Telegram connection for `agent_id`. `name` is 1 to 80 characters.
    pub fn new(
        agent_id: impl Into<AgentId>,
        name: impl Into<String>,
        credentials: TelegramCredentials,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            enabled: None,
            configuration: None,
            credentials,
        }
    }

    /// Whether the connection accepts messages. Defaults to `true`.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Chat IDs health checks look at, up to 20. They do not restrict who can
    /// talk to the agent.
    pub fn chat_ids(mut self, chat_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.configuration
            .get_or_insert_with(Default::default)
            .chat_ids = Some(chat_ids.into_iter().map(Into::into).collect());
        self
    }

    /// Whether the bot runs in Telegram Business mode. Defaults to `false`.
    pub fn business_mode(mut self, business_mode: bool) -> Self {
        self.configuration
            .get_or_insert_with(Default::default)
            .business_mode = Some(business_mode);
        self
    }
}

/// The optional settings of a new Telegram connection.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct NewTelegramConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    business_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_ids: Option<Vec<String>>,
}

/// Replacement credentials: one variant per platform. The platform must match
/// the connection's.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(tag = "platform", rename_all = "lowercase")]
pub enum ChatCredentials {
    /// Slack credentials.
    Slack(SlackCredentials),
    /// Telegram credentials.
    Telegram(TelegramCredentials),
}

impl ChatCredentials {
    /// A Slack bot token and signing secret.
    pub fn slack(bot_token: impl Into<String>, signing_secret: impl Into<String>) -> Self {
        Self::Slack(SlackCredentials::new(bot_token, signing_secret))
    }

    /// A Telegram bot token.
    pub fn telegram(bot_token: impl Into<String>) -> Self {
        Self::Telegram(TelegramCredentials::new(bot_token))
    }

    fn secrets(&self) -> Vec<String> {
        match self {
            Self::Slack(credentials) => credentials.secrets(),
            Self::Telegram(credentials) => credentials.secrets(),
        }
    }
}

/// Request state of [`ChatConnections::list`].
#[derive(Clone, Debug)]
pub struct ListChatConnections;
impl_request!(ListChatConnections => ChatConnectionList; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct CreateChatConnection {
    connection: NewChatConnection,
}
impl_request!(CreateChatConnection => ChatConnection; encode = json; decode = credential_safe_json;
    sensitive = (|request: &Self| request.connection.secrets()));

/// Request state of [`ChatConnections::get`].
#[derive(Clone, Debug)]
pub struct GetChatConnection;
impl_request!(GetChatConnection => ChatConnection; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameChatConnection {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<ChatConfigurationUpdate>,
}
impl_request!(RenameChatConnection => ChatConnection; encode = json; decode = credential_safe_json);

/// The configuration fields of a chat connection update. Only the set keys are
/// sent, and the object itself is left out when none are.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatConfigurationUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    business_mode: Option<bool>,
}

impl Call<RenameChatConnection> {
    /// A new display name, 1 to 80 characters.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.map(|request| request.name = Some(name.into()))
    }

    /// Slack channel IDs health checks look at, up to 20.
    pub fn channel_ids(self, channel_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.map(|request| {
            request
                .configuration
                .get_or_insert_with(Default::default)
                .channel_ids = Some(channel_ids.into_iter().map(Into::into).collect());
        })
    }

    /// Telegram chat IDs health checks look at, up to 20.
    pub fn chat_ids(self, chat_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.map(|request| {
            request
                .configuration
                .get_or_insert_with(Default::default)
                .chat_ids = Some(chat_ids.into_iter().map(Into::into).collect());
        })
    }

    /// Whether a Telegram bot runs in Business mode.
    pub fn business_mode(self, business_mode: bool) -> Self {
        self.map(|request| {
            request
                .configuration
                .get_or_insert_with(Default::default)
                .business_mode = Some(business_mode);
        })
    }
}

/// Request state of [`ChatConnections::delete`].
#[derive(Clone, Debug)]
pub struct DeleteChatConnection;
impl_request!(DeleteChatConnection => (); encode = none; decode = none);

/// Request state of [`ChatConnections::enable`].
#[derive(Clone, Debug)]
pub struct EnableChatConnection;
impl_request!(EnableChatConnection => ChatConnection; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::disable`].
#[derive(Clone, Debug)]
pub struct DisableChatConnection;
impl_request!(DisableChatConnection => ChatConnection; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::check_health`].
#[derive(Clone, Debug)]
pub struct CheckChatHealth;
impl_request!(CheckChatHealth => ChatConnection; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::rotate_credentials`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct RotateChatCredentials {
    credentials: ChatCredentials,
}
impl_request!(RotateChatCredentials => ChatConnection; encode = json; decode = credential_safe_json;
    sensitive = (|request: &Self| request.credentials.secrets()));

/// Request state of [`ChatConnections::deliveries`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListChatDeliveries {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListChatDeliveries => Page<ChatDelivery>; encode = query; decode = json; paginate);

impl Call<ListChatDeliveries> {
    /// Deliveries per page, 1 to 100. Defaults to 100.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// Request state of [`ChatConnections::repair_delivery`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChatDelivery {
    expected_attempt: u64,
    previous_sender_stopped: bool,
    accept_duplicate_risk: bool,
}
impl_request!(RepairChatDelivery => ChatDeliveryResult; encode = json; decode = json);

/// Request state of [`ChatDeliveries::list`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTenantChatDeliveries {
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u8>,
}
impl_request!(ListTenantChatDeliveries => Page<TenantChatDelivery>; encode = query; decode = json; paginate);

impl Call<ListTenantChatDeliveries> {
    /// Only deliveries with these statuses, sent comma-separated. An empty
    /// iterator leaves the filter out.
    pub fn status(self, status: impl IntoIterator<Item = ChatDeliveryListStatus>) -> Self {
        self.map(|request| {
            let status = status
                .into_iter()
                .map(|status| status.as_str().to_owned())
                .collect::<Vec<_>>()
                .join(",");
            request.status = (!status.is_empty()).then_some(status);
        })
    }

    /// Only deliveries created at or after this instant.
    pub fn since(self, since: Timestamp) -> Self {
        self.map(|request| request.since = Some(since))
    }

    /// Deliveries per page, 1 to 100. Defaults to 50.
    pub fn limit(self, limit: u8) -> Self {
        self.map(|request| request.limit = Some(limit))
    }
}

/// A chat connection. Its credentials are never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatConnection {
    /// The connection's ID.
    pub id: ChatConnectionId,
    /// The tenant that owns the connection.
    pub tenant_id: TenantId,
    /// The agent that answers messages on this connection.
    pub agent_id: AgentId,
    /// The display name.
    pub name: String,
    /// The chat platform.
    pub platform: ChatPlatform,
    /// Whether the connection accepts messages.
    pub enabled: bool,
    /// The platform settings.
    pub configuration: ChatConfiguration,
    /// The bot's identity on the platform.
    pub identity: ChatIdentity,
    /// The last health check.
    pub health: ChatHealth,
    /// The last characters of the credential, for display.
    pub credential_fragment: String,
    /// The credential's version, incremented on each rotation.
    pub credential_version: u64,
    /// When the connection was created.
    pub created_at: Timestamp,
    /// When the connection last changed.
    pub updated_at: Timestamp,
    /// The webhook URL registered with the platform.
    pub webhook_url: String,
}

/// A chat connection's platform settings: one variant per platform.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ChatConfiguration {
    /// Slack settings.
    Slack(SlackConfiguration),
    /// Telegram settings.
    Telegram(TelegramConfiguration),
    /// The raw object for a platform this version of the SDK does not know.
    Unrecognized(serde_json::Value),
}

impl<'de> Deserialize<'de> for ChatConfiguration {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::types::decode_tagged(
            deserializer,
            "platform",
            |platform, raw| match platform {
                "slack" => Some(SlackConfiguration::deserialize(raw).map(Self::Slack)),
                "telegram" => Some(TelegramConfiguration::deserialize(raw).map(Self::Telegram)),
                _ => None,
            },
            |raw| Self::Unrecognized(serde_json::Value::Object(raw)),
        )
    }
}

impl Serialize for ChatConfiguration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        /// The known variants, serialized with the platform tag.
        #[derive(Serialize)]
        #[serde(tag = "platform", rename_all = "lowercase")]
        enum Known<'a> {
            Slack(&'a SlackConfiguration),
            Telegram(&'a TelegramConfiguration),
        }

        match self {
            Self::Slack(configuration) => Known::Slack(configuration).serialize(serializer),
            Self::Telegram(configuration) => Known::Telegram(configuration).serialize(serializer),
            Self::Unrecognized(value) => value.serialize(serializer),
        }
    }
}

/// The Slack settings of a chat connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SlackConfiguration {
    /// Channel IDs health checks look at.
    #[serde(default)]
    pub channel_ids: Vec<String>,
}

/// The Telegram settings of a chat connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TelegramConfiguration {
    /// Whether the bot runs in Business mode.
    #[serde(default)]
    pub business_mode: bool,
    /// Chat IDs health checks look at.
    #[serde(default)]
    pub chat_ids: Vec<String>,
}

/// The bot's identity on the chat platform.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatIdentity {
    /// The bot's ID on the platform.
    pub bot_id: String,
    /// The bot user's ID on the platform.
    pub bot_user_id: String,
    /// The workspace or team ID, when the platform has one.
    pub team_id: Option<String>,
    /// The platform app's ID, when the platform has one.
    pub app_id: Option<String>,
}

/// The last health check of a chat connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatHealth {
    /// When the check ran.
    pub checked_at: Timestamp,
    /// Whether the credential authenticated.
    pub token_valid: bool,
    /// Whether the platform reported the bot's identity.
    pub identity_verified: bool,
    /// The individual checks.
    pub checks: Vec<ChatHealthCheck>,
}

/// One check in a chat connection's health report.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatHealthCheck {
    /// The check's code.
    pub code: String,
    /// The check's outcome.
    pub status: ChatHealthCheckStatus,
    /// What the check looked at, when it names one.
    #[serde(default)]
    pub subject: Option<String>,
}

/// The tenant's chat connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatConnectionList {
    /// The connections.
    pub chat_connections: Vec<ChatConnection>,
}

/// One delivery of a chat connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatDelivery {
    /// The delivery's ID.
    pub id: ChatDeliveryId,
    /// What the delivery carries.
    pub kind: ChatDeliveryKind,
    /// Where the delivery stands.
    pub status: ChatDeliveryStatus,
    /// How many sends have been tried.
    pub attempt: u64,
    /// The credential version the delivery used.
    pub credential_version: u64,
    /// What the delivery sent, as text.
    pub representation: String,
    /// Why the delivery failed, when it did.
    pub diagnostic: Option<String>,
    /// The platform's receipts, in its own shape.
    #[serde(default)]
    pub receipts: Option<serde_json::Value>,
    /// The session the message belongs to.
    pub session_id: SessionId,
    /// The message's ID, when one exists.
    pub message_id: Option<String>,
    /// The approval the message answers, when it is one.
    pub approval_id: Option<ApprovalId>,
    /// The platform thread the delivery belongs to.
    pub thread_id: String,
    /// When the delivery was created.
    pub created_at: Timestamp,
    /// When the delivery last changed.
    pub updated_at: Timestamp,
}

/// One delivery in the tenant-wide list, with its connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TenantChatDelivery {
    /// The delivery's ID.
    pub id: ChatDeliveryId,
    /// What the delivery carries.
    pub kind: ChatDeliveryKind,
    /// Where the delivery stands.
    pub status: ChatDeliveryStatus,
    /// How many sends have been tried.
    pub attempt: u64,
    /// The credential version the delivery used.
    pub credential_version: u64,
    /// What the delivery sent, as text.
    pub representation: String,
    /// Why the delivery failed, when it did.
    pub diagnostic: Option<String>,
    /// The platform's receipts, in its own shape.
    #[serde(default)]
    pub receipts: Option<serde_json::Value>,
    /// The session the message belongs to.
    pub session_id: SessionId,
    /// The message's ID, when one exists.
    pub message_id: Option<String>,
    /// The approval the message answers, when it is one.
    pub approval_id: Option<ApprovalId>,
    /// The platform thread the delivery belongs to.
    pub thread_id: String,
    /// When the delivery was created.
    pub created_at: Timestamp,
    /// When the delivery last changed.
    pub updated_at: Timestamp,
    /// The connection the delivery went through.
    pub connection_id: ChatConnectionId,
    /// The agent that produced the message.
    pub agent_id: AgentId,
    /// The chat platform.
    pub platform: ChatPlatform,
}

/// The outcome of repairing a delivery.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatDeliveryResult {
    /// The delivery's ID.
    pub id: ChatDeliveryId,
    /// The session the message belongs to.
    pub session_id: SessionId,
    /// The platform thread the delivery belongs to.
    pub thread_id: String,
    /// The connection the delivery went through.
    pub connection_id: ChatConnectionId,
    /// What the delivery carries.
    pub kind: ChatDeliveryKind,
    /// Where the delivery stands after the repair.
    pub status: ChatDeliveryStatus,
    /// How many sends have been tried.
    pub attempt: u64,
}
