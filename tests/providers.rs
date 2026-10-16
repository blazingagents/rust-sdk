mod support;

use blazing_agents::Error;
use blazing_agents::ids::ProviderId;
use blazing_agents::providers::ProviderType;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn provider(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": "Production OpenRouter",
        "providerType": "openrouter",
        "baseUrl": null,
        "keyFragment": "9f2c",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_the_body_and_decodes_the_provider() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(json(201, provider("prv_7Tn4Kd9QwE2sLx5R")))
        .mount(&server)
        .await;

    let created = client
        .providers()
        .create(
            "Production OpenRouter",
            ProviderType::OpenRouter,
            "sk-or-v1-3b7e9f2c",
        )
        .await
        .unwrap();
    client
        .providers()
        .create("Custom", ProviderType::Custom, "key-9f2c")
        .base_url("https://models.example.com/v1")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "name": "Production OpenRouter",
            "providerType": "openrouter",
            "apiKey": "sk-or-v1-3b7e9f2c"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "name": "Custom",
            "providerType": "custom",
            "apiKey": "key-9f2c",
            "baseUrl": "https://models.example.com/v1"
        })
    );
    assert_eq!(created.id, ProviderId::from("prv_7Tn4Kd9QwE2sLx5R"));
    assert_eq!(created.provider_type, ProviderType::OpenRouter);
    assert_eq!(created.base_url, None);
    assert_eq!(created.key_fragment, "9f2c");
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn list_decodes_summaries_and_ignores_unknown_fields() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers"))
        .respond_with(json(
            200,
            json!({
                "providers": [
                    {
                        "id": "prv_7Tn4Kd9QwE2sLx5R",
                        "name": "Production OpenRouter",
                        "providerType": "openrouter",
                        "createdAt": "2026-07-10T10:00:00.000Z",
                        "updatedAt": "2026-07-10T10:00:00.000Z",
                        "addedNextYear": true
                    }
                ],
                "addedNextYear": { "nested": true }
            }),
        ))
        .mount(&server)
        .await;

    let list = client.providers().list().await.unwrap();

    assert_eq!(list.providers.len(), 1);
    assert_eq!(
        list.providers[0].id,
        ProviderId::from("prv_7Tn4Kd9QwE2sLx5R")
    );
    assert_eq!(list.providers[0].provider_type, ProviderType::OpenRouter);
}

#[tokio::test]
async fn get_decodes_an_unknown_provider_type_as_unrecognized() {
    let (server, client) = mock().await;
    let mut body = provider("prv_7Tn4Kd9QwE2sLx5R");
    body["providerType"] = json!("vendor_added_next_year");
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client
        .providers()
        .get("prv_7Tn4Kd9QwE2sLx5R")
        .await
        .unwrap();

    assert_eq!(fetched.provider_type.as_str(), "vendor_added_next_year");
    match fetched.provider_type {
        ProviderType::Unrecognized(other) => {
            assert_eq!(other.as_str(), "vendor_added_next_year")
        }
        other => panic!("expected an unrecognized provider type, got {other:?}"),
    }
}

#[tokio::test]
async fn update_sends_the_new_name() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R"))
        .respond_with(json(200, provider("prv_7Tn4Kd9QwE2sLx5R")))
        .mount(&server)
        .await;

    client
        .providers()
        .update("prv_7Tn4Kd9QwE2sLx5R", "Primary OpenRouter")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Primary OpenRouter" }));
}

#[tokio::test]
async fn delete_sends_the_confirmation_query_parameter() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client
        .providers()
        .delete("prv_7Tn4Kd9QwE2sLx5R")
        .await
        .unwrap();
    client
        .providers()
        .delete("prv_7Tn4Kd9QwE2sLx5R")
        .confirm_snapshot_invalidation(true)
        .await
        .unwrap();
    client
        .providers()
        .delete("prv_7Tn4Kd9QwE2sLx5R")
        .confirm_snapshot_invalidation(false)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), None);
    assert_eq!(
        sent[1].url.query(),
        Some("confirmSnapshotInvalidation=true")
    );
    assert_eq!(
        sent[2].url.query(),
        Some("confirmSnapshotInvalidation=false")
    );
}

#[tokio::test]
async fn list_models_decodes_the_model_ids() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R/models"))
        .respond_with(json(
            200,
            json!({ "models": [{ "id": "openai/gpt-6-luna" }, { "id": "openai/gpt-6-nova" }] }),
        ))
        .mount(&server)
        .await;

    let models = client
        .providers()
        .list_models("prv_7Tn4Kd9QwE2sLx5R")
        .await
        .unwrap();

    assert_eq!(
        models
            .models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["openai/gpt-6-luna", "openai/gpt-6-nova"]
    );
}

#[tokio::test]
async fn get_thinking_levels_sends_the_model_query() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R/thinking-levels"))
        .and(query_param("model", "openai/gpt-6-luna"))
        .respond_with(json(
            200,
            json!({ "known": true, "levels": ["off", "low", "medium", "high"] }),
        ))
        .mount(&server)
        .await;

    let levels = client
        .providers()
        .get_thinking_levels("prv_7Tn4Kd9QwE2sLx5R", "openai/gpt-6-luna")
        .await
        .unwrap();

    assert!(levels.known);
    assert_eq!(levels.levels, ["off", "low", "medium", "high"]);
}

#[tokio::test]
async fn an_error_echoing_the_api_key_is_redacted() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(api_error(
            400,
            "invalid_request",
            "The API key sk-or-v1-3b7e9f2c is not valid.",
        ))
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("Broken", ProviderType::OpenRouter, "sk-or-v1-3b7e9f2c")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert!(!api.body().contains("sk-or-v1-3b7e9f2c"));
    assert!(api.body().contains("[REDACTED]"));
    assert!(!api.message().contains("sk-or-v1-3b7e9f2c"));
}

#[tokio::test]
async fn a_response_echoing_an_api_key_is_rejected() {
    let (server, client) = mock().await;
    let mut body = provider("prv_7Tn4Kd9QwE2sLx5R");
    body["apiKey"] = json!("sk-or-v1-3b7e9f2c");
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_7Tn4Kd9QwE2sLx5R"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client
        .providers()
        .get("prv_7Tn4Kd9QwE2sLx5R")
        .await
        .unwrap_err();

    let Error::CredentialEcho { operation_id, .. } = error else {
        panic!("expected a credential echo error, got {error:?}");
    };
    assert_eq!(operation_id, "getProvider");
}

#[tokio::test]
async fn the_call_debug_does_not_leak_the_request() {
    let (_server, client) = mock().await;

    // `Call`'s Debug prints the route only: request bodies can hold secrets.
    let call = client
        .providers()
        .create(
            "Production OpenRouter",
            ProviderType::OpenRouter,
            "sk-or-v1-3b7e9f2c",
        )
        .base_url("https://models.example.com/v1");
    let debug = format!("{call:?}");

    assert!(!debug.contains("sk-or-v1-3b7e9f2c"));
    assert!(!debug.contains("api_key"));
    assert!(!debug.contains("apiKey"));
    assert!(!debug.contains("Production OpenRouter"));
    assert!(debug.contains("createProvider"));
}
