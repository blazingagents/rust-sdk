mod support;

use std::io::{Read, Write};
use std::time::Duration;

use blazing_agents::Error;
use blazing_agents::error::ObjectErrorKind;
use blazing_agents::generation::{ObjectSchema, SavedPrompt};
use futures_util::TryStreamExt;
use serde_json::json;
use support::{json_body, mock, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[derive(Debug, serde::Deserialize, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
struct Triage {
    priority: u8,
    summary: String,
}

fn text(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "text/plain; charset=utf-8")
}

#[tokio::test]
async fn completion_sends_the_prompt_and_returns_the_text() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/generation"))
        .respond_with(text("Hello there"))
        .mount(&server)
        .await;

    let answer = client.completion("ag_1", "Say hi").await.unwrap();

    assert_eq!(answer, "Hello there");
    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "prompt": "Say hi", "output": { "type": "text" } })
    );
}

#[tokio::test]
async fn saved_prompts_send_their_id_and_variables() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(text("ok"))
        .mount(&server)
        .await;

    client
        .completion("ag_1", SavedPrompt::new("prompt_1").var("audience", "devs"))
        .user_id("customer_123")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({
            "promptId": "prompt_1",
            "variables": { "audience": "devs" },
            "output": { "type": "text" },
            "userId": "customer_123"
        })
    );
}

#[tokio::test]
async fn object_sends_the_schema_and_decodes_the_result() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(text(r#"{"priority": 2, "summary": "double charge"}"#))
        .mount(&server)
        .await;
    let schema = json!({ "type": "object", "properties": { "priority": { "type": "integer" } } });

    let triage: Triage = client
        .object(
            "ag_1",
            "Triage it",
            ObjectSchema::from_json_schema(schema.clone()),
        )
        .await
        .unwrap();

    assert_eq!(
        triage,
        Triage {
            priority: 2,
            summary: "double charge".to_owned()
        }
    );
    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "prompt": "Triage it", "output": { "type": "object", "schema": schema } })
    );
}

#[tokio::test]
async fn object_failures_are_told_apart() {
    let (server, client) = mock().await;
    for (body, kind) in [
        (r#"{"priority": 2,"#, ObjectErrorKind::Truncated),
        ("not json at all", ObjectErrorKind::InvalidJson),
        (
            r#"{"priority": "high", "summary": "x"}"#,
            ObjectErrorKind::Mismatch,
        ),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(text(body))
            .mount(&server)
            .await;

        let error = client
            .object::<Triage>(
                "ag_1",
                "Triage it",
                ObjectSchema::from_json_schema(json!({})),
            )
            .await
            .unwrap_err();

        let Error::Object(object) = error else {
            panic!("expected an object error for {body:?}, got {error:?}");
        };
        assert_eq!(object.kind(), kind, "{body:?}");
        assert_eq!(object.text(), body);
        assert_eq!(object.meta().status(), 200);
    }
}

#[cfg(feature = "schemars")]
#[test]
fn object_schemas_derive_from_the_type() {
    let schema = ObjectSchema::<Triage>::derive();
    assert_eq!(schema.schema()["type"], "object");
    assert!(schema.schema()["properties"]["summary"].is_object());
}

#[tokio::test]
async fn completion_stream_yields_deltas_and_the_final_text() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/generation"))
        .respond_with(text("Hello, wörld"))
        .mount(&server)
        .await;

    let mut stream = client.completion_stream("ag_1", "Say hi").await.unwrap();

    assert_eq!(stream.meta().status(), 200);
    let first = stream.try_next().await.unwrap().unwrap();
    assert!(!first.is_empty());
    assert!("Hello, wörld".starts_with(&first));
    let full = stream.final_text().await.unwrap();
    assert_eq!(full, "Hello, wörld");
    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0]),
        json!({ "prompt": "Say hi", "output": { "type": "text" } })
    );
}

#[tokio::test]
async fn object_stream_yields_json_text_and_decodes_at_the_end() {
    let (server, client) = mock().await;
    let body = r#"{"priority": 1, "summary": "late delivery"}"#;
    Mock::given(method("POST"))
        .respond_with(text(body))
        .mount(&server)
        .await;

    let stream = client
        .object_stream::<Triage>(
            "ag_1",
            "Triage it",
            ObjectSchema::from_json_schema(json!({})),
        )
        .await
        .unwrap();
    let deltas: Vec<String> = stream.try_collect().await.unwrap();
    assert_eq!(deltas.concat(), body);

    let stream = client
        .object_stream::<Triage>(
            "ag_1",
            "Triage it",
            ObjectSchema::from_json_schema(json!({})),
        )
        .await
        .unwrap();
    let triage = stream.final_object().await.unwrap();
    assert_eq!(
        triage,
        Triage {
            priority: 1,
            summary: "late delivery".to_owned()
        }
    );
}

#[tokio::test]
async fn object_stream_failures_are_told_apart() {
    let (server, client) = mock().await;
    for (body, kind) in [
        (r#"{"priority": 2,"#, ObjectErrorKind::Truncated),
        ("not json at all", ObjectErrorKind::InvalidJson),
        (
            r#"{"priority": "high", "summary": "x"}"#,
            ObjectErrorKind::Mismatch,
        ),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(text(body))
            .mount(&server)
            .await;

        let stream = client
            .object_stream::<Triage>(
                "ag_1",
                "Triage it",
                ObjectSchema::from_json_schema(json!({})),
            )
            .await
            .unwrap();
        let error = stream.final_object().await.unwrap_err();

        let Error::Object(object) = error else {
            panic!("expected an object error for {body:?}, got {error:?}");
        };
        assert_eq!(object.kind(), kind, "{body:?}");
        assert_eq!(object.text(), body);
    }
}

/// Answers one request with a chunked body whose second chunk arrives after
/// `pause`, so the headers are fast and the body is slow.
fn slow_body_server(pause: Duration) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = socket.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&request);
            let Some((head, body)) = text.split_once("\r\n\r\n") else {
                continue;
            };
            let length = head
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if body.len() >= length {
                break;
            }
        }
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nHello\r\n",
            )
            .unwrap();
        socket.flush().unwrap();
        std::thread::sleep(pause);
        socket.write_all(b"6\r\n world\r\n0\r\n\r\n").unwrap();
        socket.flush().unwrap();
    });
    format!("http://{address}")
}

#[tokio::test]
async fn a_buffered_completion_is_not_cut_off_by_the_timeout_once_headers_arrived() {
    let base_url = slow_body_server(Duration::from_millis(300));
    let client = blazing_agents::Client::builder()
        .api_key("test-key")
        .base_url(base_url)
        .timeout(Some(Duration::from_millis(100)))
        .build()
        .unwrap();

    let answer = client.completion("ag_1", "Say hi").await.unwrap();

    assert_eq!(answer, "Hello world");
}
