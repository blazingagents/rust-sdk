//! Model providers. Tenant only; start from [`Client::providers`].
//!
//! Documentation: <https://docs.blazingagents.com/agents/providers-and-models>.

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::ProviderId;
use crate::op::TenantOp;
use crate::types::Timestamp;

/// Provider operations. Returned by [`Client::providers`].
#[derive(Clone, Copy, Debug)]
pub struct Providers<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Provider operations.
    ///
    /// ```compile_fail
    /// # fn f(client: blazing_agents::UserClient) {
    /// client.providers();
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # fn f(client: blazing_agents::Client) {
    /// let providers = client.providers();
    /// # }
    /// ```
    pub fn providers(&self) -> Providers<'_> {
        Providers { client: self }
    }
}

impl Providers<'_> {
    /// Lists providers.
    pub fn list(&self) -> Call<ListProviders> {
        Call::tenant(self.client, TenantOp::ListProviders, ListProviders)
    }

    /// Registers a provider with its API key. The key is never returned.
    pub fn create(
        &self,
        name: impl Into<String>,
        provider_type: ProviderType,
        api_key: impl Into<String>,
    ) -> Call<CreateProvider> {
        Call::tenant(
            self.client,
            TenantOp::CreateProvider,
            CreateProvider {
                name: name.into(),
                provider_type,
                api_key: api_key.into(),
                base_url: None,
            },
        )
    }

    /// Gets one provider.
    pub fn get(&self, provider_id: impl Into<ProviderId>) -> Call<GetProvider> {
        let provider_id = provider_id.into();
        Call::tenant(
            self.client,
            TenantOp::GetProvider {
                id: provider_id.as_str(),
            },
            GetProvider,
        )
    }

    /// Renames a provider.
    pub fn update(
        &self,
        provider_id: impl Into<ProviderId>,
        name: impl Into<String>,
    ) -> Call<UpdateProvider> {
        let provider_id = provider_id.into();
        Call::tenant(
            self.client,
            TenantOp::UpdateProvider {
                id: provider_id.as_str(),
            },
            UpdateProvider { name: name.into() },
        )
    }

    /// Deletes a provider.
    pub fn delete(&self, provider_id: impl Into<ProviderId>) -> Call<DeleteProvider> {
        let provider_id = provider_id.into();
        Call::tenant(
            self.client,
            TenantOp::DeleteProvider {
                id: provider_id.as_str(),
            },
            DeleteProvider::default(),
        )
    }

    /// Lists the models a provider offers.
    pub fn list_models(&self, provider_id: impl Into<ProviderId>) -> Call<ListProviderModels> {
        let provider_id = provider_id.into();
        Call::tenant(
            self.client,
            TenantOp::ListProviderModels {
                id: provider_id.as_str(),
            },
            ListProviderModels,
        )
    }

    /// Lists the thinking levels of one model.
    pub fn get_thinking_levels(
        &self,
        provider_id: impl Into<ProviderId>,
        model: impl Into<String>,
    ) -> Call<ListThinkingLevels> {
        let provider_id = provider_id.into();
        Call::tenant(
            self.client,
            TenantOp::ListThinkingLevels {
                id: provider_id.as_str(),
            },
            ListThinkingLevels {
                model: model.into(),
            },
        )
    }
}

crate::open_enum! {
    /// The model vendor behind a provider.
    pub enum ProviderType {
        /// `openai`
        OpenAi = "openai",
        /// `anthropic`
        Anthropic = "anthropic",
        /// `openrouter`
        OpenRouter = "openrouter",
        /// `google`
        Google = "google",
        /// `vercel_ai_gateway`
        VercelAiGateway = "vercel_ai_gateway",
        /// `custom`
        Custom = "custom",
    }
}

/// Request state of [`Providers::list`].
#[derive(Clone, Debug)]
pub struct ListProviders;
impl_request!(ListProviders => ProviderList; encode = none; decode = json);

/// Request state of [`Providers::create`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProvider {
    name: String,
    provider_type: ProviderType,
    api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
}
impl_request!(CreateProvider => Provider; encode = json; decode = json;
    sensitive = (|request: &Self| vec![request.api_key.clone()]));

impl Call<CreateProvider> {
    /// The provider's endpoint override. Required for `custom`, rejected for
    /// `vercel_ai_gateway`, and fixed after creation.
    pub fn base_url(self, base_url: impl Into<String>) -> Self {
        self.map(|request| request.base_url = Some(base_url.into()))
    }
}

/// Request state of [`Providers::get`].
#[derive(Clone, Debug)]
pub struct GetProvider;
impl_request!(GetProvider => Provider; encode = none; decode = json);

/// Request state of [`Providers::update`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProvider {
    name: String,
}
impl_request!(UpdateProvider => Provider; encode = json; decode = json);

/// Request state of [`Providers::delete`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    confirm_snapshot_invalidation: Option<bool>,
}
impl_request!(DeleteProvider => (); encode = query; decode = none);

impl Call<DeleteProvider> {
    /// Confirms that sessions and queued or running task runs with a saved
    /// reference to this provider may stop working. It never overrides use by
    /// a current agent.
    pub fn confirm_snapshot_invalidation(self, confirm: bool) -> Self {
        self.map(|request| request.confirm_snapshot_invalidation = Some(confirm))
    }
}

/// Request state of [`Providers::list_models`].
#[derive(Clone, Debug)]
pub struct ListProviderModels;
impl_request!(ListProviderModels => ProviderModelList; encode = none; decode = json);

/// Request state of [`Providers::get_thinking_levels`].
#[derive(Clone, Debug, Serialize)]
pub struct ListThinkingLevels {
    model: String,
}
impl_request!(ListThinkingLevels => ProviderThinkingLevels; encode = query; decode = json);

/// A provider. Its API key is never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    /// The provider's ID.
    pub id: ProviderId,
    /// The display name.
    pub name: String,
    /// The model vendor.
    pub provider_type: ProviderType,
    /// The endpoint override, if any.
    pub base_url: Option<String>,
    /// The last characters of the stored API key.
    pub key_fragment: String,
    /// When the provider was registered.
    pub created_at: Timestamp,
    /// When the provider last changed.
    pub updated_at: Timestamp,
}

/// The tenant's providers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderList {
    /// Every provider, without endpoint or key details.
    pub providers: Vec<ProviderSummary>,
}

/// A provider in a [`ProviderList`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    /// The provider's ID.
    pub id: ProviderId,
    /// The display name.
    pub name: String,
    /// The model vendor.
    pub provider_type: ProviderType,
    /// When the provider was registered.
    pub created_at: Timestamp,
    /// When the provider last changed.
    pub updated_at: Timestamp,
}

/// The models a provider offers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelList {
    /// Every advertised model.
    pub models: Vec<ProviderModel>,
}

/// A model a provider offers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    /// The provider's own model ID.
    pub id: String,
}

/// The thinking levels of one model.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderThinkingLevels {
    /// Whether the provider knows this model's thinking levels.
    pub known: bool,
    /// The supported levels.
    pub levels: Vec<String>,
}
