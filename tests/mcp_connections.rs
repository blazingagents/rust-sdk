mod support;

use blazing_agents::Error;
use blazing_agents::ids::McpConnectionId;
use blazing_agents::mcp_connections::{
    McpAuthType, McpConnectionErrorCode, McpConnectionStatus, McpReconnect, NewMcpConnection,
};
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

fn connection(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": "Docs Search",
        "url": "https://mcp.example.com/mcp",
        "authType": "bearer",
        "status": "connected",
        "credentialFragment": "x7Qa",
        "lastAuthErrorCode": null,
        "oauthIssuer": null,
        "oauthResource": null,
        "tokenExpiresAt": null,
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

#[tokio::test]
async fn create_sends_one_body_per_auth_type() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections"))
        .respond_with(json(201, connection("mcp_3Kd9QwE2sLx5R7Tn")))
        .mount(&server)
        .await;

    client
        .mcp_connections()
        .create(NewMcpConnection::None {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
        })
        .await
        .unwrap();
    client
        .mcp_connections()
        .create(NewMcpConnection::Bearer {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            bearer_token: "mcp_live_4f8Kq2Lz9Xw7x7Qa".to_owned(),
        })
        .await
        .unwrap();
    client
        .mcp_connections()
        .create(NewMcpConnection::OAuthAuthorizationCode {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            client_id: None,
            client_secret: None,
            scope: None,
        })
        .await
        .unwrap();
    client
        .mcp_connections()
        .create(NewMcpConnection::OAuthAuthorizationCode {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            client_id: Some("client-42".to_owned()),
            client_secret: Some("oauth-secret-a1b2c3".to_owned()),
            scope: Some("read:docs".to_owned()),
        })
        .await
        .unwrap();
    client
        .mcp_connections()
        .create(NewMcpConnection::OAuthClientCredentials {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            client_id: "client-42".to_owned(),
            client_secret: "oauth-secret-a1b2c3".to_owned(),
            scope: None,
        })
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 5);
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "authType": "none",
            "name": "Docs Search",
            "url": "https://mcp.example.com/mcp"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "authType": "bearer",
            "name": "Docs Search",
            "url": "https://mcp.example.com/mcp",
            "bearerToken": "mcp_live_4f8Kq2Lz9Xw7x7Qa"
        })
    );
    assert_eq!(
        json_body(&sent[2]),
        json!({
            "authType": "oauth_authorization_code",
            "name": "Docs Search",
            "url": "https://mcp.example.com/mcp"
        })
    );
    assert_eq!(
        json_body(&sent[3]),
        json!({
            "authType": "oauth_authorization_code",
            "name": "Docs Search",
            "url": "https://mcp.example.com/mcp",
            "clientId": "client-42",
            "clientSecret": "oauth-secret-a1b2c3",
            "scope": "read:docs"
        })
    );
    assert_eq!(
        json_body(&sent[4]),
        json!({
            "authType": "oauth_client_credentials",
            "name": "Docs Search",
            "url": "https://mcp.example.com/mcp",
            "clientId": "client-42",
            "clientSecret": "oauth-secret-a1b2c3"
        })
    );
}

#[tokio::test]
async fn list_decodes_connections_and_ignores_unknown_fields() {
    let (server, client) = mock().await;
    let mut entry = connection("mcp_3Kd9QwE2sLx5R7Tn");
    entry["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/mcp-connections"))
        .respond_with(json(200, json!({ "mcpConnections": [entry] })))
        .mount(&server)
        .await;

    let list = client.mcp_connections().list().await.unwrap();

    assert_eq!(list.mcp_connections.len(), 1);
    let connection = &list.mcp_connections[0];
    assert_eq!(connection.id, McpConnectionId::from("mcp_3Kd9QwE2sLx5R7Tn"));
    assert_eq!(connection.auth_type, McpAuthType::Bearer);
    assert_eq!(connection.status, McpConnectionStatus::Connected);
    assert_eq!(connection.credential_fragment.as_deref(), Some("x7Qa"));
    assert_eq!(connection.last_auth_error_code, None);
    assert_eq!(connection.created_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn unknown_enum_values_decode_as_unrecognized() {
    let (server, client) = mock().await;
    let mut body = connection("mcp_3Kd9QwE2sLx5R7Tn");
    body["authType"] = json!("mtls");
    body["status"] = json!("suspended");
    body["lastAuthErrorCode"] = json!("MCP_CONNECTION_RATE_LIMITED");
    Mock::given(method("GET"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client
        .mcp_connections()
        .get("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();

    assert_eq!(fetched.auth_type.as_str(), "mtls");
    assert!(matches!(fetched.auth_type, McpAuthType::Unrecognized(_)));
    assert_eq!(fetched.status.as_str(), "suspended");
    assert!(matches!(
        fetched.status,
        McpConnectionStatus::Unrecognized(_)
    ));
    assert_eq!(
        fetched
            .last_auth_error_code
            .as_ref()
            .map(McpConnectionErrorCode::as_str),
        Some("MCP_CONNECTION_RATE_LIMITED")
    );
}

#[tokio::test]
async fn update_sends_the_new_name() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn"))
        .respond_with(json(200, connection("mcp_3Kd9QwE2sLx5R7Tn")))
        .mount(&server)
        .await;

    client
        .mcp_connections()
        .update("mcp_3Kd9QwE2sLx5R7Tn", "Docs Search v2")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Docs Search v2" }));
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .mcp_connections()
        .delete("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_decodes_the_success_and_failure_shapes() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/test"))
        .respond_with(json(
            200,
            json!({
                "ok": true,
                "latencyMs": 120,
                "server": { "name": "docs-search", "version": "1.4.0" },
                "toolCount": 2,
                "toolNames": ["search", "fetch"]
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_9QwE2sLx5R7Tn3Kd4/test"))
        .respond_with(json(
            200,
            json!({
                "ok": false,
                "error": {
                    "code": "MCP_CONNECTION_UNREACHABLE",
                    "message": "The server did not respond."
                }
            }),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let ok = client
        .mcp_connections()
        .test("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();
    let failed = client
        .mcp_connections()
        .test("mcp_9QwE2sLx5R7Tn3Kd4")
        .await
        .unwrap();

    assert!(ok.ok);
    assert_eq!(ok.latency_ms, Some(120));
    let server_info = ok.server.unwrap();
    assert_eq!(server_info.name, "docs-search");
    assert_eq!(server_info.version, "1.4.0");
    assert_eq!(ok.tool_count, Some(2));
    assert_eq!(
        ok.tool_names.as_deref().unwrap(),
        ["search".to_owned(), "fetch".to_owned()]
    );
    assert!(!failed.ok);
    let error = failed.error.unwrap();
    assert_eq!(error.code, McpConnectionErrorCode::Unreachable);
    assert_eq!(error.message, "The server did not respond.");
    assert!(failed.server.is_none());
}

#[tokio::test]
async fn reconnect_sends_the_auth_body_and_decodes_needs_auth() {
    let (server, client) = mock().await;
    let mut pending = connection("mcp_3Kd9QwE2sLx5R7Tn");
    pending["authType"] = json!("oauth_authorization_code");
    pending["status"] = json!("needs_auth");
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/reconnect"))
        .respond_with(json(
            200,
            json!({ "status": "needs_auth", "connection": pending }),
        ))
        .mount(&server)
        .await;

    let result = client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::Bearer {
                url: "https://mcp.example.com/mcp".to_owned(),
                bearer_token: "mcp_live_9Tb3Vc6Nm1Pq8Rw2".to_owned(),
            },
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "authType": "bearer",
            "url": "https://mcp.example.com/mcp",
            "bearerToken": "mcp_live_9Tb3Vc6Nm1Pq8Rw2"
        })
    );
    assert_eq!(result.status, McpConnectionStatus::NeedsAuth);
    assert_eq!(result.connection.status, McpConnectionStatus::NeedsAuth);
    assert_eq!(
        result.connection.auth_type,
        McpAuthType::OAuthAuthorizationCode
    );
}

#[tokio::test]
async fn an_error_echoing_a_secret_is_redacted() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections"))
        .respond_with(api_error(
            400,
            "invalid_request",
            "The bearer token mcp_live_4f8Kq2Lz9Xw7x7Qa is rejected.",
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/reconnect"))
        .respond_with(api_error(
            400,
            "invalid_request",
            "The client secret oauth-secret-a1b2c3 is rejected.",
        ))
        .mount(&server)
        .await;

    let create_error = client
        .mcp_connections()
        .create(NewMcpConnection::Bearer {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            bearer_token: "mcp_live_4f8Kq2Lz9Xw7x7Qa".to_owned(),
        })
        .await
        .unwrap_err();
    let reconnect_error = client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::OAuthClientCredentials {
                url: "https://mcp.example.com/mcp".to_owned(),
                client_id: "client-42".to_owned(),
                client_secret: "oauth-secret-a1b2c3".to_owned(),
                scope: None,
            },
        )
        .await
        .unwrap_err();

    let Error::Api(create_api) = create_error else {
        panic!("expected an API error, got {create_error:?}");
    };
    assert!(!create_api.body().contains("mcp_live_4f8Kq2Lz9Xw7x7Qa"));
    assert!(!create_api.message().contains("mcp_live_4f8Kq2Lz9Xw7x7Qa"));
    let Error::Api(reconnect_api) = reconnect_error else {
        panic!("expected an API error, got {reconnect_error:?}");
    };
    assert!(!reconnect_api.body().contains("oauth-secret-a1b2c3"));
    assert!(!reconnect_api.message().contains("oauth-secret-a1b2c3"));
}

#[tokio::test]
async fn the_debug_output_does_not_leak_secrets() {
    let create = NewMcpConnection::Bearer {
        name: "Docs Search".to_owned(),
        url: "https://mcp.example.com/mcp".to_owned(),
        bearer_token: "mcp_live_4f8Kq2Lz9Xw7x7Qa".to_owned(),
    };
    let reconnect = McpReconnect::OAuthClientCredentials {
        url: "https://mcp.example.com/mcp".to_owned(),
        client_id: "client-42".to_owned(),
        client_secret: "oauth-secret-a1b2c3".to_owned(),
        scope: None,
    };

    let create_debug = format!("{create:?}");
    let reconnect_debug = format!("{reconnect:?}");

    assert!(!create_debug.contains("mcp_live_4f8Kq2Lz9Xw7x7Qa"));
    assert!(create_debug.contains("[redacted]"));
    assert!(create_debug.contains("Docs Search"));
    assert!(!reconnect_debug.contains("oauth-secret-a1b2c3"));
    assert!(reconnect_debug.contains("[redacted]"));
    assert!(reconnect_debug.contains("client-42"));
}
