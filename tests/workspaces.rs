mod support;

use blazing_agents::Error;
use blazing_agents::ids::{TenantId, WorkspaceId};
use blazing_agents::types::Metadata;
use blazing_agents::workspaces::{NetworkPolicy, WorkspaceTier};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn workspace(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "name": "Release files",
        "userId": "user_42",
        "metadata": { "project": "docs" },
        "tier": "core",
        "networkPolicy": {
            "mode": "allowlist",
            "allowedHosts": ["registry.npmjs.org"]
        },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_the_body_and_decodes_the_workspace() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/workspaces"))
        .respond_with(json(201, workspace("ws_3Vb8Ny6HpU1cGf4M")))
        .mount(&server)
        .await;

    let created = client
        .workspaces()
        .create()
        .tier(WorkspaceTier::Plus)
        .name("Release files")
        .user_id("user_42")
        .metadata(Metadata::new().insert("project", "docs"))
        .network_policy(NetworkPolicy::allowlist(["registry.npmjs.org"]))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "tier": "plus",
            "name": "Release files",
            "userId": "user_42",
            "metadata": { "project": "docs" },
            "networkPolicy": {
                "mode": "allowlist",
                "allowedHosts": ["registry.npmjs.org"]
            }
        })
    );
    assert_eq!(created.id, WorkspaceId::from("ws_3Vb8Ny6HpU1cGf4M"));
    assert_eq!(created.tenant_id, TenantId::from("ten_8Hq2Zr5WcY1bJt6D"));
    assert_eq!(created.tier, WorkspaceTier::Core);
    assert_eq!(created.name.as_deref(), Some("Release files"));
    assert_eq!(created.user_id, "user_42");
    assert_eq!(created.metadata.get("project"), Some(&json!("docs")));
    assert_eq!(
        created.network_policy,
        NetworkPolicy::allowlist(["registry.npmjs.org"])
    );
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(created.updated_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn create_with_no_options_sends_an_empty_body() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/workspaces"))
        .respond_with(json(201, workspace("ws_3Vb8Ny6HpU1cGf4M")))
        .mount(&server)
        .await;

    client.workspaces().create().await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({}));
}

#[tokio::test]
async fn list_sends_filters_as_query_parameters() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/workspaces"))
        .and(query_param("userId", "customer_123"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [workspace("ws_3Vb8Ny6HpU1cGf4M")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .workspaces()
        .list()
        .user_id("customer_123")
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].id, WorkspaceId::from("ws_3Vb8Ny6HpU1cGf4M"));
    assert_eq!(
        page.next_cursor.map(|cursor| cursor.to_string()),
        Some("c2".to_owned())
    );
}

#[tokio::test]
async fn get_ignores_unknown_response_fields() {
    let (server, client) = mock().await;
    let mut body = workspace("ws_3Vb8Ny6HpU1cGf4M");
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client
        .workspaces()
        .get("ws_3Vb8Ny6HpU1cGf4M")
        .await
        .unwrap();

    assert_eq!(fetched.name.as_deref(), Some("Release files"));
}

#[tokio::test]
async fn unknown_modes_decode_as_unrecognized() {
    let (server, client) = mock().await;
    let mut body = workspace("ws_3Vb8Ny6HpU1cGf4M");
    body["tier"] = json!("ultra");
    body["networkPolicy"] = json!({ "mode": "vpn", "ruleSet": "corp" });
    Mock::given(method("GET"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client
        .workspaces()
        .get("ws_3Vb8Ny6HpU1cGf4M")
        .await
        .unwrap();

    match &fetched.tier {
        WorkspaceTier::Unrecognized(tier) => assert_eq!(tier.as_str(), "ultra"),
        other => panic!("expected an unrecognized tier, got {other:?}"),
    }
    assert_eq!(fetched.tier.as_str(), "ultra");
    match &fetched.network_policy {
        NetworkPolicy::Unrecognized(policy) => assert_eq!(policy.mode(), "vpn"),
        other => panic!("expected an unrecognized mode, got {other:?}"),
    }
}

#[tokio::test]
async fn get_rejects_a_malformed_allowlist() {
    let (server, client) = mock().await;
    let mut body = workspace("ws_3Vb8Ny6HpU1cGf4M");
    body["networkPolicy"] = json!({ "mode": "allowlist" });
    Mock::given(method("GET"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client
        .workspaces()
        .get("ws_3Vb8Ny6HpU1cGf4M")
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Decode { .. }), "got {error:?}");
}

#[tokio::test]
async fn update_sends_back_an_unknown_policy_unchanged() {
    let (server, client) = mock().await;
    let mut body = workspace("ws_3Vb8Ny6HpU1cGf4M");
    body["networkPolicy"] = json!({ "mode": "vpn", "gateway": "10.0.0.1" });
    Mock::given(method("GET"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, workspace("ws_3Vb8Ny6HpU1cGf4M")))
        .mount(&server)
        .await;

    let fetched = client
        .workspaces()
        .get("ws_3Vb8Ny6HpU1cGf4M")
        .await
        .unwrap();
    client
        .workspaces()
        .update("ws_3Vb8Ny6HpU1cGf4M")
        .network_policy(fetched.network_policy)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[1]),
        json!({ "networkPolicy": { "mode": "vpn", "gateway": "10.0.0.1" } })
    );
}

#[tokio::test]
async fn update_omits_unset_fields_and_sends_null_to_clear() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(json(200, workspace("ws_3Vb8Ny6HpU1cGf4M")))
        .mount(&server)
        .await;

    client
        .workspaces()
        .update("ws_3Vb8Ny6HpU1cGf4M")
        .metadata(Metadata::new().insert("project", "docs"))
        .await
        .unwrap();
    client
        .workspaces()
        .update("ws_3Vb8Ny6HpU1cGf4M")
        .clear_name()
        .await
        .unwrap();
    client
        .workspaces()
        .update("ws_3Vb8Ny6HpU1cGf4M")
        .name("Renamed")
        .network_policy(NetworkPolicy::Offline)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "metadata": { "project": "docs" } })
    );
    assert_eq!(json_body(&sent[1]), json!({ "name": null }));
    assert_eq!(
        json_body(&sent[2]),
        json!({ "name": "Renamed", "networkPolicy": { "mode": "offline" } })
    );
}

#[tokio::test]
async fn delete_accepts_202_with_an_empty_body() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/workspaces/ws_3Vb8Ny6HpU1cGf4M"))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;

    client
        .workspaces()
        .delete("ws_3Vb8Ny6HpU1cGf4M")
        .await
        .unwrap();
}

#[tokio::test]
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/workspaces"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [workspace("ws_1"), workspace("ws_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/workspaces"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [workspace("ws_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.workspaces().list().limit(2).items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|workspace| workspace.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["ws_1", "ws_2", "ws_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[test]
fn network_policy_serializes_to_the_wire_shape() {
    assert_eq!(
        serde_json::to_value(NetworkPolicy::Unrestricted).unwrap(),
        json!({ "mode": "unrestricted" })
    );
    assert_eq!(
        serde_json::to_value(NetworkPolicy::allowlist([
            "registry.npmjs.org",
            "api.example.com"
        ]))
        .unwrap(),
        json!({
            "mode": "allowlist",
            "allowedHosts": ["registry.npmjs.org", "api.example.com"]
        })
    );
    assert_eq!(
        serde_json::to_value(NetworkPolicy::Offline).unwrap(),
        json!({ "mode": "offline" })
    );
}

#[test]
fn network_policy_decodes_each_mode() {
    assert_eq!(
        serde_json::from_value::<NetworkPolicy>(json!({ "mode": "unrestricted" })).unwrap(),
        NetworkPolicy::Unrestricted
    );
    let NetworkPolicy::Allowlist { allowed_hosts, .. } = serde_json::from_value(
        json!({ "mode": "allowlist", "allowedHosts": ["registry.npmjs.org"] }),
    )
    .unwrap() else {
        panic!("expected an allowlist policy");
    };
    assert_eq!(allowed_hosts, ["registry.npmjs.org"]);
    assert_eq!(
        serde_json::from_value::<NetworkPolicy>(json!({ "mode": "offline" })).unwrap(),
        NetworkPolicy::Offline
    );
}

#[test]
fn unrecognized_network_policy_round_trips() {
    let raw = json!({ "mode": "vpn", "gateway": "10.0.0.1" });
    let policy = serde_json::from_value::<NetworkPolicy>(raw.clone()).unwrap();

    let NetworkPolicy::Unrecognized(inner) = &policy else {
        panic!("expected an unrecognized policy, got {policy:?}");
    };
    assert_eq!(inner.mode(), "vpn");
    assert_eq!(inner.as_json()["gateway"], json!("10.0.0.1"));
    assert_eq!(serde_json::to_value(&policy).unwrap(), raw);
}

#[test]
fn network_policy_rejects_a_missing_or_non_string_mode() {
    assert!(serde_json::from_value::<NetworkPolicy>(json!({ "allowedHosts": [] })).is_err());
    assert!(serde_json::from_value::<NetworkPolicy>(json!({ "mode": 7 })).is_err());
}

#[test]
fn network_policy_rejects_a_malformed_allowlist() {
    assert!(serde_json::from_value::<NetworkPolicy>(json!({ "mode": "allowlist" })).is_err());
    assert!(
        serde_json::from_value::<NetworkPolicy>(
            json!({ "mode": "allowlist", "allowedHosts": "registry.npmjs.org" })
        )
        .is_err()
    );
}
