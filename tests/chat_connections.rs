mod support;

use blazing_agents::Error;
use blazing_agents::chat_connections::{
    ChatConfiguration, ChatCredentials, ChatDeliveryListStatus, ChatDeliveryStatus,
    ChatHealthCheckStatus, ChatPlatform, NewChatConnection, NewSlackConnection,
    NewTelegramConnection, SlackCredentials, TelegramCredentials,
};
use blazing_agents::ids::{ChatConnectionId, ChatDeliveryId};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn chat_connection(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "Support bot",
        "platform": "slack",
        "enabled": true,
        "configuration": { "platform": "slack", "channelIds": ["C0123ABC"] },
        "identity": {
            "botId": "B0123ABC",
            "botUserId": "U0123ABC",
            "teamId": "T0123ABC",
            "appId": "A0123ABC"
        },
        "health": {
            "checkedAt": "2026-07-10T10:05:00.000Z",
            "tokenValid": true,
            "identityVerified": true,
            "checks": [
                { "code": "token", "status": "pass" },
                { "code": "channel", "status": "fail", "subject": "C0123ABC" }
            ]
        },
        "credentialFragment": "yzab",
        "credentialVersion": 3,
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "webhookUrl": "https://api.blazingagents.com/hooks/chat/cc_1"
    })
}

fn delivery(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "kind": "reply",
        "status": "failed",
        "attempt": 2,
        "credentialVersion": 3,
        "representation": "Your refund is on its way.",
        "diagnostic": "channel_not_found",
        "sessionId": "ss_9Pm2Vb6HxR4kTq8W",
        "messageId": "msg_1",
        "approvalId": null,
        "threadId": "1720684800.000100",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

#[tokio::test]
async fn list_returns_the_connections() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections"))
        .respond_with(json(
            200,
            json!({ "chatConnections": [chat_connection("cc_1")] }),
        ))
        .mount(&server)
        .await;

    let list = client.chat_connections().list().await.unwrap();

    assert_eq!(list.chat_connections.len(), 1);
    let connection = &list.chat_connections[0];
    assert_eq!(connection.id, ChatConnectionId::from("cc_1"));
    assert_eq!(connection.platform, ChatPlatform::Slack);
    assert_eq!(connection.credential_fragment, "yzab");
    assert_eq!(connection.credential_version, 3);
    let ChatConfiguration::Slack(configuration) = &connection.configuration else {
        panic!(
            "expected a Slack configuration, got {:?}",
            connection.configuration
        );
    };
    assert_eq!(configuration.channel_ids, ["C0123ABC"]);
    assert_eq!(connection.identity.team_id.as_deref(), Some("T0123ABC"));
    assert!(connection.health.token_valid);
    assert_eq!(
        connection.health.checks[0].status,
        ChatHealthCheckStatus::Pass
    );
    assert_eq!(connection.health.checks[0].subject, None);
    assert_eq!(
        connection.health.checks[1].subject.as_deref(),
        Some("C0123ABC")
    );
    assert_eq!(connection.created_at.to_string(), "2026-07-10T10:00:00Z");
}

#[tokio::test]
async fn create_sends_only_the_set_fields() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections"))
        .respond_with(json(201, chat_connection("cc_1")))
        .mount(&server)
        .await;

    client
        .chat_connections()
        .create(NewChatConnection::slack(
            "ag_4kP9sT2vXq7LmN3a",
            "Support bot",
            SlackCredentials::new("xoxb-token-value", "0123456789abcdef0123456789abcdef"),
        ))
        .await
        .unwrap();
    client
        .chat_connections()
        .create(
            NewTelegramConnection::new(
                "ag_4kP9sT2vXq7LmN3a",
                "Business bot",
                TelegramCredentials::new("7123456789:AAFtoken"),
            )
            .enabled(false)
            .business_mode(true)
            .chat_ids(["-1001234"]),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "platform": "slack",
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "name": "Support bot",
            "credentials": {
                "botToken": "xoxb-token-value",
                "signingSecret": "0123456789abcdef0123456789abcdef"
            }
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "platform": "telegram",
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "name": "Business bot",
            "enabled": false,
            "configuration": { "businessMode": true, "chatIds": ["-1001234"] },
            "credentials": { "botToken": "7123456789:AAFtoken" }
        })
    );
}

#[tokio::test]
async fn create_accepts_a_platform_variant_directly() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections"))
        .respond_with(json(201, chat_connection("cc_1")))
        .mount(&server)
        .await;

    client
        .chat_connections()
        .create(
            NewSlackConnection::new(
                "ag_4kP9sT2vXq7LmN3a",
                "Support bot",
                SlackCredentials::new("xoxb-token-value", "0123456789abcdef0123456789abcdef"),
            )
            .channel_ids(["C0123ABC"]),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "platform": "slack",
            "agentId": "ag_4kP9sT2vXq7LmN3a",
            "name": "Support bot",
            "configuration": { "channelIds": ["C0123ABC"] },
            "credentials": {
                "botToken": "xoxb-token-value",
                "signingSecret": "0123456789abcdef0123456789abcdef"
            }
        })
    );
}

#[tokio::test]
async fn get_decodes_an_unknown_platform_and_extra_fields() {
    let (server, client) = mock().await;
    let configuration = json!({ "platform": "discord", "guildIds": ["1"], "extra": { "a": 1 } });
    let mut body = chat_connection("cc_1");
    body["platform"] = json!("discord");
    body["configuration"] = configuration.clone();
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let connection = client.chat_connections().get("cc_1").await.unwrap();

    assert_eq!(connection.platform.as_str(), "discord");
    assert!(matches!(connection.platform, ChatPlatform::Unrecognized(_)));
    let ChatConfiguration::Unrecognized(raw) = &connection.configuration else {
        panic!(
            "expected an unrecognized configuration, got {:?}",
            connection.configuration
        );
    };
    assert_eq!(*raw, configuration);
    assert_eq!(
        serde_json::to_value(&connection.configuration).unwrap(),
        configuration
    );
}

#[tokio::test]
async fn a_slack_configuration_round_trips() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;

    let connection = client.chat_connections().get("cc_1").await.unwrap();

    assert_eq!(
        serde_json::to_value(&connection.configuration).unwrap(),
        json!({ "platform": "slack", "channelIds": ["C0123ABC"] })
    );
}

#[tokio::test]
async fn a_malformed_known_platform_fails_to_decode() {
    let (server, client) = mock().await;
    let mut body = chat_connection("cc_1");
    body["configuration"] = json!({ "platform": "slack", "channelIds": "not-a-list" });
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client.chat_connections().get("cc_1").await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::Decode {
                operation_id: "getChatConnection",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn update_sends_only_the_set_fields() {
    let (server, client) = mock().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;

    client
        .chat_connections()
        .update("cc_1")
        .name("Renamed")
        .await
        .unwrap();
    client
        .chat_connections()
        .update("cc_1")
        .channel_ids(["C0123ABC", "C9876ZYX"])
        .await
        .unwrap();
    client
        .chat_connections()
        .update("cc_1")
        .chat_ids(["-1001234"])
        .business_mode(true)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Renamed" }));
    assert_eq!(
        json_body(&sent[1]),
        json!({ "configuration": { "channelIds": ["C0123ABC", "C9876ZYX"] } })
    );
    assert_eq!(
        json_body(&sent[2]),
        json!({ "configuration": { "chatIds": ["-1001234"], "businessMode": true } })
    );
}

#[tokio::test]
async fn delete_sends_delete_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.chat_connections().delete("cc_1").await.unwrap();
}

#[tokio::test]
async fn enable_disable_and_health_post_to_their_paths() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/enable"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/disable"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/health"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;

    client.chat_connections().enable("cc_1").await.unwrap();
    client.chat_connections().disable("cc_1").await.unwrap();
    let checked = client
        .chat_connections()
        .check_health("cc_1")
        .await
        .unwrap();

    assert_eq!(checked.health.checks.len(), 2);
}

#[tokio::test]
async fn rotate_credentials_sends_the_platform_tagged_body() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/credentials"))
        .respond_with(json(200, chat_connection("cc_1")))
        .mount(&server)
        .await;

    client
        .chat_connections()
        .rotate_credentials(
            "cc_1",
            ChatCredentials::slack("xoxb-new-token", "fedcba9876543210fedcba9876543210"),
        )
        .await
        .unwrap();
    client
        .chat_connections()
        .rotate_credentials("cc_1", ChatCredentials::telegram("7123456789:AAFrotated"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "platform": "slack",
            "botToken": "xoxb-new-token",
            "signingSecret": "fedcba9876543210fedcba9876543210"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({ "platform": "telegram", "botToken": "7123456789:AAFrotated" })
    );
}

#[tokio::test]
async fn deliveries_sends_the_limit_and_decodes() {
    let (server, client) = mock().await;
    let mut body = delivery("cd_1");
    body["status"] = json!("status_added_next_year");
    body["addedNextYear"] = json!(true);
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1/deliveries"))
        .and(query_param("limit", "10"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(200, json!({ "data": [body], "nextCursor": null })))
        .mount(&server)
        .await;

    let page = client
        .chat_connections()
        .deliveries("cc_1")
        .limit(10)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.next_cursor, None);
    let delivery = &page.data[0];
    assert_eq!(delivery.id, ChatDeliveryId::from("cd_1"));
    assert!(matches!(
        delivery.status,
        ChatDeliveryStatus::Unrecognized(_)
    ));
    assert_eq!(delivery.status.as_str(), "status_added_next_year");
    assert_eq!(delivery.diagnostic.as_deref(), Some("channel_not_found"));
    assert_eq!(delivery.receipts, None);
    assert_eq!(delivery.approval_id, None);
}

#[tokio::test]
async fn delivery_items_walk_every_page() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1/deliveries"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [delivery("cd_1"), delivery("cd_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1/deliveries"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [delivery("cd_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.chat_connections().deliveries("cc_1").items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|delivery| delivery.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["cd_1", "cd_2", "cd_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn repair_delivery_sends_the_confirmation_flags() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/deliveries/cd_1/repair"))
        .respond_with(json(
            200,
            json!({
                "id": "cd_1",
                "sessionId": "ss_9Pm2Vb6HxR4kTq8W",
                "threadId": "1720684800.000100",
                "connectionId": "cc_1",
                "kind": "reply",
                "status": "confirmed",
                "attempt": 3
            }),
        ))
        .mount(&server)
        .await;

    let result = client
        .chat_connections()
        .repair_delivery("cc_1", "cd_1", 2)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "expectedAttempt": 2,
            "previousSenderStopped": true,
            "acceptDuplicateRisk": true
        })
    );
    assert_eq!(result.status, ChatDeliveryStatus::Confirmed);
    assert_eq!(result.attempt, 3);
    assert_eq!(result.connection_id, ChatConnectionId::from("cc_1"));
}

#[tokio::test]
async fn tenant_deliveries_send_status_since_and_limit() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-deliveries"))
        .and(query_param("status", "failed,ambiguous"))
        .and(query_param("since", "2026-09-20T00:00:00Z"))
        .and(query_param("limit", "25"))
        .respond_with(json(
            200,
            json!({
                "data": [{
                    "id": "cd_1",
                    "kind": "card",
                    "status": "ambiguous",
                    "attempt": 1,
                    "credentialVersion": 3,
                    "representation": "Approve deployment?",
                    "diagnostic": null,
                    "receipts": { "ts": "1720684800.000100" },
                    "sessionId": "ss_9Pm2Vb6HxR4kTq8W",
                    "messageId": null,
                    "approvalId": "ap_3Nk8QwE2sLx5Rm7T",
                    "threadId": "1720684800.000100",
                    "createdAt": "2026-09-20T10:00:00.000Z",
                    "updatedAt": "2026-09-20T10:05:00.000Z",
                    "connectionId": "cc_1",
                    "agentId": "ag_4kP9sT2vXq7LmN3a",
                    "platform": "slack",
                    "addedNextYear": 42
                }],
                "nextCursor": null
            }),
        ))
        .mount(&server)
        .await;

    let page = client
        .chat_deliveries()
        .list()
        .status([
            ChatDeliveryListStatus::Failed,
            ChatDeliveryListStatus::Ambiguous,
        ])
        .since("2026-09-20T00:00:00Z".parse().unwrap())
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    let delivery = &page.data[0];
    assert_eq!(delivery.status, ChatDeliveryStatus::Ambiguous);
    assert_eq!(delivery.connection_id, ChatConnectionId::from("cc_1"));
    assert_eq!(delivery.platform, ChatPlatform::Slack);
    assert_eq!(
        delivery.receipts,
        Some(json!({ "ts": "1720684800.000100" }))
    );
    assert!(delivery.approval_id.is_some());
}

#[tokio::test]
async fn tenant_deliveries_omit_an_empty_status_filter() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-deliveries"))
        .and(query_param_is_missing("status"))
        .and(query_param_is_missing("since"))
        .and(query_param_is_missing("limit"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(200, json!({ "data": [], "nextCursor": null })))
        .mount(&server)
        .await;

    client
        .chat_deliveries()
        .list()
        .status(std::iter::empty::<ChatDeliveryListStatus>())
        .await
        .unwrap();
}

#[tokio::test]
async fn tenant_delivery_items_walk_every_page() {
    let (server, client) = mock().await;
    let tenant_delivery = |id: &str| {
        let mut value = delivery(id);
        value["connectionId"] = json!("cc_1");
        value["agentId"] = json!("ag_4kP9sT2vXq7LmN3a");
        value["platform"] = json!("telegram");
        value
    };
    Mock::given(method("GET"))
        .and(path("/v1/chat-deliveries"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [tenant_delivery("cd_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/chat-deliveries"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [tenant_delivery("cd_2")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let ids: Vec<String> = client
        .chat_deliveries()
        .list()
        .items()
        .map_ok(|delivery| delivery.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["cd_1", "cd_2"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn submitted_credentials_are_redacted_from_errors() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections"))
        .respond_with(api_error(
            400,
            "validation_failed",
            "Token xoxb-leaked-token rejected.",
        ))
        .mount(&server)
        .await;

    let error = client
        .chat_connections()
        .create(NewChatConnection::slack(
            "ag_4kP9sT2vXq7LmN3a",
            "Support bot",
            SlackCredentials::new("xoxb-leaked-token", "0123456789abcdef0123456789abcdef"),
        ))
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Token [REDACTED] rejected.");
    assert!(!api.body().contains("xoxb-leaked-token"), "{}", api.body());
}

#[tokio::test]
async fn rotated_credentials_are_redacted_from_errors() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/credentials"))
        .respond_with(api_error(
            400,
            "validation_failed",
            "Secret 0123456789abcdef0123456789abcdef rejected.",
        ))
        .mount(&server)
        .await;

    let error = client
        .chat_connections()
        .rotate_credentials(
            "cc_1",
            ChatCredentials::slack("xoxb-token", "0123456789abcdef0123456789abcdef"),
        )
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Secret [REDACTED] rejected.");
    assert!(
        !api.body().contains("0123456789abcdef0123456789abcdef"),
        "{}",
        api.body()
    );
}

#[tokio::test]
async fn a_response_that_echoes_credentials_fails_the_call() {
    let (server, client) = mock().await;
    let mut body = chat_connection("cc_1");
    body["botToken"] = json!("xoxb-leaked");
    Mock::given(method("GET"))
        .and(path("/v1/chat-connections/cc_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client.chat_connections().get("cc_1").await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "getChatConnection",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_create_response_that_echoes_the_token_fails_the_call() {
    let (server, client) = mock().await;
    let mut body = chat_connection("cc_1");
    body["name"] = json!("xoxb-echoed-secret");
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections"))
        .respond_with(json(201, body))
        .mount(&server)
        .await;

    let error = client
        .chat_connections()
        .create(NewChatConnection::slack(
            "ag_4kP9sT2vXq7LmN3a",
            "Support bot",
            SlackCredentials::new("xoxb-echoed-secret", "0123456789abcdef0123456789abcdef"),
        ))
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "createChatConnection",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_rotate_response_that_echoes_the_token_fails_the_call() {
    let (server, client) = mock().await;
    let mut body = chat_connection("cc_1");
    body["name"] = json!("xoxb-rotated-secret");
    Mock::given(method("POST"))
        .and(path("/v1/chat-connections/cc_1/credentials"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client
        .chat_connections()
        .rotate_credentials(
            "cc_1",
            ChatCredentials::slack("xoxb-rotated-secret", "fedcba9876543210fedcba9876543210"),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "rotateChatCredentials",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn credentials_and_calls_do_not_leak_secrets_in_debug() {
    let (_server, client) = mock().await;
    let credentials =
        SlackCredentials::new("xoxb-secret-token", "abcdef0123456789abcdef0123456789");
    assert!(!format!("{credentials:?}").contains("xoxb-secret-token"));
    assert!(!format!("{credentials:?}").contains("abcdef0123456789"));

    let telegram = TelegramCredentials::new("7123456789:AAFsecret");
    assert!(!format!("{telegram:?}").contains("AAFsecret"));

    let chat_credentials = ChatCredentials::slack("xoxb-secret-token", "secret-signing");
    assert!(!format!("{chat_credentials:?}").contains("xoxb-secret-token"));
    assert!(!format!("{chat_credentials:?}").contains("secret-signing"));

    let new_connection = NewChatConnection::slack(
        "ag_4kP9sT2vXq7LmN3a",
        "Support bot",
        SlackCredentials::new("xoxb-secret-token", "secret-signing"),
    );
    assert!(!format!("{new_connection:?}").contains("xoxb-secret-token"));
    assert!(!format!("{new_connection:?}").contains("secret-signing"));

    let new_telegram = NewTelegramConnection::new(
        "ag_4kP9sT2vXq7LmN3a",
        "Business bot",
        TelegramCredentials::new("7123456789:AAFsecret"),
    );
    assert!(!format!("{new_telegram:?}").contains("AAFsecret"));

    // A Call's Debug shows only the route, never the request body.
    let create = client.chat_connections().create(new_connection);
    assert!(!format!("{create:?}").contains("xoxb-secret-token"));
    assert!(!format!("{create:?}").contains("secret-signing"));

    let rotate = client
        .chat_connections()
        .rotate_credentials("cc_1", ChatCredentials::telegram("7123456789:AAFsecret"));
    assert!(!format!("{rotate:?}").contains("AAFsecret"));
}
