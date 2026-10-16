//! Chat platform connections (Slack, Telegram) and their deliveries. Tenant only;
//! start from [`Client::chat_connections`] and [`Client::chat_deliveries`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::{ChatConnectionId, ChatDeliveryId};
use crate::op::Op;
use crate::pagination::{Cursor, Page};

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
        Call::new(self.client, Op::ListChatConnections, ListChatConnections)
    }

    /// Connects an agent to a chat platform. Credentials are never returned.
    pub fn create(&self, connection: NewChatConnection) -> Call<CreateChatConnection> {
        Call::new(
            self.client,
            Op::CreateChatConnection,
            CreateChatConnection { connection },
        )
    }

    /// Gets one chat connection.
    pub fn get(&self, chat_connection_id: impl Into<ChatConnectionId>) -> Call<GetChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::new(
            self.client,
            Op::GetChatConnection {
                id: chat_connection_id.as_str(),
            },
            GetChatConnection,
        )
    }

    /// Changes a chat connection's name or configuration.
    pub fn update(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
    ) -> Call<RenameChatConnection> {
        let chat_connection_id = chat_connection_id.into();
        Call::new(
            self.client,
            Op::RenameChatConnection {
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
        Call::new(
            self.client,
            Op::DeleteChatConnection {
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
        Call::new(
            self.client,
            Op::EnableChatConnection {
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
        Call::new(
            self.client,
            Op::DisableChatConnection {
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
        Call::new(
            self.client,
            Op::CheckChatHealth {
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
        Call::new(
            self.client,
            Op::RotateChatCredentials {
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
        Call::new(
            self.client,
            Op::ListChatDeliveries {
                id: chat_connection_id.as_str(),
            },
            ListChatDeliveries::default(),
        )
    }

    /// Resends a failed or ambiguous delivery. `expected_attempt` is the attempt
    /// you inspected; the other flags acknowledge what you verified.
    pub fn repair_delivery(
        &self,
        chat_connection_id: impl Into<ChatConnectionId>,
        delivery_id: impl Into<ChatDeliveryId>,
        expected_attempt: u64,
        previous_sender_stopped: bool,
        accept_duplicate_risk: bool,
    ) -> Call<RepairChatDelivery> {
        let chat_connection_id = chat_connection_id.into();
        let delivery_id = delivery_id.into();
        Call::new(
            self.client,
            Op::RepairChatDelivery {
                id: chat_connection_id.as_str(),
                delivery_id: delivery_id.as_str(),
            },
            RepairChatDelivery {
                expected_attempt,
                previous_sender_stopped,
                accept_duplicate_risk,
            },
        )
    }
}

impl ChatDeliveries<'_> {
    /// Lists failed and ambiguous deliveries across chat connections.
    pub fn list(&self) -> Call<ListTenantChatDeliveries> {
        Call::new(
            self.client,
            Op::ListTenantChatDeliveries,
            ListTenantChatDeliveries::default(),
        )
    }
}

/// A new chat connection: one variant per platform.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(untagged)]
pub enum NewChatConnection {}

/// Replacement credentials: one variant per platform.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
#[serde(untagged)]
pub enum ChatCredentials {}

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
impl_request!(CreateChatConnection => ChatConnection; encode = json; decode = credential_safe_json);

/// Request state of [`ChatConnections::get`].
#[derive(Clone, Debug)]
pub struct GetChatConnection;
impl_request!(GetChatConnection => ChatConnection; encode = none; decode = credential_safe_json);

/// Request state of [`ChatConnections::update`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameChatConnection {}
impl_request!(RenameChatConnection => ChatConnection; encode = json; decode = credential_safe_json);

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
impl_request!(RotateChatCredentials => ChatConnection; encode = json; decode = credential_safe_json);

/// Request state of [`ChatConnections::deliveries`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListChatDeliveries {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<Cursor>,
}
impl_request!(ListChatDeliveries => Page<ChatDelivery>; encode = query; decode = json; paginate);

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
    cursor: Option<Cursor>,
}
impl_request!(ListTenantChatDeliveries => Page<TenantChatDelivery>; encode = query; decode = json; paginate);

/// A chat connection. Its credentials are never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatConnection {}

/// The tenant's chat connections.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatConnectionList {}

/// One delivery of a chat connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatDelivery {}

/// One delivery in the tenant-wide list, with its connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TenantChatDelivery {}

/// The outcome of repairing a delivery.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ChatDeliveryResult {}
