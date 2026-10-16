mod support;

use std::time::Duration;

use blazing_agents::Error;
use blazing_agents::error::ErrorCode;
use blazing_agents::ids::{AgentId, PromptId};
use blazing_agents::types::Metadata;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn prompt(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "Refund reply",
        "template": "Reply to {{customerName}} about {{orderId}}.",
        "variables": ["customerName", "orderId"],
        "userId": "",
        "metadata": { "team": "support" },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_the_body_and_decodes_the_prompt() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/prompts"))
        .respond_with(json(201, prompt("prompt_5Wn3Hc7TbK2xQv9F")))
        .mount(&server)
        .await;

    let created = client
        .prompts()
        .create(
            "Refund reply",
            "Reply to {{customerName}} about {{orderId}}.",
        )
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .metadata(Metadata::new().insert("team", "support"))
        .await
        .unwrap();
    client
        .prompts()
        .create("Refund reply", "Hi {{name}}")
        .user_id("customer_123")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "name": "Refund reply",
            "template": "Reply to {{customerName}} about {{orderId}}.",
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "metadata": { "team": "support" }
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "name": "Refund reply",
            "template": "Hi {{name}}",
            "userId": "customer_123"
        })
    );
    assert_eq!(created.id, PromptId::from("prompt_5Wn3Hc7TbK2xQv9F"));
    assert_eq!(created.agent_id, Some(AgentId::from("ag_4kP9sT2vXq7LmN3a")));
    assert_eq!(created.variables, ["customerName", "orderId"]);
    assert_eq!(created.user_id, "");
    assert_eq!(created.metadata.get("team"), Some(&json!("support")));
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert!(created.updated_at > created.created_at);
}

#[tokio::test]
async fn list_sends_filters_as_query_parameters() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param("userId", "customer_123"))
        .and(query_param("agentId", "ag_4kP9sT2vXq7LmN3a"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .prompts()
        .list()
        .user_id("customer_123")
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );
}

#[tokio::test]
async fn get_ignores_unknown_response_fields() {
    let (server, client) = mock().await;
    let mut body = prompt("prompt_1");
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.prompts().get("prompt_1").await.unwrap();

    assert_eq!(fetched.name, "Refund reply");
}

#[tokio::test]
async fn update_omits_unset_fields_and_sends_null_to_clear() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(200, prompt("prompt_1")))
        .mount(&server)
        .await;

    client
        .prompts()
        .update("prompt_1")
        .name("Renamed")
        .await
        .unwrap();
    client
        .prompts()
        .update("prompt_1")
        .clear_agent_id()
        .await
        .unwrap();
    client
        .prompts()
        .update("prompt_1")
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .template("New {{x}}")
        .await
        .unwrap();
    client
        .prompts()
        .update("prompt_1")
        .metadata(Metadata::new().insert("team", "billing"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Renamed" }));
    assert_eq!(json_body(&sent[1]), json!({ "agentId": null }));
    assert_eq!(
        json_body(&sent[2]),
        json!({ "agentId": "ag_4kP9sT2vXq7LmN3a", "template": "New {{x}}" })
    );
    assert_eq!(
        json_body(&sent[3]),
        json!({ "metadata": { "team": "billing" } })
    );
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();
}

#[tokio::test]
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_1"), prompt("prompt_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.prompts().list().limit(2).items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|prompt| prompt.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["prompt_1", "prompt_2", "prompt_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[tokio::test]
async fn api_errors_carry_the_envelope() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_missing"))
        .respond_with(
            api_error(404, "not_found", "Prompt not found.")
                .insert_header("x-request-id", "req-42")
                .insert_header("retry-after", "2"),
        )
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_missing").await.unwrap_err();

    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), Some(&ErrorCode::NotFound));
    assert_eq!(error.request_id(), Some("req-42"));
    assert_eq!(error.to_string(), "[not_found] Prompt not found.");
    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Prompt not found.");
    assert_eq!(api.retry_after(), Some(Duration::from_secs(2)));
    assert_eq!(api.param(), None);
}

#[tokio::test]
async fn unknown_error_codes_decode_as_unrecognized() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(api_error(422, "code_added_next_year", "New failure."))
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    match error.code() {
        Some(ErrorCode::Unrecognized(code)) => assert_eq!(code.as_str(), "code_added_next_year"),
        other => panic!("expected an unrecognized code, got {other:?}"),
    }
    assert_eq!(
        error.code().map(ErrorCode::as_str),
        Some("code_added_next_year")
    );
}

#[tokio::test]
async fn user_scope_sends_the_user_header() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(200, prompt("prompt_1")))
        .mount(&server)
        .await;

    client.prompts().get("prompt_1").await.unwrap();
    let user_client = client.for_user("customer_123").unwrap();
    user_client.prompts().get("prompt_1").await.unwrap();

    let sent = requests(&server).await;
    assert!(!sent[0].headers.contains_key("x-ba-user-id"));
    assert_eq!(
        sent[1]
            .headers
            .get("x-ba-user-id")
            .map(|v| v.to_str().unwrap()),
        Some("customer_123")
    );
    assert_eq!(
        sent[1]
            .headers
            .get("authorization")
            .map(|v| v.to_str().unwrap()),
        Some("Bearer test-key")
    );
}
