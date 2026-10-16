//! Model providers. Tenant only; start from [`Client::providers`].

use serde::{Deserialize, Serialize};

use crate::call::{Call, impl_request};
use crate::client::{Client, Tenant};
use crate::ids::ProviderId;
use crate::op::Op;

/// Provider operations. Returned by [`Client::providers`].
#[derive(Clone, Copy, Debug)]
pub struct Providers<'a> {
    client: &'a Client<Tenant>,
}

impl Client<Tenant> {
    /// Provider operations.
    pub fn providers(&self) -> Providers<'_> {
        Providers { client: self }
    }
}

impl Providers<'_> {
    /// Lists providers.
    pub fn list(&self) -> Call<ListProviders> {
        Call::new(self.client, Op::ListProviders, ListProviders)
    }

    /// Registers a provider with its API key. The key is never returned.
    pub fn create(
        &self,
        name: impl Into<String>,
        provider_type: ProviderType,
        api_key: impl Into<String>,
    ) -> Call<CreateProvider> {
        Call::new(
            self.client,
            Op::CreateProvider,
            CreateProvider {
                name: name.into(),
                provider_type,
                api_key: api_key.into(),
            },
        )
    }

    /// Gets one provider.
    pub fn get(&self, provider_id: impl Into<ProviderId>) -> Call<GetProvider> {
        let provider_id = provider_id.into();
        Call::new(
            self.client,
            Op::GetProvider {
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
        Call::new(
            self.client,
            Op::UpdateProvider {
                id: provider_id.as_str(),
            },
            UpdateProvider { name: name.into() },
        )
    }

    /// Deletes a provider.
    pub fn delete(&self, provider_id: impl Into<ProviderId>) -> Call<DeleteProvider> {
        let provider_id = provider_id.into();
        Call::new(
            self.client,
            Op::DeleteProvider {
                id: provider_id.as_str(),
            },
            DeleteProvider::default(),
        )
    }

    /// Lists the models a provider offers.
    pub fn list_models(&self, provider_id: impl Into<ProviderId>) -> Call<ListProviderModels> {
        let provider_id = provider_id.into();
        Call::new(
            self.client,
            Op::ListProviderModels {
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
        Call::new(
            self.client,
            Op::ListThinkingLevels {
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
impl_request!(ListProviders => ProviderList; encode = none; decode = credential_safe_json);

/// Request state of [`Providers::create`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProvider {
    name: String,
    provider_type: ProviderType,
    api_key: String,
}
impl_request!(CreateProvider => Provider; encode = json; decode = credential_safe_json;
    sensitive = (|request: &Self| vec![request.api_key.clone()]));

/// Request state of [`Providers::get`].
#[derive(Clone, Debug)]
pub struct GetProvider;
impl_request!(GetProvider => Provider; encode = none; decode = credential_safe_json);

/// Request state of [`Providers::update`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProvider {
    name: String,
}
impl_request!(UpdateProvider => Provider; encode = json; decode = credential_safe_json);

/// Request state of [`Providers::delete`].
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProvider {}
impl_request!(DeleteProvider => (); encode = query; decode = none);

/// Request state of [`Providers::list_models`].
#[derive(Clone, Debug)]
pub struct ListProviderModels;
impl_request!(ListProviderModels => ProviderModelList; encode = none; decode = credential_safe_json);

/// Request state of [`Providers::get_thinking_levels`].
#[derive(Clone, Debug, Serialize)]
pub struct ListThinkingLevels {
    model: String,
}
impl_request!(ListThinkingLevels => ProviderThinkingLevels; encode = query; decode = credential_safe_json);

/// A provider. Its API key is never returned.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Provider {}

/// The tenant's providers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderList {}

/// The models a provider offers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelList {}

/// The thinking levels of one model.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ProviderThinkingLevels {}
