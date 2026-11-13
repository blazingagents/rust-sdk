mod support;

use std::io::{Read, Write};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::routing::post;
use blazing_agents::error::StreamErrorKind;
use blazing_agents::functions::Functions;
use blazing_agents::relay::{ChatRelay, RelayBody, RelayContext};
use blazing_agents::types::Metadata;
use blazing_agents::{Client, Error};
use bytes::Bytes;
use http::{HeaderMap, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use serde_json::{Value, json};
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const AGENT: &str = "ag_0123456789abcdef";
const SESSION: &str = "ss_0123456789abcdef";
const TURN: &str = "turn_0123456789abcdef";
const SCOPE: &str = "x-ba-user-id";

const EVENTS: &str = "data: {\"type\":\"start\"}\n\ndata: {\"type\":\"finish\"}\n\n";

fn user_message() -> Value {
    json!({ "id": "m-1", "role": "user", "parts": [{ "type": "text", "text": "Hello" }] })
}

fn signed_in(client: Client) -> ChatRelay {
    ChatRelay::new(client, |_request| async {
        Ok(Some(
            RelayContext::new(AGENT, "user-a").metadata(Metadata::new().insert("plan", "demo")),
        ))
    })
}

fn nobody(client: Client) -> ChatRelay {
    ChatRelay::new(client, |_request| async { Ok(None) })
}

fn lookup_order() -> Functions {
    Functions::builder()
        .function_json(
            "lookup_order",
            "Looks up an order.",
            json!({ "type": "object" }),
            |_input, _context| async { Ok::<_, std::convert::Infallible>(json!({})) },
        )
        .build()
        .unwrap()
}

fn lookup_order_descriptor() -> Value {
    json!({ "lookup_order": { "description": "Looks up an order.", "inputSchema": { "type": "object" } } })
}

fn post_json(body: &Value) -> Request<Full<Bytes>> {
    post_bytes(Bytes::from(body.to_string()))
}

fn post_bytes(body: Bytes) -> Request<Full<Bytes>> {
    Request::post("/api/chat")
        .header("content-type", "application/json")
        .body(Full::new(body))
        .unwrap()
}

async fn read(response: Response<RelayBody>) -> (StatusCode, HeaderMap, Bytes) {
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    (parts.status, parts.headers, bytes)
}

/// Sends `body` through the relay and reads the whole answer as JSON.
async fn ask(relay: &ChatRelay, body: Value) -> (StatusCode, HeaderMap, Value) {
    let (status, headers, bytes) = read(relay.handle(post_json(&body)).await).await;
    (status, headers, serde_json::from_slice(&bytes).unwrap())
}

fn events(status: u16) -> ResponseTemplate {
    ResponseTemplate::new(status)
        .set_body_raw(EVENTS.as_bytes().to_vec(), "text/event-stream")
        .insert_header("x-ba-turn-id", TURN)
        .insert_header("x-request-id", "req-chat")
}

fn new_session() -> ResponseTemplate {
    events(201).insert_header(
        "location",
        format!("/v1/agents/{AGENT}/sessions/{SESSION}").as_str(),
    )
}

async fn mount(server: &wiremock::MockServer, verb: &str, route: String, reply: ResponseTemplate) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(reply)
        .mount(server)
        .await;
}

fn sessions_path() -> String {
    format!("/v1/agents/{AGENT}/sessions")
}

fn session_path(tail: &str) -> String {
    format!("/v1/agents/{AGENT}/sessions/{SESSION}{tail}")
}

fn error_code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap()
}

#[tokio::test]
async fn a_chat_without_a_session_starts_one_as_the_signed_in_user() {
    let (server, client) = mock().await;
    mount(&server, "POST", sessions_path(), new_session()).await;
    let relay = signed_in(client);

    let response = relay
        .handle(post_json(&json!({
            "op": "chat",
            "message": user_message(),
            "trigger": "regenerate-message",
            "agentId": "ag_attacker0000000",
            "userId": "attacker"
        })))
        .await;
    let (status, headers, body) = read(response).await;

    assert_eq!(status, 201);
    assert_eq!(body, EVENTS.as_bytes());
    assert_eq!(headers["content-type"], "text/event-stream");
    assert_eq!(headers["cache-control"], "no-cache");
    assert_eq!(headers["x-vercel-ai-ui-message-stream"], "v1");
    assert_eq!(headers["x-accel-buffering"], "no");
    assert_eq!(
        headers["location"],
        format!("/v1/agents/{AGENT}/sessions/{SESSION}").as_str()
    );
    assert_eq!(headers["x-ba-turn-id"], TURN);
    assert_eq!(headers["x-request-id"], "req-chat");
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].url.path(), sessions_path());
    assert_eq!(sent[0].headers.get(SCOPE).unwrap(), "user-a");
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "messages": [user_message()],
            "metadata": { "plan": "demo" },
            "trigger": "submit-message"
        })
    );
}

#[tokio::test]
async fn a_chat_with_a_session_continues_it_and_a_regeneration_cuts_back_to_its_message() {
    let (server, client) = mock().await;
    mount(&server, "POST", session_path(""), events(200)).await;
    let relay = signed_in(client);

    let resumed = relay
        .handle(post_json(&json!({
            "op": "chat", "message": user_message(), "sessionId": SESSION
        })))
        .await;
    read(resumed).await;
    let regenerated = relay
        .handle(post_json(&json!({
            "op": "chat",
            "message": user_message(),
            "sessionId": SESSION,
            "trigger": "regenerate-message",
            "messageId": "assistant-1"
        })))
        .await;
    let (status, headers, _) = read(regenerated).await;

    assert_eq!(status, 200);
    assert!(headers.get("location").is_none());
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    for request in &sent {
        assert_eq!(request.url.path(), session_path(""));
        assert_eq!(request.headers.get(SCOPE).unwrap(), "user-a");
    }
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "messages": [user_message()],
            "metadata": { "plan": "demo" },
            "trigger": "submit-message"
        })
    );
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "messages": [user_message()],
            "messageId": "assistant-1",
            "metadata": { "plan": "demo" },
            "trigger": "regenerate-message"
        })
    );
}

#[tokio::test]
async fn the_context_functions_ride_along_on_every_turn_request() {
    let (server, client) = mock().await;
    mount(&server, "POST", sessions_path(), new_session()).await;
    mount(&server, "POST", session_path(""), events(200)).await;
    let relay = ChatRelay::new(client, |_request| async {
        Ok(Some(
            RelayContext::new(AGENT, "user-a").functions(lookup_order()),
        ))
    });

    for body in [
        json!({ "op": "chat", "message": user_message() }),
        json!({ "op": "chat", "message": user_message(), "sessionId": SESSION }),
        json!({
            "op": "chat", "message": user_message(), "sessionId": SESSION,
            "trigger": "regenerate-message"
        }),
    ] {
        let (status, _, _) = read(relay.handle(post_json(&body)).await).await;
        assert!(status.is_success());
    }

    for request in requests(&server).await {
        assert_eq!(json_body(&request)["functions"], lookup_order_descriptor());
        assert!(json_body(&request).get("metadata").is_none());
    }
}

fn approval_message() -> Value {
    json!({
        "id": "assistant-1",
        "role": "assistant",
        "parts": [
            {
                "type": "tool-bash", "toolCallId": "c0", "state": "approval-responded",
                "input": {}, "approval": { "id": "apr_stale", "approved": true }
            },
            { "type": "step-start" },
            {
                "type": "dynamic-tool", "toolName": "lookup_order", "toolCallId": "c1",
                "state": "approval-responded", "input": {},
                "approval": { "id": "apr_1", "approved": true }
            },
            {
                "type": "tool-bash", "toolCallId": "c2", "state": "approval-responded",
                "input": {}, "approval": { "id": "apr_2", "approved": false, "reason": "No" }
            },
            {
                "type": "tool-bash", "toolCallId": "c3", "state": "approval-requested",
                "input": {}, "approval": { "id": "apr_3" }
            },
            { "type": "text", "text": "Waiting.", "state": "approval-responded" }
        ]
    })
}

fn approval_decisions() -> Value {
    json!([
        { "approvalId": "apr_1", "approved": true },
        { "approvalId": "apr_2", "approved": false, "reason": "No" }
    ])
}

#[tokio::test]
async fn an_approval_round_continues_the_turn_with_the_last_steps_decisions() {
    let (server, client) = mock().await;
    mount(
        &server,
        "POST",
        session_path("/tool-approvals/continue"),
        events(200),
    )
    .await;
    let relay = signed_in(client);

    let response = relay
        .handle(post_json(&json!({
            "op": "chat", "message": approval_message(), "sessionId": SESSION
        })))
        .await;
    let (status, headers, body) = read(response).await;

    assert_eq!(status, 200);
    assert_eq!(body, EVENTS.as_bytes());
    assert_eq!(headers["x-ba-turn-id"], TURN);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].headers.get(SCOPE).unwrap(), "user-a");
    assert_eq!(
        json_body(&sent[0]),
        json!({ "decisions": approval_decisions(), "functions": {} })
    );
}

#[tokio::test]
async fn an_approval_round_continues_with_the_context_functions() {
    let (server, client) = mock().await;
    mount(
        &server,
        "POST",
        session_path("/tool-approvals/continue"),
        events(200),
    )
    .await;
    let relay = ChatRelay::new(client, |_request| async {
        Ok(Some(
            RelayContext::new(AGENT, "user-a").functions(lookup_order()),
        ))
    });

    let response = relay
        .handle(post_json(&json!({
            "op": "chat", "message": approval_message(), "sessionId": SESSION
        })))
        .await;
    read(response).await;

    assert_eq!(
        json_body(&requests(&server).await[0]),
        json!({ "decisions": approval_decisions(), "functions": lookup_order_descriptor() })
    );
}

fn stored(id: &str, role: &str) -> Value {
    json!({
        "id": id, "role": role, "branchable": role == "assistant",
        "parts": [{ "type": "text", "text": "Stored." }],
        "createdNextYear": { "kept": true }
    })
}

#[tokio::test]
async fn history_forwards_one_page_of_200_and_drops_the_latest_cursor() {
    let (server, client) = mock().await;
    mount(
        &server,
        "GET",
        session_path("/messages"),
        json(
            200,
            json!({
                "data": [stored("m-1", "user"), stored("m-2", "assistant")],
                "nextCursor": "older",
                "latestCursor": "latest"
            }),
        ),
    )
    .await;
    let relay = signed_in(client);

    let (status, headers, newest) = ask(
        &relay,
        json!({ "op": "history", "sessionId": SESSION, "userId": "attacker" }),
    )
    .await;
    ask(
        &relay,
        json!({ "op": "history", "sessionId": SESSION, "cursor": "older" }),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "application/json");
    assert_eq!(newest["nextCursor"], "older");
    assert!(newest.get("latestCursor").is_none());
    assert_eq!(newest["data"][0]["id"], "m-1");
    assert_eq!(newest["data"][1]["branchable"], true);
    assert_eq!(
        newest["data"][1]["createdNextYear"],
        json!({ "kept": true })
    );
    let sent = requests(&server).await;
    assert_eq!(sent[0].url.query(), Some("limit=200"));
    assert_eq!(sent[1].url.query(), Some("cursor=older&limit=200"));
    for request in &sent {
        assert_eq!(request.headers.get(SCOPE).unwrap(), "user-a");
    }
}

#[tokio::test]
async fn the_last_history_page_has_no_next_cursor() {
    let (server, client) = mock().await;
    mount(
        &server,
        "GET",
        session_path("/messages"),
        json(
            200,
            json!({ "data": [], "nextCursor": null, "latestCursor": null }),
        ),
    )
    .await;

    let (_, _, page) = ask(
        &signed_in(client),
        json!({ "op": "history", "sessionId": SESSION }),
    )
    .await;

    assert_eq!(page, json!({ "data": [], "nextCursor": null }));
}

fn receipt(state: &str) -> Value {
    json!({
        "data": {
            "requestId": "steer-1",
            "sequence": 1,
            "message": user_message(),
            "state": state,
            "turnId": TURN,
            "createdAt": "2026-11-01T10:00:00Z",
            "updatedAt": "2026-11-01T10:00:00Z",
            "reason": null
        },
        "activity": { "state": "running", "turnId": TURN }
    })
}

#[tokio::test]
async fn a_steer_answers_with_the_receipt_state() {
    let (server, client) = mock().await;
    mount(
        &server,
        "POST",
        session_path("/inputs"),
        json(202, receipt("committed")),
    )
    .await;
    let relay = signed_in(client);

    let (status, _, body) = ask(
        &relay,
        json!({
            "op": "steer", "sessionId": SESSION, "requestId": "steer-1", "message": user_message()
        }),
    )
    .await;

    assert_eq!(status, 202);
    assert_eq!(body, json!({ "state": "committed" }));
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].headers.get(SCOPE).unwrap(), "user-a");
    assert_eq!(
        json_body(&sent[0]),
        json!({ "requestId": "steer-1", "message": user_message() })
    );
}

fn every_op() -> [Value; 4] {
    [
        json!({ "op": "chat", "message": user_message(), "sessionId": SESSION }),
        json!({ "op": "chat", "message": approval_message(), "sessionId": SESSION }),
        json!({ "op": "history", "sessionId": SESSION }),
        json!({
            "op": "steer", "sessionId": SESSION, "requestId": "steer-1", "message": user_message()
        }),
    ]
}

#[tokio::test]
async fn the_apis_refusals_keep_their_status_code_message_and_request_id() {
    for (status, code) in [
        (404, "not_found"),
        (409, "steer_not_available"),
        (409, "input_idempotency_conflict"),
    ] {
        let (server, client) = mock().await;
        Mock::given(wiremock::matchers::any())
            .respond_with(
                api_error(status, code, "Refused by the API.")
                    .insert_header("x-request-id", "req-api"),
            )
            .mount(&server)
            .await;
        let relay = signed_in(client);

        for body in every_op() {
            let (got, headers, answer) = ask(&relay, body).await;

            assert_eq!(got, status);
            assert_eq!(headers["x-request-id"], "req-api");
            assert_eq!(
                answer,
                json!({ "error": { "code": code, "message": "Refused by the API." } })
            );
        }
        for request in requests(&server).await {
            assert_eq!(request.headers.get(SCOPE).unwrap(), "user-a");
        }
    }
}

#[tokio::test]
async fn a_request_that_breaks_the_contract_is_refused_before_any_api_call() {
    let (server, client) = mock().await;
    let relay = signed_in(client);
    let long_request_id = "r".repeat(129);
    let user = user_message();
    let cases = [
        (
            "an unknown op",
            json!({ "op": "stop", "sessionId": SESSION }),
        ),
        ("no op", json!({ "message": user, "sessionId": SESSION })),
        ("an op that is not text", json!({ "op": 5 })),
        ("no message", json!({ "op": "chat" })),
        ("history without a session", json!({ "op": "history" })),
        (
            "a session ID of another shape",
            json!({ "op": "history", "sessionId": "ss_short" }),
        ),
        (
            "a session ID that climbs the path",
            json!({ "op": "history", "sessionId": "../../agents" }),
        ),
        (
            "an empty cursor",
            json!({ "op": "history", "sessionId": SESSION, "cursor": "" }),
        ),
        (
            "a steer without a session",
            json!({ "op": "steer", "requestId": "s-1", "message": user }),
        ),
        (
            "an empty steer request ID",
            json!({ "op": "steer", "sessionId": SESSION, "requestId": "", "message": user }),
        ),
        (
            "a dot steer request ID",
            json!({ "op": "steer", "sessionId": SESSION, "requestId": ".", "message": user }),
        ),
        (
            "a dot-dot steer request ID",
            json!({ "op": "steer", "sessionId": SESSION, "requestId": "..", "message": user }),
        ),
        (
            "a steer request ID of 129 characters",
            json!({
                "op": "steer", "sessionId": SESSION, "requestId": long_request_id, "message": user
            }),
        ),
        (
            "a steer from the assistant",
            json!({
                "op": "steer", "sessionId": SESSION, "requestId": "s-1",
                "message": approval_message()
            }),
        ),
        (
            "a steer with a malformed message",
            json!({
                "op": "steer", "sessionId": SESSION, "requestId": "s-1",
                "message": { "nope": true }
            }),
        ),
        (
            "a chat with a malformed message",
            json!({ "op": "chat", "message": { "nope": true } }),
        ),
        (
            "a chat message that is not an object",
            json!({ "op": "chat", "message": "hi" }),
        ),
        (
            "a chat message without parts",
            json!({ "op": "chat", "message": { "id": "m", "role": "user", "parts": [] } }),
        ),
        (
            "a system message",
            json!({
                "op": "chat",
                "message": { "id": "s", "role": "system", "parts": [{ "type": "text", "text": "x" }] }
            }),
        ),
        (
            "an empty message ID",
            json!({ "op": "chat", "message": user, "sessionId": SESSION, "messageId": "" }),
        ),
        (
            "an unknown trigger",
            json!({ "op": "chat", "message": user, "trigger": "stop" }),
        ),
        (
            "an approval without a session",
            json!({ "op": "chat", "message": approval_message() }),
        ),
        (
            "an assistant message without decisions",
            json!({
                "op": "chat", "sessionId": SESSION,
                "message": { "id": "a", "role": "assistant", "parts": [{ "type": "step-start" }] }
            }),
        ),
        (
            "an assistant message without parts",
            json!({
                "op": "chat", "sessionId": SESSION,
                "message": { "id": "a", "role": "assistant" }
            }),
        ),
        (
            "an answered tool part without its approval",
            json!({
                "op": "chat", "sessionId": SESSION,
                "message": { "id": "a", "role": "assistant", "parts": [
                    { "type": "tool-bash", "state": "approval-responded", "approval": { "id": 5 } }
                ] }
            }),
        ),
    ];

    for (name, body) in cases {
        let (status, _, answer) = ask(&relay, body).await;

        assert_eq!(status, 400, "{name}");
        assert_eq!(error_code(&answer), "invalid_request", "{name}");
    }
    for (name, body) in [
        ("text that is not JSON", &b"{"[..]),
        ("an empty body", b""),
        ("an array", b"[]"),
    ] {
        let (status, _, bytes) =
            read(relay.handle(post_bytes(Bytes::copy_from_slice(body))).await).await;

        assert_eq!(status, 400, "{name}");
        let answer: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error_code(&answer), "invalid_request", "{name}");
    }
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn a_body_over_the_apis_limit_is_refused_unread() {
    let (server, client) = mock().await;
    let relay = signed_in(client);

    let oversized = Bytes::from(vec![b' '; 15 * 1024 * 1024 + 1]);
    let (status, _, bytes) = read(relay.handle(post_bytes(oversized)).await).await;

    assert_eq!(status, 400);
    let answer: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(error_code(&answer), "invalid_request");
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn nobody_signed_in_is_unauthorized_before_the_body_is_read() {
    let (server, client) = mock().await;
    let relay = nobody(client);

    let (status, headers, answer) =
        ask(&relay, json!({ "op": "history", "sessionId": SESSION })).await;
    let garbage = read(relay.handle(post_bytes(Bytes::from_static(b"{"))).await).await;

    assert_eq!(status, 401);
    assert_eq!(headers["content-type"], "application/json");
    assert_eq!(
        answer,
        json!({ "error": { "code": "unauthorized", "message": "Authentication required." } })
    );
    assert_eq!(garbage.0, 401);
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn the_resolver_decides_the_user_and_agent_from_the_request_it_is_given() {
    let (server, client) = mock().await;
    mount(
        &server,
        "GET",
        "/v1/agents/ag_fedcba9876543210/sessions/ss_0123456789abcdef/messages".to_owned(),
        json(
            200,
            json!({ "data": [], "nextCursor": null, "latestCursor": null }),
        ),
    )
    .await;
    let relay = ChatRelay::new(client, |request| async move {
        let user = request
            .headers()
            .get("x-test-user")
            .unwrap()
            .to_str()?
            .to_owned();
        let body: Value = serde_json::from_slice(request.body())?;
        let agent = body["workspace"].as_str().unwrap().to_owned();
        assert_eq!(request.method(), http::Method::POST);
        assert_eq!(request.uri().path(), "/api/chat");
        Ok(Some(RelayContext::new(agent, user)))
    });

    let request = Request::post("/api/chat")
        .header("x-test-user", "user-b")
        .body(Full::new(Bytes::from(
            json!({ "op": "history", "sessionId": SESSION, "workspace": "ag_fedcba9876543210" })
                .to_string(),
        )))
        .unwrap();
    let (status, _, _) = read(relay.handle(request).await).await;

    assert_eq!(status, 200);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].headers.get(SCOPE).unwrap(), "user-b");
}

#[tokio::test]
async fn a_resolver_that_fails_is_an_internal_error_that_hides_its_cause() {
    let (server, client) = mock().await;
    let relay = ChatRelay::new(client, |_request| async {
        Err("database password is hunter2".into())
    });

    let (status, _, bytes) = read(
        relay
            .handle(post_json(&json!({ "op": "history", "sessionId": SESSION })))
            .await,
    )
    .await;

    assert_eq!(status, 500);
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("hunter2"), "{text}");
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        answer,
        json!({ "error": { "code": "internal_error", "message": "Request failed." } })
    );
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn a_context_the_sdk_rejects_is_an_internal_error_and_sends_nothing() {
    let (server, client) = mock().await;
    let body = json!({ "op": "history", "sessionId": SESSION });
    for (agent, user) in [(AGENT, " padded "), ("", "user-a"), ("..", "user-a")] {
        let relay = ChatRelay::new(client.clone(), move |_request| async move {
            Ok(Some(RelayContext::new(agent, user)))
        });

        let (status, _, bytes) = read(relay.handle(post_json(&body)).await).await;

        assert_eq!(status, 500, "{agent:?} {user:?}");
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!text.contains("padded"), "{text}");
        assert!(text.contains("internal_error"), "{text}");
    }
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn an_unreachable_api_is_a_bad_gateway() {
    let client = Client::builder()
        .api_key("test-key")
        .base_url("http://127.0.0.1:1")
        .build()
        .unwrap();
    let relay = signed_in(client);

    for body in every_op() {
        let (status, _, answer) = ask(&relay, body).await;

        assert_eq!(status, 502);
        assert_eq!(error_code(&answer), "network_error");
    }
}

#[tokio::test]
async fn an_answer_the_sdk_cannot_read_is_a_bad_gateway_with_its_request_id() {
    let (server, client) = mock().await;
    mount(
        &server,
        "GET",
        session_path("/messages"),
        json(200, json!({ "data": "not a list" })).insert_header("x-request-id", "req-odd"),
    )
    .await;

    let (status, headers, answer) = ask(
        &signed_in(client),
        json!({ "op": "history", "sessionId": SESSION }),
    )
    .await;

    assert_eq!(status, 502);
    assert_eq!(headers["x-request-id"], "req-odd");
    assert_eq!(error_code(&answer), "invalid_response");
}

#[tokio::test]
async fn a_new_session_without_its_location_is_a_stream_error_that_relays_no_events() {
    let (server, client) = mock().await;
    let function_call = json!({
        "type": "data-ba-function-call",
        "data": { "id": "fc_0123456789abcdef", "name": "lookup_order", "input": {} },
        "transient": true
    });
    let reply = ResponseTemplate::new(201)
        .set_body_raw(
            format!("data: {function_call}\n\n{EVENTS}").into_bytes(),
            "text/event-stream",
        )
        .insert_header("x-ba-turn-id", TURN)
        .insert_header("x-request-id", "req-chat");
    mount(&server, "POST", sessions_path(), reply).await;
    let relay = ChatRelay::new(client, |_request| async {
        Ok(Some(
            RelayContext::new(AGENT, "user-a").functions(lookup_order()),
        ))
    });

    let (status, headers, bytes) = read(
        relay
            .handle(post_json(
                &json!({ "op": "chat", "message": user_message() }),
            ))
            .await,
    )
    .await;

    assert_eq!(status, 502);
    assert_eq!(headers["x-request-id"], "req-chat");
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains("data-ba-function-call"), "{text}");
    assert!(text.contains("stream_error"), "{text}");
}

#[tokio::test]
async fn a_stream_that_fails_after_its_headers_fails_the_body() {
    let (server, client) = mock().await;
    let reply = ResponseTemplate::new(200)
        .set_body_raw(
            b"data: {\"type\":\"start\"}\n\ndata: {\"type\":\"te".to_vec(),
            "text/event-stream",
        )
        .insert_header("x-ba-turn-id", TURN);
    mount(&server, "POST", session_path(""), reply).await;
    let relay = signed_in(client);

    let response = relay
        .handle(post_json(&json!({
            "op": "chat", "message": user_message(), "sessionId": SESSION
        })))
        .await;
    let (parts, mut body) = response.into_parts();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    let failure = body.frame().await.unwrap().unwrap_err();

    assert_eq!(parts.status, 200);
    assert_eq!(first, "data: {\"type\":\"start\"}\n\n".as_bytes());
    assert!(
        matches!(&failure, Error::Stream(error) if error.kind() == StreamErrorKind::Incomplete),
        "{failure:?}"
    );
}

#[tokio::test]
async fn debug_output_names_the_relay_and_never_the_api_key() {
    let (_server, client) = mock().await;

    let debug = format!("{:?}", signed_in(client));

    assert!(debug.starts_with("ChatRelay"), "{debug}");
    assert!(!debug.contains("test-key"), "{debug}");
}

async fn chat_handler(
    State(relay): State<ChatRelay>,
    request: axum::extract::Request,
) -> Response<RelayBody> {
    relay.handle(request).await
}

/// Serves the relay with axum, which needs no glue beyond this handler.
async fn serve(relay: ChatRelay) -> String {
    let app = Router::new()
        .route("/api/chat", post(chat_handler))
        .with_state(relay);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/api/chat")
}

/// What an API connection did, as the fake API saw it.
#[derive(Debug, PartialEq)]
enum Seen {
    Request,
    Closed,
}

/// An API that reads one request, sends `reply` if it has one, and then
/// reports each thing it sees until the peer closes the connection.
fn api(reply: Option<String>) -> (String, tokio::sync::mpsc::UnboundedReceiver<Seen>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (seen, events) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        let mut buffer = [0u8; 4096];
        while !String::from_utf8_lossy(&received).contains("\r\n\r\n") {
            let read = socket.read(&mut buffer).unwrap();
            received.extend_from_slice(&buffer[..read]);
        }
        seen.send(Seen::Request).unwrap();
        if let Some(reply) = reply {
            socket.write_all(reply.as_bytes()).unwrap();
            socket.flush().unwrap();
        }
        while matches!(socket.read(&mut buffer), Ok(read) if read > 0) {}
        seen.send(Seen::Closed).unwrap();
    });
    (format!("http://{address}"), events)
}

async fn next(events: &mut tokio::sync::mpsc::UnboundedReceiver<Seen>) -> Seen {
    tokio::time::timeout(Duration::from_secs(10), events.recv())
        .await
        .expect("the fake API saw nothing in time")
        .expect("the fake API stopped")
}

fn relay_for(base_url: String) -> ChatRelay {
    let client = Client::builder()
        .api_key("test-key")
        .base_url(base_url)
        .build()
        .unwrap();
    signed_in(client)
}

fn browser() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn chat_body() -> String {
    json!({ "op": "chat", "message": user_message(), "sessionId": SESSION }).to_string()
}

#[tokio::test]
async fn a_browser_that_disconnects_mid_stream_closes_the_api_connection() {
    let (base_url, mut seen) = api(Some(format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nx-ba-turn-id: {TURN}\r\nContent-Length: 100000\r\n\r\ndata: {{\"type\":\"start\"}}\n\n"
    )));
    let url = serve(relay_for(base_url)).await;

    let mut response = browser().post(url).body(chat_body()).send().await.unwrap();
    let first = response.chunk().await.unwrap().unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(first, "data: {\"type\":\"start\"}\n\n".as_bytes());
    assert_eq!(next(&mut seen).await, Seen::Request);
    assert!(
        seen.try_recv().is_err(),
        "the API connection stays open while the browser reads"
    );
    drop(response);
    assert_eq!(next(&mut seen).await, Seen::Closed);
}

#[tokio::test]
async fn a_browser_that_disconnects_before_the_api_answers_closes_the_api_connection() {
    let (base_url, mut seen) = api(None);
    let url = serve(relay_for(base_url)).await;

    let pending = tokio::spawn(browser().post(url).body(chat_body()).send());
    assert_eq!(next(&mut seen).await, Seen::Request);
    pending.abort();

    assert_eq!(next(&mut seen).await, Seen::Closed);
}

#[tokio::test]
async fn a_json_answer_declares_its_exact_length() {
    let (_server, client) = mock().await;
    let relay = nobody(client);

    let direct = relay.handle(post_json(&json!({ "op": "history" }))).await;
    let declared = http_body::Body::size_hint(direct.body()).exact().unwrap();
    let served = browser()
        .post(serve(relay).await)
        .body(chat_body())
        .send()
        .await
        .unwrap();

    assert_eq!(served.status(), 401);
    assert_eq!(served.content_length(), Some(declared));
    assert_eq!(served.bytes().await.unwrap().len() as u64, declared);
}
