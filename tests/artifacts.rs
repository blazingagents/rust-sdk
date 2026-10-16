mod support;

use blazing_agents::ids::{AgentId, ArtifactId, SessionId, TenantId};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn artifact(id: &str) -> serde_json::Value {
    json!({
        "artifactId": id,
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "sessionId": "ss_9Fh3Lc7VbN2kDs5Y",
        "filename": "report.pdf",
        "mediaType": "application/pdf",
        "sizeBytes": 24830,
        "userId": "",
        "metadata": {},
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

#[tokio::test]
async fn list_sends_filters_as_query_parameters() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/artifacts"))
        .and(query_param("agentId", "ag_4kP9sT2vXq7LmN3a"))
        .and(query_param("sessionId", "ss_9Fh3Lc7VbN2kDs5Y"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [artifact("at_6Jm2Qx8RtW4nPz1K")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .artifacts()
        .list()
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .session_id("ss_9Fh3Lc7VbN2kDs5Y")
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );
}

#[tokio::test]
async fn get_decodes_every_field_and_ignores_unknown_ones() {
    let (server, client) = mock().await;
    let mut body = artifact("at_6Jm2Qx8RtW4nPz1K");
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/artifacts/at_6Jm2Qx8RtW4nPz1K"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.artifacts().get("at_6Jm2Qx8RtW4nPz1K").await.unwrap();

    assert_eq!(fetched.artifact_id, ArtifactId::from("at_6Jm2Qx8RtW4nPz1K"));
    assert_eq!(fetched.agent_id, AgentId::from("ag_4kP9sT2vXq7LmN3a"));
    assert_eq!(fetched.tenant_id, TenantId::from("ten_8Hq2Zr5WcY1bJt6D"));
    assert_eq!(fetched.session_id, SessionId::from("ss_9Fh3Lc7VbN2kDs5Y"));
    assert_eq!(fetched.filename, "report.pdf");
    assert_eq!(fetched.media_type, "application/pdf");
    assert_eq!(fetched.size_bytes, 24830);
    assert_eq!(fetched.user_id, "");
    assert_eq!(fetched.metadata.as_map(), &serde_json::Map::new());
    assert_eq!(fetched.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(fetched.updated_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn create_download_url_decodes_and_redacts_debug() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/artifacts/at_6Jm2Qx8RtW4nPz1K/download-url"))
        .respond_with(json(
            200,
            json!({
                "url": "https://downloads.example.com/at_6Jm2Qx8RtW4nPz1K/report.pdf?signature=3f9a2c",
                "expiresAt": "2026-07-10T10:05:00.000Z"
            }),
        ))
        .mount(&server)
        .await;

    let download = client
        .artifacts()
        .create_download_url("at_6Jm2Qx8RtW4nPz1K")
        .await
        .unwrap();

    assert_eq!(
        download.url,
        "https://downloads.example.com/at_6Jm2Qx8RtW4nPz1K/report.pdf?signature=3f9a2c"
    );
    assert_eq!(download.expires_at.to_string(), "2026-07-10T10:05:00Z");
    let debug = format!("{download:?}");
    assert!(
        !debug.contains(&download.url),
        "debug leaks the URL: {debug}"
    );
    assert!(debug.contains("[redacted]"));
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/artifacts/at_6Jm2Qx8RtW4nPz1K"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .artifacts()
        .delete("at_6Jm2Qx8RtW4nPz1K")
        .await
        .unwrap();
}

#[tokio::test]
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/artifacts"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [artifact("at_1"), artifact("at_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/artifacts"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [artifact("at_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.artifacts().list().items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|artifact| artifact.artifact_id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["at_1", "at_2", "at_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}
