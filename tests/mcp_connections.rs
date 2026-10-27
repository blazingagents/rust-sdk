mod support;

use blazing_agents::Error;
use blazing_agents::ids::McpConnectionId;
use blazing_agents::mcp_connections::{
    McpAuthType, McpConnectionErrorCode, McpConnectionStatus, McpConnectionTest, McpReconnect,
    McpReconnectStatus, NewMcpConnection, OAuthClient,
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
            client: None,
            scope: None,
        })
        .await
        .unwrap();
    client
        .mcp_connections()
        .create(NewMcpConnection::OAuthAuthorizationCode {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
            scope: Some("read:docs".to_owned()),
        })
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 4);
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
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/reconnect"))
        .respond_with(json(
            200,
            json!({
                "status": "provisioning",
                "connection": connection("mcp_3Kd9QwE2sLx5R7Tn")
            }),
        ))
        .mount(&server)
        .await;

    let fetched = client
        .mcp_connections()
        .get("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();
    let reconnected = client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::None {
                url: "https://mcp.example.com/mcp".to_owned(),
            },
        )
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
    assert_eq!(reconnected.status.as_str(), "provisioning");
    assert!(matches!(
        reconnected.status,
        McpReconnectStatus::Unrecognized(_)
    ));
}

#[tokio::test]
async fn get_decodes_oauth_fields() {
    let (server, client) = mock().await;
    let mut body = connection("mcp_3Kd9QwE2sLx5R7Tn");
    body["authType"] = json!("oauth_authorization_code");
    body["status"] = json!("error");
    body["lastAuthErrorCode"] = json!("MCP_CONNECTION_AUTHENTICATION_FAILED");
    body["credentialFragment"] = json!(null);
    body["oauthIssuer"] = json!("https://auth.example.com");
    body["oauthResource"] = json!("https://mcp.example.com/mcp");
    body["tokenExpiresAt"] = json!("2026-07-10T11:00:00.000Z");
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

    assert_eq!(fetched.auth_type, McpAuthType::OAuthAuthorizationCode);
    assert_eq!(fetched.status, McpConnectionStatus::Error);
    assert_eq!(
        fetched.last_auth_error_code,
        Some(McpConnectionErrorCode::AuthenticationFailed)
    );
    assert_eq!(fetched.credential_fragment, None);
    assert_eq!(
        fetched.oauth_issuer.as_deref(),
        Some("https://auth.example.com")
    );
    assert_eq!(
        fetched.oauth_resource.as_deref(),
        Some("https://mcp.example.com/mcp")
    );
    assert_eq!(
        fetched.token_expires_at.map(|at| at.to_string()),
        Some("2026-07-10T11:00:00Z".to_owned())
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

    let McpConnectionTest::Passed {
        server,
        tool_names,
        latency_ms,
        tool_count,
    } = ok
    else {
        panic!("expected a passed test, got {ok:?}");
    };
    assert_eq!(latency_ms, 120);
    assert_eq!(server.name, "docs-search");
    assert_eq!(server.version, "1.4.0");
    assert_eq!(tool_count, 2);
    assert_eq!(tool_names, ["search".to_owned(), "fetch".to_owned()]);

    let McpConnectionTest::Failed { error } = failed else {
        panic!("expected a failed test, got {failed:?}");
    };
    assert_eq!(error.code, McpConnectionErrorCode::Unreachable);
    assert_eq!(error.message, "The server did not respond.");
}

#[tokio::test]
async fn a_test_result_serializes_back_to_its_wire_shape() {
    let (server, client) = mock().await;
    let passed_body = json!({
        "ok": true,
        "latencyMs": 120,
        "server": { "name": "docs-search", "version": "1.4.0" },
        "toolCount": 2,
        "toolNames": ["search", "fetch"]
    });
    let failed_body = json!({
        "ok": false,
        "error": {
            "code": "MCP_CONNECTION_UNREACHABLE",
            "message": "The server did not respond."
        }
    });
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/test"))
        .respond_with(json(200, passed_body.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_9QwE2sLx5R7Tn3Kd4/test"))
        .respond_with(json(200, failed_body.clone()))
        .expect(1)
        .mount(&server)
        .await;

    let passed = client
        .mcp_connections()
        .test("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();
    let failed = client
        .mcp_connections()
        .test("mcp_9QwE2sLx5R7Tn3Kd4")
        .await
        .unwrap();

    let serialized = serde_json::to_value(&passed).unwrap();
    assert_eq!(serialized, passed_body);
    let roundtripped = serde_json::from_value::<McpConnectionTest>(serialized).unwrap();
    assert!(matches!(roundtripped, McpConnectionTest::Passed { .. }));

    let serialized = serde_json::to_value(&failed).unwrap();
    assert_eq!(serialized, failed_body);
    let roundtripped = serde_json::from_value::<McpConnectionTest>(serialized).unwrap();
    assert!(matches!(roundtripped, McpConnectionTest::Failed { .. }));
}

#[tokio::test]
async fn a_failed_test_with_an_unknown_code_decodes_as_unrecognized() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/test"))
        .respond_with(json(
            200,
            json!({
                "ok": false,
                "error": {
                    "code": "MCP_CONNECTION_RATE_LIMITED",
                    "message": "Slow down."
                }
            }),
        ))
        .mount(&server)
        .await;

    let failed = client
        .mcp_connections()
        .test("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap();

    let McpConnectionTest::Failed { error } = failed else {
        panic!("expected a failed test, got {failed:?}");
    };
    assert!(matches!(
        error.code,
        McpConnectionErrorCode::Unrecognized(_)
    ));
    assert_eq!(error.code.as_str(), "MCP_CONNECTION_RATE_LIMITED");
    assert_eq!(error.message, "Slow down.");
}

#[tokio::test]
async fn malformed_test_results_are_decode_errors() {
    let (server, client) = mock().await;
    let bodies = [
        // `ok: true` without the success fields.
        json!({ "ok": true }),
        // `ok: false` without the error.
        json!({ "ok": false }),
        // `ok: true` carrying only an error.
        json!({
            "ok": true,
            "error": {
                "code": "MCP_CONNECTION_UNREACHABLE",
                "message": "The server did not respond."
            }
        }),
        // A non-boolean `ok`.
        json!({
            "ok": "yes",
            "server": { "name": "docs-search", "version": "1.4.0" },
            "toolNames": ["search"],
            "latencyMs": 120,
            "toolCount": 1
        }),
    ];
    for body in bodies {
        let _guard = Mock::given(method("POST"))
            .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/test"))
            .respond_with(json(200, body.clone()))
            .expect(1)
            .mount_as_scoped(&server)
            .await;

        let error = client
            .mcp_connections()
            .test("mcp_3Kd9QwE2sLx5R7Tn")
            .await
            .unwrap_err();

        let Error::Decode { operation_id, .. } = error else {
            panic!("expected a decode error for {body}, got {error:?}");
        };
        assert_eq!(operation_id, "testMcpConnection");
    }
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
    assert_eq!(result.status, McpReconnectStatus::NeedsAuth);
    assert_eq!(result.connection.status, McpConnectionStatus::NeedsAuth);
    assert_eq!(
        result.connection.auth_type,
        McpAuthType::OAuthAuthorizationCode
    );
}

#[tokio::test]
async fn reconnect_sends_one_body_per_auth_type() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn/reconnect"))
        .respond_with(json(
            200,
            json!({
                "status": "connected",
                "connection": connection("mcp_3Kd9QwE2sLx5R7Tn")
            }),
        ))
        .mount(&server)
        .await;

    let first = client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::None {
                url: "https://mcp.example.com/mcp".to_owned(),
            },
        )
        .await
        .unwrap();
    client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: None,
                scope: None,
            },
        )
        .await
        .unwrap();
    client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
                scope: Some("read:docs".to_owned()),
            },
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 3);
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "authType": "none",
            "url": "https://mcp.example.com/mcp"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "authType": "oauth_authorization_code",
            "url": "https://mcp.example.com/mcp"
        })
    );
    assert_eq!(
        json_body(&sent[2]),
        json!({
            "authType": "oauth_authorization_code",
            "url": "https://mcp.example.com/mcp",
            "clientId": "client-42",
            "clientSecret": "oauth-secret-a1b2c3",
            "scope": "read:docs"
        })
    );
    assert_eq!(first.status, McpReconnectStatus::Connected);
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
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
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
async fn an_error_echoing_an_oauth_client_secret_is_redacted() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/mcp-connections"))
        .respond_with(api_error(
            400,
            "invalid_request",
            "The client secret oauth-secret-a1b2c3 is rejected.",
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

    let oauth_code_error = client
        .mcp_connections()
        .create(NewMcpConnection::OAuthAuthorizationCode {
            name: "Docs Search".to_owned(),
            url: "https://mcp.example.com/mcp".to_owned(),
            client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
            scope: None,
        })
        .await
        .unwrap_err();
    let reconnect_error = client
        .mcp_connections()
        .reconnect(
            "mcp_3Kd9QwE2sLx5R7Tn",
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
                scope: None,
            },
        )
        .await
        .unwrap_err();

    for error in [oauth_code_error, reconnect_error] {
        let Error::Api(api) = error else {
            panic!("expected an API error, got {error:?}");
        };
        assert!(!api.body().contains("oauth-secret-a1b2c3"));
        assert!(api.body().contains("[REDACTED]"));
        assert!(!api.message().contains("oauth-secret-a1b2c3"));
    }
}

#[tokio::test]
async fn a_response_echoing_a_bearer_token_is_rejected() {
    let (server, client) = mock().await;
    let mut body = connection("mcp_3Kd9QwE2sLx5R7Tn");
    body["bearerToken"] = json!("mcp_live_4f8Kq2Lz9Xw7x7Qa");
    Mock::given(method("GET"))
        .and(path("/v1/mcp-connections/mcp_3Kd9QwE2sLx5R7Tn"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client
        .mcp_connections()
        .get("mcp_3Kd9QwE2sLx5R7Tn")
        .await
        .unwrap_err();

    let Error::CredentialEcho { operation_id, .. } = error else {
        panic!("expected a credential echo error, got {error:?}");
    };
    assert_eq!(operation_id, "getMcpConnection");
}

#[test]
fn every_debug_variant_redacts_its_secrets() {
    let create_cases = [
        (
            NewMcpConnection::None {
                name: "Docs Search".to_owned(),
                url: "https://mcp.example.com/mcp".to_owned(),
            },
            r#"None { name: "Docs Search", url: "https://mcp.example.com/mcp" }"#,
        ),
        (
            NewMcpConnection::Bearer {
                name: "Docs Search".to_owned(),
                url: "https://mcp.example.com/mcp".to_owned(),
                bearer_token: "mcp_live_4f8Kq2Lz9Xw7x7Qa".to_owned(),
            },
            r#"Bearer { name: "Docs Search", url: "https://mcp.example.com/mcp", bearer_token: "[redacted]" }"#,
        ),
        (
            NewMcpConnection::OAuthAuthorizationCode {
                name: "Docs Search".to_owned(),
                url: "https://mcp.example.com/mcp".to_owned(),
                client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
                scope: Some("read:docs".to_owned()),
            },
            r#"OAuthAuthorizationCode { name: "Docs Search", url: "https://mcp.example.com/mcp", client: Some(OAuthClient { id: "client-42", secret: "[redacted]" }), scope: Some("read:docs") }"#,
        ),
        (
            NewMcpConnection::OAuthAuthorizationCode {
                name: "Docs Search".to_owned(),
                url: "https://mcp.example.com/mcp".to_owned(),
                client: None,
                scope: None,
            },
            r#"OAuthAuthorizationCode { name: "Docs Search", url: "https://mcp.example.com/mcp", client: None, scope: None }"#,
        ),
    ];
    for (connection, expected) in create_cases {
        assert_eq!(format!("{connection:?}"), expected);
    }

    let reconnect_cases = [
        (
            McpReconnect::None {
                url: "https://mcp.example.com/mcp".to_owned(),
            },
            r#"None { url: "https://mcp.example.com/mcp" }"#,
        ),
        (
            McpReconnect::Bearer {
                url: "https://mcp.example.com/mcp".to_owned(),
                bearer_token: "mcp_live_4f8Kq2Lz9Xw7x7Qa".to_owned(),
            },
            r#"Bearer { url: "https://mcp.example.com/mcp", bearer_token: "[redacted]" }"#,
        ),
        (
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: Some(OAuthClient::new("client-42", "oauth-secret-a1b2c3")),
                scope: Some("read:docs".to_owned()),
            },
            r#"OAuthAuthorizationCode { url: "https://mcp.example.com/mcp", client: Some(OAuthClient { id: "client-42", secret: "[redacted]" }), scope: Some("read:docs") }"#,
        ),
        (
            McpReconnect::OAuthAuthorizationCode {
                url: "https://mcp.example.com/mcp".to_owned(),
                client: None,
                scope: None,
            },
            r#"OAuthAuthorizationCode { url: "https://mcp.example.com/mcp", client: None, scope: None }"#,
        ),
    ];
    for (auth, expected) in reconnect_cases {
        assert_eq!(format!("{auth:?}"), expected);
    }
}
