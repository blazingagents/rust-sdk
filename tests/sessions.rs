mod support;

use blazing_agents::Error;
use blazing_agents::agents::{AgentStatus, AgentTool, ApprovalDecision, ToolReference};
use blazing_agents::ids::TurnId;
use blazing_agents::messages::{UserMessage, UserPart};
use blazing_agents::pagination::Cursor;
use blazing_agents::sessions::{ActivityState, ContinuationState, InputState, ToolApprovalStatus};
use blazing_agents::transcript::MessageRole;
use futures_util::TryStreamExt;
use http::{HeaderName, HeaderValue};
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

const AGENT: &str = "ag_4kP9sT2vXq7LmN3a";
const SESSION: &str = "ss_6Rt2Mw8KqZ4Nc1Hp";
const TURN: &str = "turn_3Vx9KpL2QwR8TmN5";

fn session(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "messageCount": 4,
        "lastMessagePreview": "Open Settings, choose Security, and select Reset password.",
        "userId": "customer_123",
        "metadata": { "plan": "pro" },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:05:00.000Z"
    })
}

fn agent_config() -> serde_json::Value {
    json!({
        "approvalInChat": {
            "default": "manual",
            "overrides": [
                { "tool": { "type": "builtin", "name": "bash" }, "decision": "deny" },
                {
                    "tool": {
                        "type": "mcp",
                        "connectionId": "mcp_1Aa2Bb3Cc4Dd5Ee6",
                        "name": "lookup_order"
                    },
                    "decision": "auto"
                }
            ]
        },
        "approvalInTasks": { "default": "full", "overrides": [] },
        "name": "Support Agent",
        "model": "openai/gpt-6-luna",
        "thinkingLevel": null,
        "providerId": "prv_7Tn4Kd9QwE2sLx5R",
        "autoCompaction": true,
        "compactionReserveTokens": 16384,
        "memoryInjectionEnabled": false,
        "tools": ["read", "grep", "glob", "write_todos"],
        "instructions": "Answer billing questions clearly and briefly.",
        "metadata": { "team": "support" },
        "mcpConnectionIds": ["mcp_1Aa2Bb3Cc4Dd5Ee6"]
    })
}

fn ui_message(id: &str, role: &str) -> serde_json::Value {
    json!({
        "id": id,
        "role": role,
        "branchable": role == "assistant",
        "parts": [{"type": "text", "text": "A stored reply."}]
    })
}

fn session_input(sequence: u64) -> serde_json::Value {
    json!({
        "requestId": "req-1",
        "sequence": sequence,
        "message": {
            "id": "m-1",
            "role": "user",
            "parts": [{"type": "text", "text": "Please also compare costs."}]
        },
        "state": "accepted",
        "turnId": TURN,
        "createdAt": "2026-07-10T10:01:00.000Z",
        "updatedAt": "2026-07-10T10:01:00.000Z",
        "reason": null
    })
}

#[tokio::test]
async fn list_sends_filters_and_decodes_compact_items() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/sessions")))
        .and(query_param("userId", "customer_123"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [session("ss_1"), session("ss_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;

    let page = client
        .sessions()
        .list(AGENT)
        .user_id("customer_123")
        .limit(25)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 2);
    assert_eq!(page.data[0].id.as_str(), "ss_1");
    assert_eq!(page.data[0].message_count, 4);
    assert_eq!(
        page.data[0].last_message_preview.as_deref(),
        Some("Open Settings, choose Security, and select Reset password.")
    );
    assert_eq!(page.data[0].user_id, "customer_123");
    assert_eq!(page.data[0].created_at.to_string(), "2026-07-10T10:00:00Z");
    assert_eq!(page.next_cursor.as_ref().map(Cursor::as_str), Some("c2"));
}

#[tokio::test]
async fn list_sends_no_params_and_empty_user_id_is_sent() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/sessions")))
        .respond_with(json(200, json!({ "data": [], "nextCursor": null })))
        .mount(&server)
        .await;

    client.sessions().list(AGENT).await.unwrap();
    client.sessions().list(AGENT).user_id("").await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), None);
    assert_eq!(sent[1].url.query(), Some("userId="));
}

#[tokio::test]
async fn list_latest_sends_by_agent_and_decodes_status() {
    let (server, client) = mock().await;
    let mut latest = session("ss_9");
    latest["agentId"] = json!(AGENT);
    latest["model"] = json!("openai/gpt-6-luna");
    latest["thinkingLevel"] = json!(null);
    latest["status"] = json!("active");
    let mut archived = latest.clone();
    archived["id"] = json!("ss_8");
    archived["status"] = json!("archived");
    Mock::given(method("GET"))
        .and(path("/v1/sessions/latest"))
        .and(query_param("byAgent", "true"))
        .and(query_param("userId", "customer_123"))
        .and(query_param("limit", "10"))
        .respond_with(json(
            200,
            json!({ "data": [latest, archived], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let page = client
        .sessions()
        .list_latest()
        .user_id("customer_123")
        .by_agent(true)
        .limit(10)
        .await
        .unwrap();

    assert_eq!(page.data[0].agent_id.as_str(), AGENT);
    assert_eq!(page.data[0].status, AgentStatus::Active);
    match &page.data[1].status {
        AgentStatus::Unrecognized(value) => assert_eq!(value.as_str(), "archived"),
        other => panic!("expected an unrecognized status, got {other:?}"),
    }
    assert_eq!(page.data[1].status.as_str(), "archived");
}

#[tokio::test]
async fn get_decodes_agent_config_and_provenance() {
    let (server, client) = mock().await;
    let mut body = session(SESSION);
    body["agentConfig"] = agent_config();
    body["forkedFrom"] = json!({
        "sessionId": "ss_2Wq8TyHn5KbVz6Lx",
        "messageId": "m-42"
    });
    body["addedNextYear"] = json!({ "nested": true });
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}")))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let fetched = client.sessions().get(AGENT, SESSION).await.unwrap();

    let config = &fetched.agent_config;
    assert_eq!(config.name, "Support Agent");
    assert_eq!(config.model.as_deref(), Some("openai/gpt-6-luna"));
    assert_eq!(
        config.tools,
        [
            AgentTool::Read,
            AgentTool::Grep,
            AgentTool::Glob,
            AgentTool::WriteTodos
        ]
    );
    assert_eq!(config.approval_in_chat.default, ApprovalDecision::Manual);
    assert_eq!(fetched.id.as_str(), SESSION);
    assert_eq!(fetched.message_count, 4);
    assert_eq!(fetched.user_id, "customer_123");
    assert_eq!(fetched.metadata.get("plan"), Some(&json!("pro")));

    let forked_from = fetched.forked_from.expect("a forked session");
    assert_eq!(forked_from.session_id.as_str(), "ss_2Wq8TyHn5KbVz6Lx");
    assert_eq!(forked_from.message_id, "m-42");
}

#[tokio::test]
async fn get_fails_without_agent_config() {
    let (server, client) = mock().await;
    let mut body = session(SESSION);
    body["forkedFrom"] = json!(null);
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}")))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    let error = client.sessions().get(AGENT, SESSION).await.unwrap_err();

    assert!(matches!(error, Error::Decode { .. }));
}

#[tokio::test]
async fn fork_posts_the_message_and_key() {
    let (server, client) = mock().await;
    let mut created = session("ss_3New4Child5XyZaB");
    created["agentConfig"] = agent_config();
    created["forkedFrom"] = json!(null);
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}/fork")))
        .respond_with(json(201, created))
        .mount(&server)
        .await;

    let child = client
        .sessions()
        .fork(AGENT, SESSION, "m-42", "fork-key-1")
        .await
        .unwrap();
    client
        .sessions()
        .fork(AGENT, SESSION, "m-42", "fork-key-2")
        .with_header(
            HeaderName::from_static("idempotency-key"),
            HeaderValue::from_static("other-key"),
        )
        .await
        .unwrap();

    assert_eq!(child.agent_config.name, "Support Agent");
    assert!(child.forked_from.is_none());

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0]), json!({ "messageId": "m-42" }));
    assert_eq!(
        sent[0]
            .headers
            .get("idempotency-key")
            .map(|v| v.to_str().unwrap()),
        Some("fork-key-1")
    );
    // The positional key wins over `with_header`.
    let all = sent[1].headers.get_all("idempotency-key");
    let values: Vec<&str> = all.iter().map(|v| v.to_str().unwrap()).collect();
    assert_eq!(values, ["fork-key-2"]);
}

#[tokio::test]
async fn fork_rejects_an_invalid_key_without_a_request() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}/fork")))
        .respond_with(json(200, session("ss_1")))
        .expect(0)
        .mount(&server)
        .await;

    let error = client
        .sessions()
        .fork(AGENT, SESSION, "m-42", "key\n")
        .await
        .unwrap_err();
    let empty = client
        .sessions()
        .fork(AGENT, SESSION, "m-42", "")
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidArgument(_)));
    assert!(matches!(empty, Error::InvalidArgument(_)));
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn fork_replays_decode_the_same_shape() {
    let (server, client) = mock().await;
    let mut replayed = session("ss_3New4Child5XyZaB");
    replayed["agentConfig"] = agent_config();
    replayed["forkedFrom"] = json!(null);
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}/fork")))
        .respond_with(json(200, replayed))
        .mount(&server)
        .await;

    let replay = client
        .sessions()
        .fork(AGENT, SESSION, "m-42", "fork-key")
        .await
        .unwrap();

    assert_eq!(replay.id.as_str(), "ss_3New4Child5XyZaB");
}

#[tokio::test]
async fn messages_reads_the_newest_page() {
    let (server, client) = mock().await;
    let mut system = ui_message("m-1", "system");
    system["parts"] = json!([{"type": "step-start"}]);
    let mut unknown = ui_message("m-3", "tool");
    unknown["metadata"] = json!({ "trace": "t-1" });
    unknown["addedNextYear"] = json!(1);
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .and(query_param_is_missing("cursor"))
        .and(query_param_is_missing("after"))
        .and(query_param_is_missing("limit"))
        .respond_with(json(
            200,
            json!({
                "data": [system, ui_message("m-2", "assistant"), unknown],
                "nextCursor": "c2",
                "latestCursor": "lc1"
            }),
        ))
        .mount(&server)
        .await;

    let page = client.sessions().messages(AGENT, SESSION).await.unwrap();

    assert_eq!(page.latest_cursor.as_ref().map(Cursor::as_str), Some("lc1"));
    assert_eq!(page.next_cursor.as_ref().map(Cursor::as_str), Some("c2"));
    assert_eq!(page.data[0].role, MessageRole::System);
    assert!(!page.data[0].branchable);
    assert_eq!(page.data[0].parts[0].kind, "step-start");
    assert_eq!(page.data[0].parts[0].text(), None);
    assert!(page.data[1].branchable);
    assert_eq!(page.data[1].parts[0].text(), Some("A stored reply."));
    match &page.data[2].role {
        MessageRole::Unrecognized(role) => assert_eq!(role.as_str(), "tool"),
        other => panic!("expected an unrecognized role, got {other:?}"),
    }
    assert_eq!(page.data[2].metadata, Some(json!({ "trace": "t-1" })));
    // Unknown top-level fields land in `extra`; known fields do not.
    assert_eq!(page.data[2].extra.get("addedNextYear"), Some(&json!(1)));
    assert!(!page.data[2].extra.contains_key("id"));
    assert_eq!(
        serde_json::to_value(&page).unwrap()["latestCursor"],
        json!("lc1")
    );
}

#[tokio::test]
async fn messages_sends_cursor_after_and_limit() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .respond_with(json(
            200,
            json!({ "data": [], "nextCursor": null, "latestCursor": null }),
        ))
        .mount(&server)
        .await;

    client
        .sessions()
        .messages(AGENT, SESSION)
        .limit(2)
        .cursor(Cursor::from("c2"))
        .await
        .unwrap();
    client
        .sessions()
        .messages(AGENT, SESSION)
        .after(Cursor::from("lc1"))
        .await
        .unwrap();
    // `.cursor(_)` after `.after(_)` keeps the forward direction.
    client
        .sessions()
        .messages(AGENT, SESSION)
        .after(Cursor::from("lc1"))
        .cursor(Cursor::from("lc2"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), Some("cursor=c2&limit=2"));
    assert_eq!(sent[1].url.query(), Some("after=lc1"));
    assert_eq!(sent[2].url.query(), Some("after=lc2"));
}

#[tokio::test]
async fn messages_items_walks_backwards() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({
                "data": [ui_message("m-3", "assistant")],
                "nextCursor": "c2",
                "latestCursor": "lc1"
            }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({
                "data": [ui_message("m-2", "user"), ui_message("m-1", "system")],
                "nextCursor": null,
                "latestCursor": "lc1"
            }),
        ))
        .mount(&server)
        .await;

    let ids: Vec<String> = client
        .sessions()
        .messages(AGENT, SESSION)
        .items()
        .map_ok(|message| message.id)
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["m-3", "m-2", "m-1"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn messages_items_polls_forward() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .and(query_param("after", "lc1"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({
                "data": [ui_message("m-4", "user")],
                "nextCursor": "lc2",
                "latestCursor": "lc2"
            }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/messages"
        )))
        .and(query_param("after", "lc2"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({
                "data": [ui_message("m-5", "assistant")],
                "nextCursor": null,
                "latestCursor": "lc2"
            }),
        ))
        .mount(&server)
        .await;

    let ids: Vec<String> = client
        .sessions()
        .messages(AGENT, SESSION)
        .after(Cursor::from("lc1"))
        .items()
        .map_ok(|message| message.id)
        .try_collect()
        .await
        .unwrap();

    assert_eq!(ids, ["m-4", "m-5"]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("after=lc2"));
}

#[tokio::test]
async fn tool_approvals_decodes_the_round() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/tool-approvals"
        )))
        .respond_with(json(
            200,
            json!({
                "data": [
                    {
                        "tool": { "type": "builtin", "name": "bash" },
                        "assistantMessageId": "m-9",
                        "createdAt": "2026-07-10T10:04:00.000Z",
                        "decidedAt": null,
                        "approvalId": "apr_1",
                        "decision": "pending",
                        "input": { "command": "ls" },
                        "reason": null,
                        "toolCallId": "call_1",
                        "toolName": "bash"
                    },
                    {
                        "tool": {
                            "type": "mcp",
                            "connectionId": "mcp_1Aa2Bb3Cc4Dd5Ee6",
                            "name": "lookup_order"
                        },
                        "approvalId": "apr_2",
                        "decision": "approved",
                        "input": {},
                        "reason": "Looks right.",
                        "toolCallId": "call_2",
                        "toolName": "lookup_order"
                    },
                    {
                        "tool": { "type": "function", "name": "charge_card" },
                        "approvalId": "apr_3",
                        "decision": "denied",
                        "input": { "amount": 10 },
                        "reason": "Too risky.",
                        "toolCallId": "call_3",
                        "toolName": "charge_card"
                    },
                    {
                        "tool": null,
                        "approvalId": "apr_4",
                        "decision": "escalated",
                        "input": {},
                        "reason": null,
                        "toolCallId": "call_4",
                        "toolName": "unknown_tool"
                    },
                    {
                        "approvalId": "apr_5",
                        "decision": "pending",
                        "input": {},
                        "reason": null,
                        "toolCallId": "call_5",
                        "toolName": "web_search"
                    },
                    {
                        "tool": { "type": "skill", "name": "x" },
                        "approvalId": "apr_6",
                        "decision": "pending",
                        "input": {},
                        "reason": null,
                        "toolCallId": "call_6",
                        "toolName": "x"
                    },
                    {
                        "tool": { "type": "builtin", "name": "browse" },
                        "approvalId": "apr_7",
                        "decision": "pending",
                        "input": {},
                        "reason": null,
                        "toolCallId": "call_7",
                        "toolName": "browse"
                    }
                ],
                "continuation": { "id": "cont_1", "state": "waiting" }
            }),
        ))
        .mount(&server)
        .await;

    let approvals = client
        .sessions()
        .tool_approvals(AGENT, SESSION)
        .await
        .unwrap();

    let items = &approvals.data;
    assert_eq!(items.len(), 7);
    match &items[0].tool {
        Some(ToolReference::Builtin { name, .. }) => assert_eq!(*name, AgentTool::Bash),
        other => panic!("expected a builtin tool, got {other:?}"),
    }
    assert_eq!(items[0].decision, ToolApprovalStatus::Pending);
    assert_eq!(items[0].approval_id.as_str(), "apr_1");
    assert_eq!(items[0].tool_call_id, "call_1");
    assert_eq!(items[0].input.get("command"), Some(&json!("ls")));
    assert_eq!(items[0].reason, None);
    assert_eq!(
        items[0].created_at.map(|t| t.to_string()),
        Some("2026-07-10T10:04:00Z".to_owned())
    );
    assert_eq!(items[0].assistant_message_id.as_deref(), Some("m-9"));
    assert_eq!(items[0].decided_at, None);
    match &items[1].tool {
        Some(ToolReference::Mcp {
            connection_id,
            name,
            ..
        }) => {
            assert_eq!(connection_id.as_str(), "mcp_1Aa2Bb3Cc4Dd5Ee6");
            assert_eq!(name, "lookup_order");
        }
        other => panic!("expected an MCP tool, got {other:?}"),
    }
    assert_eq!(items[1].decision, ToolApprovalStatus::Approved);
    match &items[2].tool {
        Some(ToolReference::Function { name, .. }) => assert_eq!(name, "charge_card"),
        other => panic!("expected a function tool, got {other:?}"),
    }
    assert_eq!(items[2].decision, ToolApprovalStatus::Denied);
    assert!(items[3].tool.is_none());
    match &items[3].decision {
        ToolApprovalStatus::Unrecognized(value) => assert_eq!(value.as_str(), "escalated"),
        other => panic!("expected an unrecognized decision, got {other:?}"),
    }
    // Fields missing from the wire decode as None.
    assert!(items[4].tool.is_none());
    assert_eq!(items[4].assistant_message_id, None);
    assert_eq!(items[4].created_at, None);
    // An unknown tool kind decodes as Unrecognized with its raw object, and an
    // unknown builtin name as an unrecognized AgentTool.
    match &items[5].tool {
        Some(ToolReference::Unrecognized(value)) => {
            assert_eq!(value["type"], "skill")
        }
        other => panic!("expected an unrecognized tool, got {other:?}"),
    }
    match &items[6].tool {
        Some(ToolReference::Builtin { name, .. }) => {
            assert_eq!(*name, AgentTool::from("browse"));
            assert_eq!(name.as_str(), "browse");
        }
        other => panic!("expected a builtin tool, got {other:?}"),
    }

    let continuation = approvals.continuation.expect("an open continuation");
    assert_eq!(continuation.id, "cont_1");
    assert_eq!(continuation.state, ContinuationState::Waiting);
}

#[tokio::test]
async fn tool_approvals_decodes_no_continuation() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/tool-approvals"
        )))
        .respond_with(json(200, json!({ "data": [], "continuation": null })))
        .mount(&server)
        .await;

    let approvals = client
        .sessions()
        .tool_approvals(AGENT, SESSION)
        .await
        .unwrap();

    assert!(approvals.data.is_empty());
    assert!(approvals.continuation.is_none());
}

#[tokio::test]
async fn delete_sends_delete_artifacts() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client
        .sessions()
        .delete(AGENT, SESSION, true)
        .await
        .unwrap();
    client
        .sessions()
        .delete(AGENT, SESSION, false)
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), Some("deleteArtifacts=true"));
    assert_eq!(sent[1].url.query(), Some("deleteArtifacts=false"));
}

#[tokio::test]
async fn submit_input_posts_the_message_and_decodes_the_receipt() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/inputs"
        )))
        .respond_with(json(
            202,
            json!({
                "data": session_input(1),
                "activity": { "state": "running", "turnId": TURN }
            }),
        ))
        .mount(&server)
        .await;

    let submitted = client
        .sessions()
        .submit_input(
            AGENT,
            SESSION,
            "req-1",
            UserMessage::new("m-1", UserPart::text("Please also compare costs.")),
        )
        .await
        .unwrap();

    assert_eq!(
        json_body(&requests(&server).await[0]),
        json!({
            "requestId": "req-1",
            "message": {
                "id": "m-1",
                "role": "user",
                "parts": [{"type": "text", "text": "Please also compare costs."}]
            }
        })
    );
    assert_eq!(submitted.input.request_id, "req-1");
    assert_eq!(submitted.input.sequence, 1);
    assert_eq!(submitted.input.message.id(), "m-1");
    assert_eq!(submitted.input.state, InputState::Accepted);
    assert_eq!(submitted.input.turn_id, TurnId::from(TURN));
    assert_eq!(submitted.input.reason, None);
    assert_eq!(submitted.activity.state, ActivityState::Running);
    assert_eq!(submitted.activity.turn_id, Some(TurnId::from(TURN)));
}

#[tokio::test]
async fn submit_input_decodes_unknown_states() {
    let (server, client) = mock().await;
    let mut receipt = session_input(7);
    receipt["state"] = json!("held");
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/inputs"
        )))
        .respond_with(json(
            202,
            json!({
                "data": receipt,
                "activity": { "state": "idle", "turnId": null }
            }),
        ))
        .mount(&server)
        .await;

    let submitted = client
        .sessions()
        .submit_input(AGENT, SESSION, "req-7", UserMessage::text("Hi"))
        .await
        .unwrap();

    match &submitted.input.state {
        InputState::Unrecognized(state) => assert_eq!(state.as_str(), "held"),
        other => panic!("expected an unrecognized state, got {other:?}"),
    }
    assert_eq!(submitted.activity.state, ActivityState::Idle);
    assert_eq!(submitted.activity.turn_id, None);
}

#[tokio::test]
async fn inputs_sends_filters_and_decodes_activity() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/inputs"
        )))
        .and(query_param("includeCompleted", "true"))
        .and(query_param("limit", "5"))
        .respond_with(json(
            200,
            json!({
                "data": [session_input(1)],
                "nextCursor": null,
                "activity": { "state": "approval", "turnId": TURN }
            }),
        ))
        .mount(&server)
        .await;

    let page = client
        .sessions()
        .inputs(AGENT, SESSION)
        .include_completed(true)
        .limit(5)
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].state, InputState::Accepted);
    assert_eq!(page.activity.state, ActivityState::Approval);
    assert_eq!(page.activity.turn_id, Some(TurnId::from(TURN)));
    assert!(serde_json::to_value(&page).unwrap()["activity"].is_object());
}

#[tokio::test]
async fn inputs_items_walks_every_page() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/inputs"
        )))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({
                "data": [session_input(1)],
                "nextCursor": "c2",
                "activity": { "state": "running", "turnId": TURN }
            }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/agents/{AGENT}/sessions/{SESSION}/inputs"
        )))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({
                "data": [session_input(2)],
                "nextCursor": null,
                "activity": { "state": "idle", "turnId": null }
            }),
        ))
        .mount(&server)
        .await;

    let sequences: Vec<u64> = client
        .sessions()
        .inputs(AGENT, SESSION)
        .items()
        .map_ok(|input| input.sequence)
        .try_collect()
        .await
        .unwrap();

    assert_eq!(sequences, [1, 2]);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn stop_posts_the_turn_and_decodes_activity() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}/stop")))
        .respond_with(json(
            200,
            json!({
                "stoppedTurnId": TURN,
                "activity": { "state": "stopping", "turnId": TURN }
            }),
        ))
        .mount(&server)
        .await;

    let stopped = client.sessions().stop(AGENT, SESSION, TURN).await.unwrap();

    assert_eq!(
        json_body(&requests(&server).await[0]),
        json!({ "turnId": TURN })
    );
    assert_eq!(stopped.stopped_turn_id, TurnId::from(TURN));
    assert_eq!(stopped.activity.state, ActivityState::Stopping);
    assert_eq!(stopped.activity.turn_id, Some(TurnId::from(TURN)));
}

#[tokio::test]
async fn stop_decodes_an_unknown_activity() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}/stop")))
        .respond_with(json(
            200,
            json!({
                "stoppedTurnId": TURN,
                "activity": { "state": "paused", "turnId": null }
            }),
        ))
        .mount(&server)
        .await;

    let stopped = client.sessions().stop(AGENT, SESSION, TURN).await.unwrap();

    match &stopped.activity.state {
        ActivityState::Unrecognized(state) => assert_eq!(state.as_str(), "paused"),
        other => panic!("expected an unrecognized state, got {other:?}"),
    }
    assert_eq!(stopped.activity.turn_id, None);
}

#[tokio::test]
async fn user_scope_sends_the_user_header() {
    let (server, client) = mock().await;
    let mut body = session(SESSION);
    body["agentConfig"] = agent_config();
    body["forkedFrom"] = json!(null);
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/{AGENT}/sessions/{SESSION}")))
        .respond_with(json(200, body))
        .mount(&server)
        .await;

    client.sessions().get(AGENT, SESSION).await.unwrap();
    let user_client = client.for_user("customer_123").unwrap();
    user_client.sessions().get(AGENT, SESSION).await.unwrap();

    let sent = requests(&server).await;
    assert!(!sent[0].headers.contains_key("x-ba-user-id"));
    assert_eq!(
        sent[1]
            .headers
            .get("x-ba-user-id")
            .map(|v| v.to_str().unwrap()),
        Some("customer_123")
    );
}
