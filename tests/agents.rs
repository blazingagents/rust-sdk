mod support;

use blazing_agents::agents::{
    AgentConfig, AgentStatus, AgentTool, ApprovalDecision, ApprovalPolicy, ToolReference,
    WorkspaceTier,
};
use blazing_agents::error::ErrorCode;
use blazing_agents::files::Upload;
use blazing_agents::ids::{AgentId, McpConnectionId, ProviderId, WorkspaceId};
use blazing_agents::types::Metadata;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn agent(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "name": "Support writer",
        "model": "openai/gpt-6-luna",
        "thinkingLevel": "high",
        "providerId": "prv_7Tn4Kd9QwE2sLx5R",
        "workspaceId": "ws_3Jv8Km2Np6Qr4Xt7",
        "autoCompaction": true,
        "compactionReserveTokens": 16384,
        "memoryInjectionEnabled": false,
        "tools": ["read", "grep", "web_search"],
        "approvalInChat": {
            "default": "full",
            "overrides": [
                { "tool": { "type": "builtin", "name": "bash" }, "decision": "manual" }
            ]
        },
        "approvalInTasks": { "default": "deny", "overrides": [] },
        "instructions": "Answer clearly and briefly.",
        "userId": "customer_123",
        "metadata": { "team": "support" },
        "mcpConnectionIds": ["mcp_2Qw8Er5Ty9Ui3Ok6"],
        "avatarUrl": "https://files.example.com/agents/ag_1/avatar.png?sig=abc",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "status": "active"
    })
}

#[tokio::test]
async fn create_sends_every_field_and_decodes_the_agent() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents"))
        .respond_with(json(201, agent("ag_4kP9sT2vXq7LmN3a")))
        .mount(&server)
        .await;

    let created = client
        .agents()
        .create("Support writer")
        .model("prv_7Tn4Kd9QwE2sLx5R", "openai/gpt-6-luna")
        .thinking_level("high")
        .workspace_tier(WorkspaceTier::Plus)
        .workspace_id("ws_3Jv8Km2Np6Qr4Xt7")
        .auto_compaction(false)
        .compaction_reserve_tokens(8192)
        .memory_injection_enabled(true)
        .tools([AgentTool::Read, AgentTool::Grep, AgentTool::WebSearch])
        .approval_in_chat(
            ApprovalPolicy::new(ApprovalDecision::Manual)
                .with_override(
                    ToolReference::builtin(AgentTool::Read),
                    ApprovalDecision::Full,
                )
                .with_override(
                    ToolReference::mcp("mcp_2Qw8Er5Ty9Ui3Ok6", "search"),
                    ApprovalDecision::Auto,
                ),
        )
        .approval_in_tasks(ApprovalPolicy::new(ApprovalDecision::Deny))
        .instructions("Answer clearly and briefly.")
        .user_id("customer_123")
        .metadata(Metadata::new().insert("team", "support"))
        .mcp_connection_ids(["mcp_2Qw8Er5Ty9Ui3Ok6"])
        .await
        .unwrap();
    client.agents().create("Bare agent").await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "name": "Support writer",
            "providerId": "prv_7Tn4Kd9QwE2sLx5R",
            "model": "openai/gpt-6-luna",
            "thinkingLevel": "high",
            "workspaceId": "ws_3Jv8Km2Np6Qr4Xt7",
            "autoCompaction": false,
            "compactionReserveTokens": 8192,
            "memoryInjectionEnabled": true,
            "tools": ["read", "grep", "web_search"],
            "approvalInChat": {
                "default": "manual",
                "overrides": [
                    { "tool": { "type": "builtin", "name": "read" }, "decision": "full" },
                    {
                        "tool": { "type": "mcp", "connectionId": "mcp_2Qw8Er5Ty9Ui3Ok6", "name": "search" },
                        "decision": "auto"
                    }
                ]
            },
            "approvalInTasks": { "default": "deny", "overrides": [] },
            "instructions": "Answer clearly and briefly.",
            "userId": "customer_123",
            "metadata": { "team": "support" },
            "mcpConnectionIds": ["mcp_2Qw8Er5Ty9Ui3Ok6"]
        })
    );
    assert_eq!(json_body(&sent[1]), json!({ "name": "Bare agent" }));
    assert_eq!(created.id, AgentId::from("ag_4kP9sT2vXq7LmN3a"));
    assert_eq!(created.tenant_id.as_str(), "ten_8Hq2Zr5WcY1bJt6D");
    assert_eq!(created.name, "Support writer");
    assert_eq!(created.model.as_deref(), Some("openai/gpt-6-luna"));
    assert_eq!(created.thinking_level.as_deref(), Some("high"));
    assert_eq!(
        created.provider_id,
        Some(ProviderId::from("prv_7Tn4Kd9QwE2sLx5R"))
    );
    assert_eq!(
        created.workspace_id,
        WorkspaceId::from("ws_3Jv8Km2Np6Qr4Xt7")
    );
    assert!(created.auto_compaction);
    assert_eq!(created.compaction_reserve_tokens, 16384);
    assert!(!created.memory_injection_enabled);
    assert_eq!(
        created.tools,
        [AgentTool::Read, AgentTool::Grep, AgentTool::WebSearch]
    );
    assert_eq!(created.approval_in_chat.default, ApprovalDecision::Full);
    assert_eq!(created.approval_in_chat.overrides.len(), 1);
    assert_eq!(created.approval_in_tasks.default, ApprovalDecision::Deny);
    assert_eq!(created.instructions, "Answer clearly and briefly.");
    assert_eq!(created.user_id, "customer_123");
    assert_eq!(created.metadata.get("team"), Some(&json!("support")));
    assert_eq!(
        created.mcp_connection_ids,
        [McpConnectionId::from("mcp_2Qw8Er5Ty9Ui3Ok6")]
    );
    assert_eq!(
        created.avatar_url.as_deref(),
        Some("https://files.example.com/agents/ag_1/avatar.png?sig=abc")
    );
    assert_eq!(created.created_at.to_string(), "2026-07-10T10:00:00Z");
    assert!(created.updated_at > created.created_at);
    assert_eq!(created.status, AgentStatus::Active);
}

#[tokio::test]
async fn list_sends_filters_as_query_parameters() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents"))
        .and(query_param("userId", ""))
        .and(query_param("workspaceId", "ws_3Jv8Km2Np6Qr4Xt7"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [agent("ag_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .agents()
        .list()
        .user_id("")
        .workspace_id("ws_3Jv8Km2Np6Qr4Xt7")
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
async fn items_walks_every_page_lazily() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [agent("ag_1"), agent("ag_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agents"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [agent("ag_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let items = client.agents().list().limit(2).items();
    assert!(requests(&server).await.is_empty());
    let ids: Vec<String> = items
        .map_ok(|agent| agent.id.to_string())
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["ag_1", "ag_2", "ag_3"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2&limit=2"));
}

#[tokio::test]
async fn get_decodes_unknown_values_and_defaults_missing_policies() {
    let (server, client) = mock().await;
    let mut body = agent("ag_1");
    body["addedNextYear"] = json!({ "nested": true });
    body["status"] = json!("archived");
    body["tools"] = json!(["read", "quantum_leap"]);
    body["approvalInTasks"] = json!({
        "default": "judge",
        "overrides": [
            { "tool": { "type": "builtin", "name": "fly" }, "decision": "auto" },
            { "tool": { "type": "function", "name": "lookup" }, "decision": "deny" }
        ]
    });
    let object = body.as_object_mut().unwrap();
    object.remove("approvalInChat");
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1"))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.agents().get("ag_1").await.unwrap();

    match fetched.status {
        AgentStatus::Unrecognized(status) => assert_eq!(status.as_str(), "archived"),
        other => panic!("expected an unrecognized status, got {other:?}"),
    }
    assert_eq!(
        fetched.tools,
        [AgentTool::Read, AgentTool::from("quantum_leap")]
    );
    assert_eq!(fetched.approval_in_chat.default, ApprovalDecision::Full);
    assert!(fetched.approval_in_chat.overrides.is_empty());
    match fetched.approval_in_tasks.default {
        ApprovalDecision::Unrecognized(decision) => assert_eq!(decision.as_str(), "judge"),
        other => panic!("expected an unrecognized decision, got {other:?}"),
    }
    let overrides = &fetched.approval_in_tasks.overrides;
    assert_eq!(overrides.len(), 2);
    match &overrides[0].tool {
        ToolReference::Builtin { name, .. } => {
            assert_eq!(*name, AgentTool::from("fly"));
        }
        other => panic!("expected a builtin tool reference, got {other:?}"),
    }
    match &overrides[1].tool {
        ToolReference::Unrecognized(value) => {
            assert_eq!(*value, json!({ "type": "function", "name": "lookup" }))
        }
        other => panic!("expected an unrecognized tool reference, got {other:?}"),
    }
}

#[tokio::test]
async fn update_omits_unset_fields_and_sends_null_to_clear() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1"))
        .respond_with(json(200, agent("ag_1")))
        .mount(&server)
        .await;

    client
        .agents()
        .update("ag_1")
        .name("Renamed")
        .await
        .unwrap();
    client.agents().update("ag_1").clear_model().await.unwrap();
    client
        .agents()
        .update("ag_1")
        .model("prv_9Cd3Fg6Hj1Kl5Mn8", "anthropic/claude-sonnet-7")
        .await
        .unwrap();
    client
        .agents()
        .update("ag_1")
        .model_on_current_provider("openai/gpt-6-luna")
        .await
        .unwrap();
    client
        .agents()
        .update("ag_1")
        .clear_thinking_level()
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "name": "Renamed" }));
    assert_eq!(
        json_body(&sent[1]),
        json!({ "providerId": null, "model": null, "thinkingLevel": null })
    );
    assert_eq!(
        json_body(&sent[2]),
        json!({ "providerId": "prv_9Cd3Fg6Hj1Kl5Mn8", "model": "anthropic/claude-sonnet-7" })
    );
    assert_eq!(json_body(&sent[3]), json!({ "model": "openai/gpt-6-luna" }));
    assert_eq!(json_body(&sent[4]), json!({ "thinkingLevel": null }));
}

#[tokio::test]
async fn update_resends_an_unrecognized_tool_reference_unchanged() {
    let (server, client) = mock().await;
    let mut fetched = agent("ag_1");
    fetched["approvalInChat"] = json!({
        "default": "auto",
        "overrides": [
            { "tool": { "type": "function", "name": "lookup" }, "decision": "auto" }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1"))
        .respond_with(json(200, fetched))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1"))
        .respond_with(json(200, agent("ag_1")))
        .mount(&server)
        .await;

    let agent = client.agents().get("ag_1").await.unwrap();
    client
        .agents()
        .update("ag_1")
        .approval_in_chat(agent.approval_in_chat)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "approvalInChat": {
                "default": "auto",
                "overrides": [
                    { "tool": { "type": "function", "name": "lookup" }, "decision": "auto" }
                ]
            }
        })
    );
}

#[tokio::test]
async fn delete_sends_include_artifacts_and_returns_unit() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/ag_1"))
        .and(query_param("includeArtifacts", "true"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/ag_1"))
        .and(query_param("includeArtifacts", "false"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client.agents().delete("ag_1", true).await.unwrap();
    client.agents().delete("ag_1", false).await.unwrap();
}

#[tokio::test]
async fn disable_and_enable_post_to_their_paths() {
    let (server, client) = mock().await;
    let mut disabled = agent("ag_1");
    disabled["status"] = json!("disabled");
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/disable"))
        .respond_with(json(200, disabled))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/enable"))
        .respond_with(json(200, agent("ag_1")))
        .mount(&server)
        .await;

    let stopped = client.agents().disable("ag_1").await.unwrap();
    let restarted = client.agents().enable("ag_1").await.unwrap();

    assert_eq!(stopped.status, AgentStatus::Disabled);
    assert_eq!(restarted.status, AgentStatus::Active);
    let sent = requests(&server).await;
    assert!(sent[0].body.is_empty());
    assert!(sent[1].body.is_empty());
}

#[tokio::test]
async fn upload_avatar_sends_a_multipart_file() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, agent("ag_1")))
        .mount(&server)
        .await;

    client
        .agents()
        .upload_avatar(
            "ag_1",
            Upload::bytes(b"png-bytes".as_slice())
                .file_name("avatar.png")
                .content_type("image/png"),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert!(
        sent[0]
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(
        multipart.contains("name=\"file\"; filename=\"avatar.png\""),
        "{multipart}"
    );
    assert!(multipart.contains("Content-Type: image/png"), "{multipart}");
    assert!(multipart.contains("png-bytes"), "{multipart}");
}

#[tokio::test]
async fn remove_avatar_deletes_and_decodes_a_null_avatar_url() {
    let (server, client) = mock().await;
    let mut removed = agent("ag_1");
    removed["avatarUrl"] = json!(null);
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, removed))
        .mount(&server)
        .await;

    let agent = client.agents().remove_avatar("ag_1").await.unwrap();

    assert_eq!(agent.avatar_url, None);
}

#[tokio::test]
async fn list_and_update_mcp_attachments() {
    let (server, client) = mock().await;
    let attachment = json!({
        "mcpConnectionId": "mcp_2Qw8Er5Ty9Ui3Ok6",
        "forwardUserId": true,
        "forwardedMetadataKeys": ["plan"],
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    });
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/mcp-attachments"))
        .respond_with(json(200, json!({ "mcpAttachments": [attachment.clone()] })))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/v1/agents/ag_1/mcp-attachments/mcp_2Qw8Er5Ty9Ui3Ok6"))
        .respond_with(json(200, attachment))
        .mount(&server)
        .await;

    let list = client.agents().list_mcp_attachments("ag_1").await.unwrap();
    client
        .agents()
        .update_mcp_attachment("ag_1", "mcp_2Qw8Er5Ty9Ui3Ok6")
        .await
        .unwrap();
    let updated = client
        .agents()
        .update_mcp_attachment("ag_1", "mcp_2Qw8Er5Ty9Ui3Ok6")
        .forward_user_id(true)
        .forwarded_metadata_keys(["plan", "tier"])
        .await
        .unwrap();

    assert_eq!(list.mcp_attachments.len(), 1);
    let first = &list.mcp_attachments[0];
    assert_eq!(
        first.mcp_connection_id,
        McpConnectionId::from("mcp_2Qw8Er5Ty9Ui3Ok6")
    );
    assert!(first.forward_user_id);
    assert_eq!(first.forwarded_metadata_keys, ["plan"]);
    assert_eq!(updated.forwarded_metadata_keys, ["plan"]);
    let sent = requests(&server).await;
    assert_eq!(sent[1].method.as_str(), "PATCH");
    assert_eq!(json_body(&sent[1]), json!({}));
    assert_eq!(
        json_body(&sent[2]),
        json!({ "forwardUserId": true, "forwardedMetadataKeys": ["plan", "tier"] })
    );
}

#[tokio::test]
async fn spending_limits_get_and_clear() {
    let (server, client) = mock().await;
    let status = json!({
        "spendingLimit": {
            "amountUsd": 25.0,
            "resetStartDate": "2026-10-01",
            "resetInterval": "monthly"
        },
        "period": {
            "startsAt": "2026-10-01T00:00:00.000Z",
            "endsAt": "2026-11-01T00:00:00.000Z",
            "spentUsd": 4.12,
            "reservedUsd": 0.55,
            "availableUsd": 20.33
        },
        "nextResetAt": "2026-11-01T00:00:00.000Z"
    });
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/spending-limit"))
        .respond_with(json(200, status.clone()))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1/spending-limit"))
        .respond_with(json(200, status))
        .mount(&server)
        .await;

    client.agents().get_spending_limit("ag_1").await.unwrap();
    client
        .agents()
        .update_spending_limit("ag_1", None)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[1]), json!({ "spendingLimit": null }));
}

#[tokio::test]
async fn agent_errors_carry_the_envelope() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_missing"))
        .respond_with(support::api_error(404, "not_found", "Agent not found."))
        .mount(&server)
        .await;

    let error = client.agents().get("ag_missing").await.unwrap_err();

    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), Some(&ErrorCode::NotFound));
}

#[test]
fn agent_config_decodes_a_saved_snapshot() {
    let config: AgentConfig = serde_json::from_value(json!({
        "name": "Support writer",
        "model": "openai/gpt-6-luna",
        "thinkingLevel": "high",
        "providerId": "prv_7Tn4Kd9QwE2sLx5R",
        "autoCompaction": true,
        "compactionReserveTokens": 16384,
        "memoryInjectionEnabled": false,
        "tools": ["read", "web_search"],
        "approvalInChat": {
            "default": "manual",
            "overrides": [
                { "tool": { "type": "builtin", "name": "bash" }, "decision": "deny" },
                {
                    "tool": { "type": "mcp", "connectionId": "mcp_2Qw8Er5Ty9Ui3Ok6", "name": "search" },
                    "decision": "auto"
                }
            ]
        },
        "instructions": "Answer clearly and briefly.",
        "metadata": { "team": "support" },
        "mcpConnectionIds": ["mcp_2Qw8Er5Ty9Ui3Ok6"],
        "addedNextYear": { "nested": true }
    }))
    .unwrap();

    assert_eq!(config.name, "Support writer");
    assert_eq!(config.model.as_deref(), Some("openai/gpt-6-luna"));
    assert_eq!(config.thinking_level.as_deref(), Some("high"));
    assert_eq!(
        config.provider_id,
        Some(ProviderId::from("prv_7Tn4Kd9QwE2sLx5R"))
    );
    assert!(config.auto_compaction);
    assert_eq!(config.compaction_reserve_tokens, 16384);
    assert!(!config.memory_injection_enabled);
    assert_eq!(config.tools, [AgentTool::Read, AgentTool::WebSearch]);
    assert_eq!(config.approval_in_chat.default, ApprovalDecision::Manual);
    assert_eq!(config.approval_in_chat.overrides.len(), 2);
    match &config.approval_in_chat.overrides[0].tool {
        ToolReference::Builtin { name, .. } => assert_eq!(*name, AgentTool::Bash),
        other => panic!("expected a builtin tool reference, got {other:?}"),
    }
    assert_eq!(
        config.approval_in_chat.overrides[0].decision,
        ApprovalDecision::Deny
    );
    match &config.approval_in_chat.overrides[1].tool {
        ToolReference::Mcp {
            connection_id,
            name,
            ..
        } => {
            assert_eq!(
                *connection_id,
                McpConnectionId::from("mcp_2Qw8Er5Ty9Ui3Ok6")
            );
            assert_eq!(name, "search");
        }
        other => panic!("expected an mcp tool reference, got {other:?}"),
    }
    assert_eq!(
        config.approval_in_chat.overrides[1].decision,
        ApprovalDecision::Auto
    );
    assert_eq!(config.approval_in_tasks.default, ApprovalDecision::Full);
    assert!(config.approval_in_tasks.overrides.is_empty());
    assert_eq!(config.instructions, "Answer clearly and briefly.");
    assert_eq!(config.metadata.get("team"), Some(&json!("support")));
    assert_eq!(
        config.mcp_connection_ids,
        [McpConnectionId::from("mcp_2Qw8Er5Ty9Ui3Ok6")]
    );
}
