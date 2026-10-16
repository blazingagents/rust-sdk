mod support;

use std::time::Duration;

use blazing_agents::Error;
use blazing_agents::chat::ApprovalDecision;
use blazing_agents::error::StreamErrorKind;
use blazing_agents::functions::Functions;
use blazing_agents::messages::{UserMessage, UserPart};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json, json_body, mock, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

const SESSION: &str = "ss_0123456789abcdef";
const TURN: &str = "tr_0123456789abcdef";

fn sse(status: u16, events: &[&str]) -> ResponseTemplate {
    let body: String = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
    ResponseTemplate::new(status)
        .set_body_raw(body.into_bytes(), "text/event-stream")
        .insert_header("x-ba-turn-id", TURN)
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
    let message = UserMessage::new(
        "m1",
        UserPart::Text {
            text: "Hello".to_owned(),
        },
    );

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
        (sse(201, &[]), StreamErrorKind::MissingSessionId),
        (
            sse(201, &[]).insert_header("location", "/v1/agents/ag_1/sessions/nope"),
            StreamErrorKind::MalformedSessionId,
        ),
        (
            ResponseTemplate::new(201)
                .insert_header("location", format!("/sessions/{SESSION}").as_str()),
            StreamErrorKind::MissingTurnId,
        ),
    ];
    for (response, kind) in cases {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .mount(&server)
            .await;

        let error = client
            .chat("ag_1", UserMessage::text("hi"))
            .await
            .unwrap_err();

        let Error::Stream(stream) = error else {
            panic!("expected a stream error, got {error:?}");
        };
        assert_eq!(stream.kind(), kind);
        assert_eq!(stream.meta().status(), 201);
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
                ApprovalDecision::approve("apr_1"),
                ApprovalDecision::deny("apr_2").reason("Too risky."),
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
    let functions = Functions::builder()
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
        .unwrap();

    let mut stream = client
        .chat("ag_1", UserMessage::text("Where is my order?"))
        .functions(functions)
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
    assert_eq!(
        json_body(&sent[0])["functions"],
        json!([{
            "name": "lookup_order",
            "description": "Looks up an order.",
            "inputSchema": { "type": "object", "properties": { "orderId": { "type": "string" } } }
        }])
    );
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
