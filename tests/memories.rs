mod support;

use blazing_agents::error::ErrorCode;
use blazing_agents::pagination::Cursor;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

const AGENT: &str = "ag_4kP9sT2vXq7LmN3a";
const MEMORY: &str = "mem_9Zx4Cv7BnM2LkPqR";

fn memory(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": AGENT,
        "userId": "customer_123",
        "text": "Prefers release notes under five lines.",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "lastAccessedAt": "2026-07-10T10:15:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_the_body_and_unwraps_the_envelope() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/memories")))
        .respond_with(json(201, json!({ "memory": memory(MEMORY) })))
        .mount(&server)
        .await;

    let created = client
        .memories()
        .create(AGENT, "Prefers release notes under five lines.")
        .user_id("customer_123")
        .await
        .unwrap();
    client
        .memories()
        .create(AGENT, "Works in UTC.")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "text": "Prefers release notes under five lines.",
            "userId": "customer_123"
        })
    );
    // Omitted means absent, not null.
    assert_eq!(json_body(&sent[1]), json!({ "text": "Works in UTC." }));
    assert_eq!(created.id.as_str(), MEMORY);
    assert_eq!(created.tenant_id.as_str(), "ten_8Hq2Zr5WcY1bJt6D");
    assert_eq!(created.agent_id.as_str(), AGENT);
    assert_eq!(created.user_id, "customer_123");
    assert_eq!(created.text, "Prefers release notes under five lines.");
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(created.last_accessed_at.to_string(), "2026-07-10T10:15:00Z");
}

#[tokio::test]
async fn list_sends_filters_and_decodes_items() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories")))
        .and(query_param("userId", "customer_123"))
        .and(query_param("search", "release"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [memory("mem_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .memories()
        .list(AGENT)
        .user_id("customer_123")
        .search("release")
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].id.as_str(), "mem_1");
    assert_eq!(page.next_cursor.as_ref().map(Cursor::as_str), Some("c2"));

    // An empty user_id is sent, not omitted.
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories")))
        .and(query_param("userId", ""))
        .respond_with(json(200, json!({ "data": [], "nextCursor": null })))
        .mount(&server)
        .await;
    client.memories().list(AGENT).user_id("").await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[1].url.query(), Some("userId="));
}

#[tokio::test]
async fn get_unwraps_the_envelope_and_ignores_unknown_fields() {
    let (server, client) = mock().await;
    let mut body = memory(MEMORY);
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories/{MEMORY}")))
        .respond_with(json(200, json!({ "memory": body })))
        .mount(&server)
        .await;

    let fetched = client.memories().get(AGENT, MEMORY).await.unwrap();

    assert_eq!(fetched.id.as_str(), MEMORY);
    assert_eq!(fetched.updated_at.to_string(), "2026-07-10T10:15:00Z");
}

#[tokio::test]
async fn update_patches_the_text() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/agents/{AGENT}/memories/{MEMORY}")))
        .respond_with(json(200, json!({ "memory": memory(MEMORY) })))
        .mount(&server)
        .await;

    let updated = client
        .memories()
        .update(AGENT, MEMORY, "Prefers release notes under three lines.")
        .await
        .unwrap();

    assert_eq!(
        json_body(&requests(&server).await[0]),
        json!({ "text": "Prefers release notes under three lines." })
    );
    assert_eq!(updated.id.as_str(), MEMORY);
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/agents/{AGENT}/memories/{MEMORY}")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.memories().delete(AGENT, MEMORY).await.unwrap();
}

#[tokio::test]
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories")))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [memory("mem_1"), memory("mem_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories")))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [memory("mem_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.memories().list(AGENT).limit(2).items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|memory| memory.id.into_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["mem_1", "mem_2", "mem_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[tokio::test]
async fn api_errors_carry_the_envelope() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/memories/mem_missing")))
        .respond_with(api_error(404, "not_found", "Memory not found."))
        .mount(&server)
        .await;

    let error = client
        .memories()
        .get(AGENT, "mem_missing")
        .await
        .unwrap_err();

    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), Some(&ErrorCode::NotFound));
    assert_eq!(error.to_string(), "[not_found] Memory not found.");
}
