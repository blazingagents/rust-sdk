mod support;

use std::io::{Read, Write};
use std::time::Duration;

use blazing_agents::Error;
use blazing_agents::chat::{ChatInput, ToolApprovalDecision};
use blazing_agents::error::{ErrorCode, StreamErrorKind};
use blazing_agents::functions::Functions;
use blazing_agents::generation::SavedPrompt;
use blazing_agents::messages::{UserMessage, UserPart};
use blazing_agents::types::Metadata;
use futures_util::TryStreamExt;
use serde_json::json;
use support::{api_error, json, json_body, mock, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const SESSION: &str = "ss_0123456789abcdef";
const TURN: &str = "turn_0123456789abcdef";

fn sse(status: u16, events: &[&str]) -> ResponseTemplate {
    let body: String = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
    ResponseTemplate::new(status)
        .set_body_raw(body.into_bytes(), "text/event-stream")
        .insert_header("x-ba-turn-id", TURN)
}

fn lookup_order() -> Functions {
    Functions::builder()
        .function_json(
            "lookup_order",
            "Looks up an order.",
            json!({ "type": "object", "properties": { "orderId": { "type": "string" } } }),
            |input, context| async move {
                Ok::<_, std::convert::Infallible>(json!({
                    "echo": input,
                    "key": context.idempotency_key().as_str()
                }))
            },
        )
        .build()
        .unwrap()
}

fn lookup_order_descriptor() -> serde_json::Value {
    json!({ "lookup_order": {
        "description": "Looks up an order.",
        "inputSchema": { "type": "object", "properties": { "orderId": { "type": "string" } } }
    } })
}

fn new_session(events: &[&str]) -> ResponseTemplate {
    sse(201, events).insert_header(
        "location",
        format!("/v1/agents/ag_1/sessions/{SESSION}").as_str(),
    )
}

#[tokio::test]
async fn chat_starts_a_session_and_yields_each_event() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(new_session(&[
            r#"{"type":"start"}"#,
            r#"{"type":"text-delta","delta":"hi"}"#,
        ]))
        .mount(&server)
        .await;
    let message = UserMessage::new("m1", UserPart::text("Hello"));

    let stream = client.chat("ag_1", message).await.unwrap();

    assert_eq!(stream.session_id().as_str(), SESSION);
    assert_eq!(stream.turn_id().as_str(), TURN);
    assert_eq!(stream.meta().status(), 201);
    let frames: Vec<_> = stream.try_collect().await.unwrap();
    assert_eq!(
        frames,
        [
            "data: {\"type\":\"start\"}\n\n".as_bytes(),
            "data: {\"type\":\"text-delta\",\"delta\":\"hi\"}\n\n".as_bytes()
        ]
    );
    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "messages": [{ "id": "m1", "role": "user", "parts": [{ "type": "text", "text": "Hello" }] }],
            "trigger": "submit-message"
        })
    );
}

#[tokio::test]
async fn a_relayed_browser_message_keeps_every_field() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(new_session(&[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;
    let message_json = json!({
        "id": "m1",
        "role": "user",
        "parts": [
            {
                "type": "text",
                "text": "hi",
                "state": "done",
                "providerMetadata": { "openai": { "itemId": "x" } }
            },
            {
                "type": "file",
                "mediaType": "image/png",
                "url": "data:image/png;base64,AAAA",
                "filename": "a.png",
                "providerMetadata": { "anthropic": { "cache": true } }
            }
        ]
    });
    let message = serde_json::from_value::<UserMessage>(message_json.clone()).unwrap();

    let stream = client.chat("ag_1", message).await.unwrap();
    drop(stream);

    let sent = requests(&server).await;
    assert_eq!(json_body(&sent[0])["messages"][0], message_json);
}

#[tokio::test]
async fn chat_continues_an_existing_session() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/ag_1/sessions/{SESSION}")))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .expect(1)
        .mount(&server)
        .await;

    let stream = client
        .chat("ag_1", UserMessage::text("again"))
        .session(SESSION)
        .await
        .unwrap();

    assert_eq!(stream.session_id().as_str(), SESSION);
}

#[tokio::test]
async fn missing_or_malformed_handshake_headers_fail_before_the_body() {
    let (server, client) = mock().await;
    let cases = [
        (
            sse(201, &[]),
            StreamErrorKind::MissingSessionId,
            "the response has no Location header",
        ),
        (
            sse(201, &[]).insert_header("location", "/v1/agents/ag_1/sessions/nope"),
            StreamErrorKind::MalformedSessionId,
            "the Location header is not a session URL",
        ),
        (
            ResponseTemplate::new(201)
                .insert_header("location", format!("/sessions/{SESSION}").as_str()),
            StreamErrorKind::MissingTurnId,
            "the response has no X-BA-Turn-Id header",
        ),
        (
            ResponseTemplate::new(201)
                .insert_header("location", format!("/sessions/{SESSION}").as_str())
                .insert_header("x-ba-turn-id", "tr_0123456789abcdef"),
            StreamErrorKind::MalformedTurnId,
            "the X-BA-Turn-Id header is not a turn ID",
        ),
    ];
    for (response, kind, message) in cases {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(response.insert_header("x-request-id", "req_chat"))
            .mount(&server)
            .await;

        let error = client
            .chat("ag_1", UserMessage::text("hi"))
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), message);
        assert_eq!(error.status(), Some(201));
        assert_eq!(error.request_id(), Some("req_chat"));
        assert_eq!(error.code(), None);
        assert!(std::error::Error::source(&error).is_none());
        let Error::Stream(stream) = error else {
            panic!("expected a stream error, got {error:?}");
        };
        assert_eq!(stream.kind(), kind);
        assert_eq!(stream.meta().status(), 201);
        assert_eq!(stream.meta().request_id(), Some("req_chat"));
    }
}

#[tokio::test]
async fn continue_chat_sends_every_decision() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/agents/ag_1/sessions/{SESSION}/tool-approvals/continue"
        )))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;

    client
        .continue_chat(
            "ag_1",
            SESSION,
            [
                ToolApprovalDecision::approve("apr_1"),
                ToolApprovalDecision::deny("apr_2").reason("Too risky."),
            ],
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "decisions": [
                { "approvalId": "apr_1", "approved": true },
                { "approvalId": "apr_2", "approved": false, "reason": "Too risky." }
            ]
        })
    );
}

#[tokio::test]
async fn functions_are_claimed_run_and_resolved_without_reaching_the_caller() {
    let (server, client) = mock().await;
    let call_path =
        format!("/v1/agents/ag_1/sessions/{SESSION}/function-calls/fc_0123456789abcdef");
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(new_session(&[
            r#"{"type":"start"}"#,
            r#"{"type":"data-ba-function-call","data":{"id":"fc_0123456789abcdef","name":"lookup_order","input":{"orderId":"o-1"},"deadlineAt":"2099-01-01T00:00:00Z"},"transient":true}"#,
            r#"{"type":"finish"}"#,
        ]))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{call_path}/claim")))
        .respond_with(json(200, json!({ "claimed": true })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{call_path}/result")))
        .respond_with(json(200, json!({ "accepted": true })))
        .mount(&server)
        .await;
    let mut stream = client
        .chat("ag_1", UserMessage::text("Where is my order?"))
        .functions(lookup_order())
        .await
        .unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = stream.try_next().await.unwrap() {
        frames.push(String::from_utf8(frame.to_vec()).unwrap());
    }
    assert_eq!(
        frames,
        [
            "data: {\"type\":\"start\"}\n\n",
            "data: {\"type\":\"finish\"}\n\n"
        ]
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let sent = loop {
        let sent = requests(&server).await;
        if sent.len() == 3 {
            break sent;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "claim and result never arrived"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    drop(stream);
    assert_eq!(json_body(&sent[0])["functions"], lookup_order_descriptor());
    let claim = json_body(&sent[1]);
    let claim_request_id = claim["claimRequestId"].as_str().unwrap().to_owned();
    assert!(!claim_request_id.is_empty());
    assert_eq!(
        json_body(&sent[2]),
        json!({
            "claimRequestId": claim_request_id,
            "outcome": {
                "kind": "output",
                "value": { "echo": { "orderId": "o-1" }, "key": "fc_0123456789abcdef" }
            }
        })
    );
}

#[tokio::test]
async fn a_body_ending_inside_an_event_is_incomplete() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(new_session(&[r#"{"type":"start"}"#]).set_body_raw(
            b"data: {\"type\":\"start\"}\n\ndata: {\"type\":\"text-".to_vec(),
            "text/event-stream",
        ))
        .mount(&server)
        .await;

    let mut stream = client.chat("ag_1", UserMessage::text("hi")).await.unwrap();

    let first = stream.try_next().await.unwrap().unwrap();
    assert_eq!(first, "data: {\"type\":\"start\"}\n\n".as_bytes());
    let error = stream.try_next().await.unwrap_err();
    assert_eq!(error.to_string(), "the stream ended before it finished");
    assert!(std::error::Error::source(&error).is_none());
    let Error::Stream(error) = error else {
        panic!("expected a stream error, got {error:?}");
    };
    assert_eq!(error.kind(), StreamErrorKind::Incomplete);
    assert!(stream.try_next().await.unwrap().is_none());
}

#[tokio::test]
async fn events_end_at_whichever_delimiter_comes_first() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(new_session(&[]).set_body_raw(
            b"data: {\"type\":\"start\"}\r\n\r\ndata: {\"type\":\"finish\"}\n\ndata: {\"type\":\"x\"}\n\ndata: {\"type\":\"y\"}\r\n\r\n".to_vec(),
            "text/event-stream",
        ))
        .mount(&server)
        .await;

    let frames: Vec<_> = client
        .chat("ag_1", UserMessage::text("hi"))
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();

    assert_eq!(
        frames,
        [
            "data: {\"type\":\"start\"}\r\n\r\n".as_bytes(),
            "data: {\"type\":\"finish\"}\n\n".as_bytes(),
            "data: {\"type\":\"x\"}\n\n".as_bytes(),
            "data: {\"type\":\"y\"}\r\n\r\n".as_bytes(),
        ]
    );
}

#[tokio::test]
async fn trailing_whitespace_after_the_last_event_ends_the_stream_normally() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(new_session(&[]).set_body_raw(
            b"data: {\"type\":\"start\"}\n\n\r\n \n".to_vec(),
            "text/event-stream",
        ))
        .mount(&server)
        .await;

    let frames: Vec<_> = client
        .chat("ag_1", UserMessage::text("hi"))
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();

    assert_eq!(frames, ["data: {\"type\":\"start\"}\n\n".as_bytes()]);
}

#[tokio::test]
async fn a_message_batch_is_sent_in_order() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(new_session(&[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;
    let input = ChatInput::try_from(vec![
        UserMessage::new("m1", UserPart::text("a")),
        UserMessage::new("m2", UserPart::text("b")),
    ])
    .unwrap();

    client.chat("ag_1", input).await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "messages": [
                { "id": "m1", "role": "user", "parts": [{ "type": "text", "text": "a" }] },
                { "id": "m2", "role": "user", "parts": [{ "type": "text", "text": "b" }] }
            ],
            "trigger": "submit-message"
        })
    );
    let error = ChatInput::try_from(Vec::<UserMessage>::new()).unwrap_err();
    assert_eq!(error.to_string(), "a chat turn needs at least one message");
}

#[tokio::test]
async fn chat_runs_a_saved_prompt_for_an_end_user() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(new_session(&[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;

    client
        .chat("ag_1", SavedPrompt::new("prompt_1").var("audience", "devs"))
        .user_id("customer_123")
        .metadata(Metadata::new().insert("team", "support"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "promptId": "prompt_1",
            "variables": { "audience": "devs" },
            "trigger": "submit-message",
            "userId": "customer_123",
            "metadata": { "team": "support" }
        })
    );
}

#[tokio::test]
async fn regenerate_streams_in_the_existing_session() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/ag_1/sessions/{SESSION}")))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;

    let stream = client
        .regenerate(
            "ag_1",
            SESSION,
            UserMessage::new("m1", UserPart::text("again")),
        )
        .message_id("msg_5")
        .user_id("customer_123")
        .metadata(Metadata::new().insert("team", "support"))
        .await
        .unwrap();

    assert_eq!(stream.session_id().as_str(), SESSION);
    assert_eq!(stream.turn_id().as_str(), TURN);
    let debug = format!("{stream:?}");
    assert!(debug.starts_with("ChatStream {"), "{debug}");
    assert!(debug.contains(SESSION), "{debug}");
    assert!(debug.contains(TURN), "{debug}");
    assert!(debug.ends_with(".. }"), "{debug}");
    let frames: Vec<_> = stream.try_collect().await.unwrap();
    assert_eq!(frames, ["data: {\"type\":\"start\"}\n\n".as_bytes()]);
    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "messages": [
                { "id": "m1", "role": "user", "parts": [{ "type": "text", "text": "again" }] }
            ],
            "trigger": "regenerate-message",
            "messageId": "msg_5",
            "userId": "customer_123",
            "metadata": { "team": "support" }
        })
    );
}

#[tokio::test]
async fn regenerate_runs_a_saved_prompt() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/ag_1/sessions/{SESSION}")))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;

    client
        .regenerate("ag_1", SESSION, SavedPrompt::new("prompt_1"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "promptId": "prompt_1", "trigger": "regenerate-message" })
    );
}

#[tokio::test]
async fn regenerate_and_continue_chat_declare_their_functions() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/ag_1/sessions/{SESSION}")))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/v1/agents/ag_1/sessions/{SESSION}/tool-approvals/continue"
        )))
        .respond_with(sse(200, &[r#"{"type":"start"}"#]))
        .mount(&server)
        .await;

    client
        .regenerate("ag_1", SESSION, UserMessage::text("again"))
        .functions(lookup_order())
        .await
        .unwrap();
    client
        .continue_chat("ag_1", SESSION, [ToolApprovalDecision::approve("apr_1")])
        .functions(lookup_order())
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(json_body(&sent[0])["functions"], lookup_order_descriptor());
    assert_eq!(
        json_body(&sent[1]),
        json!({
            "decisions": [{ "approvalId": "apr_1", "approved": true }],
            "functions": lookup_order_descriptor()
        })
    );
}

#[tokio::test]
async fn a_rejected_turn_is_an_api_error() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/ag_1/sessions/{SESSION}")))
        .respond_with(api_error(409, "session_busy", "A turn is already running."))
        .mount(&server)
        .await;

    let error = client
        .chat("ag_1", UserMessage::text("hi"))
        .session(SESSION)
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Api(_)), "{error:?}");
    assert_eq!(error.status(), Some(409));
    assert_eq!(error.code(), Some(&ErrorCode::SessionBusy));
}

/// Answers one request with SSE headers and a CRLF-delimited event, pauses,
/// then closes with the body shorter than the declared `Content-Length`.
fn flaky_sse_server(pause: Duration) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = socket.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            if String::from_utf8_lossy(&request).contains("\r\n\r\n") {
                break;
            }
        }
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nx-ba-turn-id: {TURN}\r\nContent-Length: 1000\r\n\r\ndata: {{\"type\":\"start\"}}\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
        socket.flush().unwrap();
        std::thread::sleep(pause);
        socket.write_all(b"data: {\"type\":\"text-").unwrap();
        socket.flush().unwrap();
    });
    format!("http://{address}")
}

#[tokio::test]
async fn a_crlf_event_then_a_dropped_body_is_a_read_error() {
    let base_url = flaky_sse_server(Duration::from_millis(100));
    let client = blazing_agents::Client::builder()
        .api_key("test-key")
        .base_url(base_url)
        .build()
        .unwrap();

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .session(SESSION)
        .await
        .unwrap();

    let first = stream.try_next().await.unwrap().unwrap();
    assert_eq!(first, "data: {\"type\":\"start\"}\r\n\r\n".as_bytes());
    let error = stream.try_next().await.unwrap_err();
    assert!(
        matches!(&error, Error::Stream(e) if e.kind() == StreamErrorKind::Read),
        "{error:?}"
    );
    assert_eq!(error.to_string(), "response body read failed");
    assert!(std::error::Error::source(&error).is_some());
    assert!(stream.try_next().await.unwrap().is_none());
}
