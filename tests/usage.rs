mod support;

use blazing_agents::ids::{AgentId, SessionId};
use blazing_agents::usage::UsageGroupBy;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::Mock;
use wiremock::matchers::{method, path, query_param};

fn bucket() -> serde_json::Value {
    json!({
        "day": "2026-07-09",
        "agentId": null,
        "sessionId": null,
        "userId": null,
        "provider": null,
        "model": null,
        "inputTokens": 18240,
        "outputTokens": 6120,
        "requestCount": 14,
        "durationMs": 41300
    })
}

fn usage() -> serde_json::Value {
    json!({
        "buckets": [bucket()],
        "totals": {
            "inputTokens": 18240,
            "outputTokens": 6120,
            "requestCount": 14,
            "durationMs": 41300
        }
    })
}

#[tokio::test]
async fn get_sends_the_query_and_the_user_header() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/usage"))
        .and(query_param("from", "2026-07-01"))
        .and(query_param("to", "2026-07-10"))
        .and(query_param("agentId", "ag_4kP9sT2vXq7LmN3a"))
        .and(query_param("sessionId", ""))
        .and(query_param("userId", "customer_123"))
        .and(query_param("groupBy", "session"))
        .and(query_param("limit", "10"))
        .respond_with(json(200, usage()))
        .mount(&server)
        .await;

    let report = client
        .for_user("customer_123")
        .unwrap()
        .usage()
        .get()
        .range("2026-07-01", "2026-07-10")
        .agent_id("ag_4kP9sT2vXq7LmN3a")
        .session_id("")
        .user_id("customer_123")
        .group_by(UsageGroupBy::Session)
        .limit(10)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        sent[0]
            .headers
            .get("x-ba-user-id")
            .map(|value| value.to_str().unwrap()),
        Some("customer_123")
    );
    assert_eq!(report.buckets.len(), 1);
    assert_eq!(report.buckets[0].day.as_deref(), Some("2026-07-09"));
    assert_eq!(report.buckets[0].input_tokens, 18240);
    assert_eq!(report.totals.request_count, 14);
    assert_eq!(report.totals.duration_ms, 41300);
}

#[tokio::test]
async fn get_without_filters_sends_an_empty_query() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/usage"))
        .respond_with(json(200, usage()))
        .mount(&server)
        .await;

    client.usage().get().await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), None);
}

#[tokio::test]
async fn get_for_agent_sends_the_query_on_the_tenant_client() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_4kP9sT2vXq7LmN3a/usage"))
        .and(query_param("from", "2026-07-01"))
        .and(query_param("to", "2026-07-10"))
        .and(query_param("sessionId", "ss_0123456789abcdef"))
        .and(query_param("userId", "customer_123"))
        .and(query_param("groupBy", "model"))
        .and(query_param("limit", "25"))
        .respond_with(json(200, usage()))
        .mount(&server)
        .await;

    let report = client
        .usage()
        .get_for_agent("ag_4kP9sT2vXq7LmN3a")
        .range("2026-07-01", "2026-07-10")
        .session_id("ss_0123456789abcdef")
        .user_id("customer_123")
        .group_by(UsageGroupBy::Model)
        .limit(25)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert!(!sent[0].headers.contains_key("x-ba-user-id"));
    assert_eq!(report.totals.input_tokens, 18240);
}

#[tokio::test]
async fn overview_decodes_the_breakdowns() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/usage/overview"))
        .and(query_param("from", "2026-07-01"))
        .and(query_param("to", "2026-07-10"))
        .and(query_param("limit", "20"))
        .respond_with(json(
            200,
            json!({
                "totals": {
                    "inputTokens": 27840,
                    "outputTokens": 9170,
                    "requestCount": 22,
                    "durationMs": 64050
                },
                "daily": [bucket()],
                "byAgent": [{
                    "day": null,
                    "agentId": "ag_4kP9sT2vXq7LmN3a",
                    "sessionId": null,
                    "userId": null,
                    "provider": null,
                    "model": null,
                    "inputTokens": 27840,
                    "outputTokens": 9170,
                    "requestCount": 22,
                    "durationMs": 64050
                }],
                "byUser": [{
                    "day": null,
                    "agentId": null,
                    "sessionId": null,
                    "userId": "",
                    "provider": null,
                    "model": null,
                    "inputTokens": 7740,
                    "outputTokens": 2370,
                    "requestCount": 7,
                    "durationMs": 18850,
                    "addedNextYear": true
                }],
                "byModel": [{
                    "day": null,
                    "agentId": null,
                    "sessionId": null,
                    "userId": null,
                    "provider": "openrouter",
                    "model": "openai/gpt-6-luna",
                    "inputTokens": 27840,
                    "outputTokens": 9170,
                    "requestCount": 22,
                    "durationMs": 64050
                }],
                "activeAgentCount": 1,
                "addedNextYear": { "nested": true }
            }),
        ))
        .mount(&server)
        .await;

    let overview = client
        .usage()
        .overview()
        .range("2026-07-01", "2026-07-10")
        .limit(20)
        .await
        .unwrap();

    assert_eq!(overview.totals.input_tokens, 27840);
    assert_eq!(overview.daily.len(), 1);
    assert_eq!(
        overview.by_agent[0].agent_id,
        Some(AgentId::from("ag_4kP9sT2vXq7LmN3a"))
    );
    assert_eq!(overview.by_user[0].user_id.as_deref(), Some(""));
    assert_eq!(overview.by_model[0].provider.as_deref(), Some("openrouter"));
    assert_eq!(
        overview.by_model[0].model.as_deref(),
        Some("openai/gpt-6-luna")
    );
    assert_eq!(overview.active_agent_count, 1);
}

#[tokio::test]
async fn sessions_posts_the_ids_and_decodes_the_entries() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/usage/sessions"))
        .respond_with(json(
            200,
            json!({
                "data": [{
                    "sessionId": "ss_0123456789abcdef",
                    "totals": {
                        "inputTokens": 12,
                        "outputTokens": 6,
                        "requestCount": 1,
                        "durationMs": 450
                    }
                }]
            }),
        ))
        .mount(&server)
        .await;

    let report = client
        .for_user("customer_123")
        .unwrap()
        .usage()
        .sessions(["ss_0123456789abcdef", "ss_fedcba9876543210"])
        .range("2026-07-01", "2026-07-10")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        sent[0]
            .headers
            .get("x-ba-user-id")
            .map(|value| value.to_str().unwrap()),
        Some("customer_123")
    );
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "sessionIds": ["ss_0123456789abcdef", "ss_fedcba9876543210"],
            "from": "2026-07-01",
            "to": "2026-07-10"
        })
    );
    assert_eq!(report.data.len(), 1);
    assert_eq!(
        report.data[0].session_id,
        SessionId::from("ss_0123456789abcdef")
    );
    assert_eq!(report.data[0].totals.input_tokens, 12);
}
