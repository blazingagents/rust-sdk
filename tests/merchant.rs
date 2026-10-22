mod support;

use blazing_agents::Error;
use blazing_agents::ids::{MerchantConnectionId, MerchantUsageEventId};
use blazing_agents::merchant::{
    MerchantEnvironment, MerchantGuard, MerchantNextAction, MerchantProvider, MerchantTurnStatus,
    MerchantUsageEventStatus, MerchantWorkflowIssue,
};
use blazing_agents::providers::ProviderType;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn merchant_connection() -> serde_json::Value {
    json!({
        "id": "mch_2Kd8QwE5sLx7Rm9T",
        "provider": "polar",
        "environment": "sandbox",
        "merchantAccountId": "acct_123",
        "keyFragment": "x9Tz",
        "guard": { "enabled": true, "productIds": ["prod_1"], "meterId": null },
        "configVersion": 4,
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

fn binding(user_id: &str) -> serde_json::Value {
    json!({
        "userId": user_id,
        "customerId": "cus_456",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

fn event(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "turnId": "turn_7Tn4Kd9QwE2sLx5R",
        "connectionId": "mch_2Kd8QwE5sLx7Rm9T",
        "provider": "polar",
        "userId": "user_1",
        "customerId": "cus_456",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "sessionId": "ss_9Pm2Vb6HxR4kTq8W",
        "model": "openai/gpt-6-luna",
        "modelProvider": "openai",
        "inputTokens": 100,
        "outputTokens": 40,
        "totalTokens": 140,
        "turnStatus": "succeeded",
        "startedAt": "2026-07-10T10:00:00.000Z",
        "occurredAt": "2026-07-10T10:00:30.000Z",
        "status": "failed",
        "payload": { "providerRef": "evt_1" },
        "attemptCount": 2,
        "attemptGeneration": 1,
        "lastAttemptAt": "2026-07-10T10:05:00.000Z",
        "lastErrorCode": "provider_5xx",
        "acceptedAt": null,
        "workflowIssue": "error",
        "nextAction": "retry"
    })
}

fn assert_event(event: &blazing_agents::merchant::MerchantUsageEvent, id: &str) {
    assert_eq!(event.id, MerchantUsageEventId::from(id));
    assert_eq!(event.turn_id.as_str(), "turn_7Tn4Kd9QwE2sLx5R");
    assert_eq!(
        event.connection_id,
        MerchantConnectionId::from("mch_2Kd8QwE5sLx7Rm9T")
    );
    assert_eq!(event.provider, MerchantProvider::Polar);
    assert_eq!(event.customer_id.as_deref(), Some("cus_456"));
    assert_eq!(event.model_provider, ProviderType::OpenAi);
    assert_eq!(event.total_tokens, 140);
    assert_eq!(event.turn_status, MerchantTurnStatus::Succeeded);
    assert_eq!(event.status, MerchantUsageEventStatus::Failed);
    assert_eq!(
        event
            .payload
            .as_ref()
            .map(|payload| payload["providerRef"].clone()),
        Some(json!("evt_1"))
    );
    assert_eq!(event.attempt_count, 2);
    assert_eq!(event.workflow_issue, Some(MerchantWorkflowIssue::Error));
    assert_eq!(event.next_action, MerchantNextAction::Retry);
}

#[tokio::test]
async fn get_returns_the_connection() {
    let (server, client) = mock().await;
    let mut connection = merchant_connection();
    connection["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(
            200,
            json!({ "connection": connection, "envelopeAddedNextYear": true }),
        ))
        .mount(&server)
        .await;

    let connection = client.merchant_connection().get().await.unwrap();

    let connection = connection.expect("a connection is present");
    assert_eq!(
        connection.id,
        MerchantConnectionId::from("mch_2Kd8QwE5sLx7Rm9T")
    );
    assert_eq!(connection.provider, MerchantProvider::Polar);
    assert_eq!(connection.environment, MerchantEnvironment::Sandbox);
    assert_eq!(connection.merchant_account_id, "acct_123");
    assert_eq!(connection.key_fragment, "x9Tz");
    assert!(connection.guard.enabled);
    assert_eq!(connection.guard.product_ids, ["prod_1"]);
    assert_eq!(connection.guard.meter_id, None);
    assert_eq!(connection.config_version, 4);
    assert_eq!(connection.created_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn get_returns_none_when_there_is_no_connection() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(200, json!({ "connection": null })))
        .mount(&server)
        .await;

    let connection = client.merchant_connection().get().await.unwrap();

    assert!(connection.is_none());
}

#[tokio::test]
async fn create_sends_only_the_set_fields() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(201, json!({ "connection": merchant_connection() })))
        .mount(&server)
        .await;

    let created = client
        .merchant_connection()
        .create(
            MerchantProvider::Polar,
            MerchantEnvironment::Sandbox,
            "polar_at_secret_value",
        )
        .await
        .unwrap();
    client
        .merchant_connection()
        .create(
            MerchantProvider::Dodo,
            MerchantEnvironment::Live,
            "dodo_key_secret_value",
        )
        .guard(
            MerchantGuard::new(true)
                .product_ids(["prod_1", "prod_2"])
                .meter_id("meter_9"),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "provider": "polar",
            "environment": "sandbox",
            "credential": "polar_at_secret_value"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "provider": "dodo",
            "environment": "live",
            "credential": "dodo_key_secret_value",
            "guard": {
                "enabled": true,
                "productIds": ["prod_1", "prod_2"],
                "meterId": "meter_9"
            }
        })
    );
    assert_eq!(created.config_version, 4);
}

#[tokio::test]
async fn a_new_guard_serializes_a_null_meter_id() {
    let guard = MerchantGuard::new(false);
    assert_eq!(
        serde_json::to_value(&guard).unwrap(),
        json!({ "enabled": false, "productIds": [], "meterId": null })
    );
}

#[tokio::test]
async fn update_sends_only_the_set_fields() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(200, json!({ "connection": merchant_connection() })))
        .mount(&server)
        .await;

    client
        .merchant_connection()
        .update()
        .guard(MerchantGuard::new(false))
        .await
        .unwrap();
    client
        .merchant_connection()
        .update()
        .credential("polar_at_rotated")
        .guard(MerchantGuard::new(true).product_ids(["prod_3"]))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "guard": { "enabled": false, "productIds": [], "meterId": null } })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "credential": "polar_at_rotated",
            "guard": { "enabled": true, "productIds": ["prod_3"], "meterId": null }
        })
    );
}

#[tokio::test]
async fn retire_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/merchant-connection"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.merchant_connection().retire().await.unwrap();
}

#[tokio::test]
async fn bindings_list_sends_the_filters() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection/bindings"))
        .and(query_param("userId", "user_1"))
        .and(query_param("limit", "100"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "bindings": [binding("user_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .merchant_bindings()
        .list()
        .user_id("user_1")
        .limit(100)
        .await
        .unwrap();

    assert_eq!(page.bindings.len(), 1);
    assert_eq!(page.bindings[0].user_id, "user_1");
    assert_eq!(page.bindings[0].customer_id, "cus_456");
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );
}

#[tokio::test]
async fn binding_items_walk_every_page() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection/bindings"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "bindings": [binding("user_1"), binding("user_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection/bindings"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "bindings": [binding("user_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let ids: Vec<String> = client
        .merchant_bindings()
        .list()
        .items()
        .map_ok(|binding| binding.user_id)
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["user_1", "user_2", "user_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn set_binding_puts_the_customer() {
    let (server, client) = mock().await;
    let mut binding = binding("user_1");
    binding["addedNextYear"] = json!(true);
    Mock::given(method("PUT"))
        .and(path("/v1/merchant-connection/bindings/user_1"))
        .respond_with(json(
            200,
            json!({ "binding": binding, "envelopeAddedNextYear": "yes" }),
        ))
        .mount(&server)
        .await;

    let bound = client
        .merchant_bindings()
        .put("user_1", "cus_456")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "customerId": "cus_456" }));
    assert_eq!(bound.user_id, "user_1");
    assert_eq!(bound.customer_id, "cus_456");
}

#[tokio::test]
async fn binding_paths_encode_the_user_id_as_one_segment() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/merchant-connection/bindings/user%2F1"))
        .respond_with(json(200, json!({ "binding": binding("user/1") })))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/merchant-connection/bindings/user%2F1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let bound = client
        .merchant_bindings()
        .put("user/1", "cus_456")
        .await
        .unwrap();
    client.merchant_bindings().delete("user/1").await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        sent[0].url.path(),
        "/v1/merchant-connection/bindings/user%2F1"
    );
    assert_eq!(
        sent[1].url.path(),
        "/v1/merchant-connection/bindings/user%2F1"
    );
    assert_eq!(bound.user_id, "user/1");
}

#[tokio::test]
async fn delete_binding_sends_delete() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/merchant-connection/bindings/user_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.merchant_bindings().delete("user_1").await.unwrap();
}

#[tokio::test]
async fn usage_events_list_sends_the_filters_and_decodes() {
    let (server, client) = mock().await;
    let mut body = event("mev_1");
    body["nextAction"] = json!("action_added_next_year");
    body["addedNextYear"] = json!(true);
    Mock::given(method("GET"))
        .and(path("/v1/merchant-usage-events"))
        .and(query_param("status", "failed"))
        .and(query_param("limit", "10"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(200, json!({ "events": [body], "nextCursor": null })))
        .mount(&server)
        .await;

    let page = client
        .merchant_usage_events()
        .list()
        .status(MerchantUsageEventStatus::Failed)
        .limit(10)
        .await
        .unwrap();

    assert_eq!(page.events.len(), 1);
    let event = &page.events[0];
    assert!(matches!(
        event.next_action,
        MerchantNextAction::Unrecognized(_)
    ));
    assert_eq!(event.next_action.as_str(), "action_added_next_year");
    assert_eq!(event.status, MerchantUsageEventStatus::Failed);
}

#[tokio::test]
async fn usage_event_items_walk_every_page() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-usage-events"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "events": [event("mev_1"), event("mev_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-usage-events"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "events": [event("mev_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let ids: Vec<String> = client
        .merchant_usage_events()
        .list()
        .items()
        .map_ok(|event| event.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["mev_1", "mev_2", "mev_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn get_event_unwraps_the_envelope() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-usage-events/mev_1"))
        .respond_with(json(200, json!({ "event": event("mev_1") })))
        .mount(&server)
        .await;

    let event = client.merchant_usage_events().get("mev_1").await.unwrap();

    assert_event(&event, "mev_1");
}

#[tokio::test]
async fn discard_and_retry_post_to_their_paths() {
    let (server, client) = mock().await;
    for action in ["discard", "retry"] {
        Mock::given(method("POST"))
            .and(path(format!("/v1/merchant-usage-events/mev_1/{action}")))
            .respond_with(json(200, json!({ "event": event("mev_1") })))
            .expect(1)
            .mount(&server)
            .await;
    }

    let discarded = client
        .merchant_usage_events()
        .discard("mev_1")
        .await
        .unwrap();
    let retried = client.merchant_usage_events().retry("mev_1").await.unwrap();

    assert_event(&discarded, "mev_1");
    assert_event(&retried, "mev_1");
}

#[tokio::test]
async fn summary_sends_days_and_decodes() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/merchant-usage-events/summary"))
        .and(query_param("days", "30"))
        .respond_with(json(
            200,
            json!({
                "envelopeAddedNextYear": [1],
                "summary": {
                    "addedNextYear": { "nested": true },
                    "counts": {
                        "pending": 3,
                        "accepted": 90,
                        "failed": 4,
                        "incomplete": 0,
                        "expired": 0,
                        "discarded": 1
                    },
                    "lastAcceptedAt": "2026-07-10T10:05:00.000Z",
                    "oldestPendingOccurredAt": "2026-07-09T10:00:00.000Z",
                    "daily": [
                        {
                            "day": "2026-07-10",
                            "acceptedEvents": 12,
                            "acceptedTokens": 3400,
                            "pendingEvents": 2
                        }
                    ]
                }
            }),
        ))
        .mount(&server)
        .await;

    let summary = client
        .merchant_usage_events()
        .summary()
        .days(30)
        .await
        .unwrap();

    assert_eq!(summary.counts.accepted, 90);
    assert_eq!(summary.counts.failed, 4);
    assert_eq!(
        summary.last_accepted_at.unwrap().to_string(),
        "2026-07-10T10:05:00Z"
    );
    assert_eq!(summary.daily.len(), 1);
    assert_eq!(summary.daily[0].day, "2026-07-10");
    assert_eq!(summary.daily[0].accepted_tokens, 3400);
}

#[tokio::test]
async fn created_credential_is_redacted_from_errors() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/merchant-connection"))
        .respond_with(api_error(
            400,
            "validation_failed",
            "Credential polar_at_leaked rejected.",
        ))
        .mount(&server)
        .await;

    let error = client
        .merchant_connection()
        .create(
            MerchantProvider::Polar,
            MerchantEnvironment::Sandbox,
            "polar_at_leaked",
        )
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Credential [REDACTED] rejected.");
    assert!(!api.body().contains("polar_at_leaked"), "{}", api.body());
}

#[tokio::test]
async fn updated_credential_is_redacted_from_errors() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/merchant-connection"))
        .respond_with(api_error(
            400,
            "validation_failed",
            "Credential polar_at_rotated rejected.",
        ))
        .mount(&server)
        .await;

    let error = client
        .merchant_connection()
        .update()
        .credential("polar_at_rotated")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Credential [REDACTED] rejected.");
    assert!(!api.body().contains("polar_at_rotated"), "{}", api.body());
}

#[tokio::test]
async fn calls_do_not_leak_credentials_in_debug() {
    let (_server, client) = mock().await;

    // The request types are not constructible outside the crate, and a Call's
    // Debug shows only the route, never the request body.
    let create = client.merchant_connection().create(
        MerchantProvider::Polar,
        MerchantEnvironment::Sandbox,
        "polar_at_secret",
    );
    assert!(!format!("{create:?}").contains("polar_at_secret"));

    let update = client
        .merchant_connection()
        .update()
        .credential("polar_at_rotated_secret");
    assert!(!format!("{update:?}").contains("polar_at_rotated_secret"));
}

#[tokio::test]
async fn a_create_response_that_echoes_the_credential_fails_the_call() {
    let (server, client) = mock().await;
    let mut connection = merchant_connection();
    connection["merchantAccountId"] = json!("polar_at_secret_value");
    Mock::given(method("POST"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(201, json!({ "connection": connection })))
        .mount(&server)
        .await;

    let error = client
        .merchant_connection()
        .create(
            MerchantProvider::Polar,
            MerchantEnvironment::Sandbox,
            "polar_at_secret_value",
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "createMerchantConnection",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_update_response_that_echoes_the_credential_fails_the_call() {
    let (server, client) = mock().await;
    let mut connection = merchant_connection();
    connection["merchantAccountId"] = json!("polar_at_rotated_secret");
    Mock::given(method("PATCH"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(200, json!({ "connection": connection })))
        .mount(&server)
        .await;

    let error = client
        .merchant_connection()
        .update()
        .credential("polar_at_rotated_secret")
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "updateMerchantConnection",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_get_response_with_a_credential_field_fails_the_call() {
    let (server, client) = mock().await;
    let mut connection = merchant_connection();
    connection["apiKey"] = json!("sk_live_x");
    Mock::given(method("GET"))
        .and(path("/v1/merchant-connection"))
        .respond_with(json(200, json!({ "connection": connection })))
        .mount(&server)
        .await;

    let error = client.merchant_connection().get().await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "getMerchantConnection",
                ..
            }
        ),
        "{error:?}"
    );
}
