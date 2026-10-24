mod support;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use blazing_agents::client::ResponseObservation;
use blazing_agents::error::{ConfigError, ErrorCode, StreamErrorKind};
use blazing_agents::files::Upload;
use blazing_agents::ids::PromptId;
use blazing_agents::providers::ProviderType;
use blazing_agents::skills::ArchiveType;
use blazing_agents::types::Timestamp;
use blazing_agents::{Client, Error};
use bytes::Bytes;
use futures_util::{TryStreamExt, stream};
use http::{HeaderName, HeaderValue};
use serde_json::json;
use support::{api_error, json, mock, requests};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

fn header<'a>(request: &'a wiremock::Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .get(name)
        .map(|value| value.to_str().unwrap())
}

fn header_values(request: &wiremock::Request, name: &str) -> Vec<String> {
    request
        .headers
        .get_all(name)
        .iter()
        .map(|value| value.to_str().unwrap().to_owned())
        .collect()
}

fn prompt(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_4kP9sT2vXq7LmN3a",
        "name": "Refund reply",
        "template": "Reply to {{customerName}} about {{orderId}}.",
        "variables": ["customerName", "orderId"],
        "userId": "",
        "metadata": { "team": "support" },
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z"
    })
}

fn provider(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": "Production OpenRouter",
        "providerType": "openrouter",
        "baseUrl": null,
        "keyFragment": "9f2c",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:00:00.000Z"
    })
}

fn agent() -> serde_json::Value {
    json!({
        "id": "ag_1",
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "name": "Support writer",
        "model": "openai/gpt-6-luna",
        "thinkingLevel": "high",
        "providerId": "prv_7Tn4Kd9QwE2sLx5R",
        "workspaceId": "ws_3Jv8Km2Np6Qr4Xt7",
        "autoCompaction": true,
        "compactionReserveTokens": 16384,
        "memoryInjectionEnabled": false,
        "tools": ["read"],
        "instructions": "Answer clearly and briefly.",
        "userId": "",
        "metadata": {},
        "mcpConnectionIds": [],
        "avatarUrl": "https://files.example.com/agents/ag_1/avatar.png?sig=abc",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "status": "active"
    })
}

fn skill_detail() -> serde_json::Value {
    json!({
        "id": "skill_1",
        "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
        "agentId": "ag_1",
        "name": "refund-policy",
        "description": "Answer refund questions.",
        "createdAt": "2026-07-10T10:00:00.000Z",
        "updatedAt": "2026-07-10T10:15:00.000Z",
        "files": [{ "path": "SKILL.md", "sizeBytes": 240 }]
    })
}

/// A scratch directory under the system temp dir, removed by the caller.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ba-transport-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Answers one request with a Content-Length longer than the body it sends,
/// then closes the connection.
async fn truncated_server(status: u16, content_type: &'static str, body: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut received = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !received.windows(4).any(|window| window == b"\r\n\r\n") {
            let Ok(read) = stream.read(&mut chunk).await else {
                return;
            };
            if read == 0 {
                return;
            }
            received.extend_from_slice(&chunk[..read]);
        }
        let head = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: 100\r\nConnection: close\r\n\r\n"
        );
        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(body).await;
    });
    format!("http://{address}")
}

/// Answers one request's headers at once, then stalls the body for `delay`.
async fn slow_body_server(status: u16, delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut received = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !received.windows(4).any(|window| window == b"\r\n\r\n") {
            let Ok(read) = stream.read(&mut chunk).await else {
                return;
            };
            if read == 0 {
                return;
            }
            received.extend_from_slice(&chunk[..read]);
        }
        let head = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: 50\r\n\r\n"
        );
        if stream.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        tokio::time::sleep(delay).await;
        let _ = stream
            .write_all(br#"{"error":{"code":"gone","message":"away"}}"#)
            .await;
    });
    format!("http://{address}")
}

#[tokio::test]
async fn headers_follow_the_sdk_policy() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(format!("{}/", server.uri()))
        .default_header("x-custom", "1")
        .default_header("x-request-id", "injected")
        .default_header("authorization", "injected")
        .default_header("x-ba-user-id", "spoofed")
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client
        .prompts()
        .delete("prompt_1")
        .with_header(
            HeaderName::from_static("x-per-call"),
            HeaderValue::from_static("2"),
        )
        .with_header(
            HeaderName::from_static("x-request-id"),
            HeaderValue::from_static("injected"),
        )
        .with_header(
            HeaderName::from_static("x-ba-user-id"),
            HeaderValue::from_static("spoofed"),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(header(&sent[0], "x-custom"), Some("1"));
    assert_eq!(header(&sent[0], "x-per-call"), Some("2"));
    assert_eq!(header(&sent[0], "x-request-id"), None);
    assert_eq!(header(&sent[0], "x-ba-user-id"), None);
    assert_eq!(header(&sent[0], "authorization"), Some("Bearer test-key"));
    assert!(
        header(&sent[0], "user-agent")
            .unwrap()
            .starts_with("blazing-agents-rust/")
    );
    assert_eq!(sent[0].url.path(), "/v1/prompts/prompt_1");
}

#[tokio::test]
async fn content_type_is_owned_by_the_sdk() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/prompts"))
        .respond_with(json(200, json!({})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(200, json!({})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, json!({})))
        .mount(&server)
        .await;

    client
        .prompts()
        .create("n", "doc")
        .with_header(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain"),
        )
        .await
        .unwrap_err();
    let with_default = Client::builder()
        .api_key("test-key")
        .base_url(format!("{}/", server.uri()))
        .default_header("content-type", "text/plain")
        .build()
        .unwrap();
    with_default.prompts().create("n", "doc").await.unwrap_err();
    with_default
        .prompts()
        .get("prompt_1")
        .with_header(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain"),
        )
        .await
        .unwrap_err();
    with_default
        .agents()
        .upload_avatar(
            "ag_1",
            Upload::bytes(b"png-bytes".as_slice()).file_name("avatar.png"),
        )
        .with_header(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain"),
        )
        .await
        .unwrap_err();

    let sent = requests(&server).await;
    for index in [0, 1] {
        let values: Vec<_> = sent[index].headers.get_all("content-type").iter().collect();
        assert_eq!(values.len(), 1, "request {index}: {values:?}");
        assert_eq!(values[0], "application/json");
    }
    assert_eq!(header(&sent[2], "content-type"), None);
    let values: Vec<_> = sent[3].headers.get_all("content-type").iter().collect();
    assert_eq!(values.len(), 1, "request 3: {values:?}");
    assert!(
        values[0]
            .to_str()
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
}

#[tokio::test]
async fn client_request_id_prefers_the_call_over_the_client() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let correlated = client.with_client_request_id("client-level");

    correlated.prompts().delete("prompt_1").await.unwrap();
    correlated
        .prompts()
        .delete("prompt_1")
        .with_client_request_id("call-level")
        .await
        .unwrap();
    client.prompts().delete("prompt_1").await.unwrap();

    let sent = requests(&server).await;
    assert_eq!(
        header(&sent[0], "x-client-request-id"),
        Some("client-level")
    );
    assert_eq!(header(&sent[1], "x-client-request-id"), Some("call-level"));
    assert_eq!(header(&sent[2], "x-client-request-id"), None);
}

#[tokio::test]
async fn timeouts_apply_per_call() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .timeout(Some(Duration::from_millis(50)))
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;

    let error = client.prompts().delete("prompt_1").await.unwrap_err();
    assert!(matches!(error, Error::Timeout), "{error:?}");
    assert_eq!(error.to_string(), "request timed out");
    assert_eq!(error.status(), None);
    assert_eq!(error.request_id(), None);
    assert_eq!(error.code(), None);
    client
        .prompts()
        .delete("prompt_1")
        .without_timeout()
        .await
        .unwrap();
    client
        .prompts()
        .delete("prompt_1")
        .with_timeout(Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn on_response_observes_errors_and_survives_panics() {
    let server = wiremock::MockServer::start().await;
    let seen: Arc<Mutex<Vec<ResponseObservation>>> = Arc::default();
    let sink = Arc::clone(&seen);
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .on_response(move |observation| {
            let first = sink.lock().unwrap().is_empty();
            sink.lock().unwrap().push(observation.clone());
            assert!(!first, "the first observation panics on purpose");
        })
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(api_error(404, "not_found", "gone").insert_header("x-request-id", "req-1"))
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();
    client
        .prompts()
        .get("prompt_1")
        .with_client_request_id("cid")
        .await
        .unwrap_err();
    client
        .prompts()
        .get("prompt_1")
        .with_header(
            HeaderName::from_static("x-client-request-id"),
            HeaderValue::from_static("from-header"),
        )
        .await
        .unwrap_err();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[2].client_request_id.as_deref(), Some("from-header"));
    assert_eq!(seen[0].operation_id, "deletePrompt");
    assert_eq!(seen[1].operation_id, "getPrompt");
    assert_eq!(seen[1].method, "GET");
    assert_eq!(seen[1].path, "/v1/prompts/prompt_1");
    assert_eq!(seen[1].status, 404);
    assert_eq!(seen[1].request_id.as_deref(), Some("req-1"));
    assert_eq!(seen[1].client_request_id.as_deref(), Some("cid"));
}

#[tokio::test]
async fn a_non_json_error_body_is_kept_verbatim() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(502).set_body_string("<html>bad gateway</html>"))
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.status(), 502);
    assert!(
        matches!(api.code(), ErrorCode::Unrecognized(code) if code.as_str() == "invalid_response"),
        "{:?}",
        api.code()
    );
    assert_eq!(
        api.message(),
        "The server returned an invalid error response."
    );
    assert_eq!(api.body(), "<html>bad gateway</html>");
}

#[tokio::test]
async fn submitted_credentials_are_redacted_from_errors() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(
            api_error(400, "validation_failed", "Key sk-secret-abcdef rejected.")
                .insert_header("x-echo", "sk-secret-abcdef"),
        )
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "sk-secret-abcdef")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.message(), "Key [REDACTED] rejected.");
    assert!(!api.body().contains("sk-secret-abcdef"), "{}", api.body());
    assert!(api.body().contains("[REDACTED]"));
    assert_eq!(api.headers().get("x-echo").unwrap(), "[REDACTED]");
}

#[tokio::test]
async fn short_secrets_are_redacted_only_on_exact_match() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_string("not json with abc inside")
                .insert_header("x-echo", "abc"),
        )
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "abc")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.body(), "[REDACTED]");
    assert_eq!(api.headers().get("x-echo").unwrap(), "[REDACTED]");
}

#[tokio::test]
async fn a_response_that_echoes_credentials_fails_the_call() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_1"))
        .respond_with(json(200, json!({ "id": "prv_1", "apiKey": "leaked" })))
        .mount(&server)
        .await;

    let error = client.providers().get("prv_1").await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "getProvider",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn retry_after_accepts_an_http_date() {
    let (server, client) = mock().await;
    let at = SystemTime::now() + Duration::from_secs(90);
    Mock::given(method("GET"))
        .respond_with(
            api_error(429, "rate_limited", "slow down")
                .insert_header("retry-after", httpdate::fmt_http_date(at).as_str()),
        )
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    let retry_after = api.retry_after().unwrap();
    assert!(retry_after > Duration::from_secs(80) && retry_after <= Duration::from_secs(90));
}

#[tokio::test]
async fn path_parameters_are_encoded_as_one_segment_or_rejected() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/a%20b%2Fc%3Fd"))
        .respond_with(json(200, json!({})))
        .expect(1)
        .mount(&server)
        .await;

    client.prompts().get("a b/c?d").await.unwrap_err();
    for unsafe_id in ["", ".", ".."] {
        let error = client.prompts().get(unsafe_id).await.unwrap_err();
        assert!(
            matches!(error, Error::InvalidArgument(_)),
            "{unsafe_id:?}: {error:?}"
        );
    }

    assert_eq!(requests(&server).await.len(), 1);
}

#[tokio::test]
async fn downloads_stream_the_body() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "SKILL.md"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(b"# Skill\n".to_vec(), "application/octet-stream"),
        )
        .mount(&server)
        .await;

    let download = client
        .agent("ag_1")
        .skills()
        .read_file("skill_1", "SKILL.md")
        .await
        .unwrap();

    assert_eq!(download.content_type(), Some("application/octet-stream"));
    assert_eq!(download.content_length(), Some(8));
    assert_eq!(download.meta().status(), 200);
    assert_eq!(download.bytes().await.unwrap().as_ref(), b"# Skill\n");
}

#[tokio::test]
async fn uploads_send_multipart_and_octet_stream_bodies() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(
            200,
            json!({
                "id": "ag_1",
                "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
                "name": "Support writer",
                "model": "openai/gpt-6-luna",
                "thinkingLevel": "high",
                "providerId": "prv_7Tn4Kd9QwE2sLx5R",
                "workspaceId": "ws_3Jv8Km2Np6Qr4Xt7",
                "autoCompaction": true,
                "compactionReserveTokens": 16384,
                "memoryInjectionEnabled": false,
                "tools": ["read"],
                "instructions": "Answer clearly and briefly.",
                "userId": "",
                "metadata": {},
                "mcpConnectionIds": [],
                "avatarUrl": "https://files.example.com/agents/ag_1/avatar.png?sig=abc",
                "createdAt": "2026-07-10T10:00:00.000Z",
                "updatedAt": "2026-07-10T10:15:00.000Z",
                "status": "active"
            }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "SKILL.md"))
        .respond_with(json(
            200,
            json!({
                "id": "skill_1",
                "tenantId": "ten_8Hq2Zr5WcY1bJt6D",
                "agentId": "ag_1",
                "name": "refund-policy",
                "description": "Answer refund questions.",
                "createdAt": "2026-07-10T10:00:00.000Z",
                "updatedAt": "2026-07-10T10:15:00.000Z",
                "files": [{ "path": "SKILL.md", "sizeBytes": 240 }]
            }),
        ))
        .mount(&server)
        .await;

    client
        .agents()
        .upload_avatar(
            "ag_1",
            Upload::bytes("png!")
                .file_name("avatar.png")
                .content_type("image/png"),
        )
        .await
        .unwrap();
    client
        .agent("ag_1")
        .skills()
        .replace_file("skill_1", "SKILL.md", Upload::bytes("# Hi"))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert!(
        header(&sent[0], "content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(
        multipart.contains("name=\"file\"; filename=\"avatar.png\""),
        "{multipart}"
    );
    assert!(multipart.contains("Content-Type: image/png"), "{multipart}");
    assert!(multipart.contains("png!"), "{multipart}");
    assert_eq!(
        header(&sent[1], "content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(sent[1].body, b"# Hi");
}

#[test]
fn user_ids_are_validated() {
    let client = Client::builder().api_key("k").build().unwrap();
    for invalid in ["", " a", "a ", "tab\tin", "ünïcode", &"x".repeat(257)] {
        let error = client.for_user(invalid).unwrap_err();
        assert!(
            matches!(error, ConfigError::InvalidUserId),
            "{invalid:?} should be rejected"
        );
        assert_eq!(
            error.to_string(),
            "invalid user ID: 1 to 256 printable ASCII characters without surrounding spaces"
        );
    }
    assert!(client.for_user("customer_123").is_ok());
    assert!(client.for_user("x".repeat(256)).is_ok());
}

#[test]
fn base_urls_are_validated() {
    let error = Client::builder()
        .api_key("k")
        .base_url("not a url")
        .build()
        .unwrap_err();
    assert!(matches!(error, ConfigError::InvalidBaseUrl(_)), "{error:?}");
    assert_eq!(error.to_string(), "invalid base URL: not a url");
    let error = Client::builder()
        .api_key("k")
        .default_header("bad header", "v")
        .build()
        .unwrap_err();
    assert!(matches!(error, ConfigError::InvalidHeader(_)), "{error:?}");
    assert_eq!(error.to_string(), "invalid default header: bad header");
}

#[test]
fn open_enums_keep_unknown_values() {
    assert_eq!(ArchiveType::from("zip"), ArchiveType::Zip);
    let unknown = ArchiveType::from("7z");
    assert!(matches!(&unknown, ArchiveType::Unrecognized(value) if value.as_str() == "7z"));
    assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"7z\"");
    assert_eq!(
        serde_json::from_str::<ArchiveType>("\"tar.gz\"").unwrap(),
        ArchiveType::TarGz
    );
    let parsed = serde_json::from_str::<ArchiveType>("\"rar\"").unwrap();
    assert!(matches!(&parsed, ArchiveType::Unrecognized(value) if value.as_str() == "rar"));
}

#[test]
fn timestamps_round_trip_as_rfc_3339() {
    let parsed: Timestamp = "2026-07-10T10:00:00.250+02:00".parse().unwrap();
    assert_eq!(parsed.to_string(), "2026-07-10T08:00:00.25Z");
    assert_eq!(parsed.as_unix_millis(), 1_783_670_400_250);
    assert_eq!(
        serde_json::to_string(&parsed).unwrap(),
        "\"2026-07-10T08:00:00.25Z\""
    );
    assert!("yesterday".parse::<Timestamp>().is_err());
}

#[test]
fn api_keys_that_cannot_be_sent_fail_the_build() {
    let built = Client::builder().api_key("bad\nkey").build();
    assert!(matches!(built, Err(ConfigError::InvalidApiKey)));
    assert_eq!(
        built.unwrap_err().to_string(),
        "the API key is not a valid HTTP header value"
    );
}

#[test]
fn a_config_error_converts_into_an_error() {
    fn build() -> Result<Client, Error> {
        Ok(Client::builder().api_key("").build()?)
    }

    let error = build().unwrap_err();

    assert!(
        matches!(error, Error::Config(ConfigError::MissingApiKey)),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "no API key: pass api_key or set BLAZING_AGENTS_API_KEY"
    );
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(error.status(), None);
    assert_eq!(error.request_id(), None);
    assert_eq!(error.code(), None);
}

#[test]
fn debug_output_never_shows_request_bodies() {
    let client = Client::builder().api_key("k").build().unwrap();
    let call = client
        .providers()
        .create("n", ProviderType::OpenAi, "sk-supersecret-123");
    assert!(!format!("{call:?}").contains("sk-supersecret-123"));
}

#[test]
fn default_headers_fail_on_the_first_bad_name_or_value() {
    let error = Client::builder()
        .api_key("k")
        .default_header("bad name", "v")
        .default_header("x-ok", "v")
        .build()
        .unwrap_err();
    assert!(
        matches!(&error, ConfigError::InvalidHeader(name) if name == "bad name"),
        "{error:?}"
    );

    let error = Client::builder()
        .api_key("k")
        .default_header("x-ok", "bad\nvalue")
        .build()
        .unwrap_err();
    assert!(
        matches!(&error, ConfigError::InvalidHeader(name) if name == "x-ok"),
        "{error:?}"
    );
}

#[tokio::test]
async fn repeated_defaults_stay_and_a_call_header_replaces_them() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .default_header("x-dup", "one")
        .default_header("x-dup", "two")
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();
    client
        .prompts()
        .delete("prompt_1")
        .with_header(
            HeaderName::from_static("x-dup"),
            HeaderValue::from_static("three"),
        )
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(header_values(&sent[0], "x-dup"), ["one", "two"]);
    assert_eq!(header_values(&sent[1], "x-dup"), ["three"]);
}

#[test]
fn base_url_must_be_absolute_http_or_https() {
    for url in ["ftp://host", "file:///tmp"] {
        let error = Client::builder()
            .api_key("k")
            .base_url(url)
            .build()
            .unwrap_err();
        assert!(
            matches!(&error, ConfigError::InvalidBaseUrl(refused) if refused == url),
            "{url}: {error:?}"
        );
    }
}

#[tokio::test]
async fn a_base_url_path_prefix_is_kept() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(format!("{}/proxy//", server.uri()))
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .and(path("/proxy/v1/prompts/prompt_1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();

    assert_eq!(
        requests(&server).await[0].url.path(),
        "/proxy/v1/prompts/prompt_1"
    );
}

#[tokio::test]
async fn connect_timeout_builds_a_working_client() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .connect_timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();

    assert_eq!(requests(&server).await.len(), 1);
}

#[test]
fn debug_output_never_shows_the_api_key() {
    let builder = Client::builder()
        .api_key("sk-test-key")
        .base_url("http://localhost:9")
        .timeout(Some(Duration::from_secs(30)))
        .connect_timeout(Duration::from_secs(5));
    let debug = format!("{builder:?}");
    assert!(debug.contains("http://localhost:9"), "{debug}");
    assert!(debug.contains("timeout"), "{debug}");
    assert!(debug.contains("connect_timeout"), "{debug}");
    assert!(!debug.contains("sk-test-key"), "{debug}");

    let user = builder.build().unwrap().for_user("customer_123").unwrap();
    let debug = format!("{user:?}");
    assert!(debug.contains("http://localhost:9"), "{debug}");
    assert!(debug.contains("customer_123"), "{debug}");
    assert!(!debug.contains("sk-test-key"), "{debug}");
}

#[tokio::test]
async fn a_disabled_client_timeout_can_still_be_set_per_call() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .timeout(None)
        .build()
        .unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204).set_delay(Duration::from_millis(100)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(json(200, json!({})).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;

    client.prompts().delete("prompt_1").await.unwrap();
    let error = client
        .prompts()
        .get("prompt_1")
        .with_timeout(Duration::from_millis(50))
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Timeout), "{error:?}");
}

#[tokio::test]
async fn a_user_client_sends_the_user_id_exactly_once() {
    let server = wiremock::MockServer::start().await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .default_header("x-ba-user-id", "spoofed-default")
        .build()
        .unwrap();
    let user = client.for_user("customer_123").unwrap();
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    user.prompts()
        .delete("prompt_1")
        .with_header(
            HeaderName::from_static("x-ba-user-id"),
            HeaderValue::from_static("spoofed-call"),
        )
        .await
        .unwrap();
    user.clone().prompts().delete("prompt_1").await.unwrap();
    user.with_client_request_id("cid-1")
        .prompts()
        .delete("prompt_1")
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 3);
    for request in &sent {
        assert_eq!(header_values(request, "x-ba-user-id"), ["customer_123"]);
    }
    assert_eq!(header(&sent[2], "x-client-request-id"), Some("cid-1"));
}

#[tokio::test]
async fn send_with_meta_exposes_status_headers_and_request_id() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(
            json(200, prompt("prompt_1"))
                .insert_header("x-request-id", "req-9")
                .insert_header("x-custom", "yes"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_2"))
        .respond_with(json(200, prompt("prompt_2")))
        .mount(&server)
        .await;

    let with_meta = client
        .prompts()
        .get("prompt_1")
        .send_with_meta()
        .await
        .unwrap();
    assert_eq!(with_meta.meta.status(), 200);
    assert_eq!(with_meta.meta.request_id(), Some("req-9"));
    assert_eq!(with_meta.meta.headers().get("x-custom").unwrap(), "yes");
    assert_eq!(with_meta.value.id, PromptId::from("prompt_1"));

    let with_meta = client
        .prompts()
        .get("prompt_2")
        .send_with_meta()
        .await
        .unwrap();
    assert_eq!(with_meta.meta.request_id(), None);
}

#[tokio::test]
async fn an_invalid_client_request_id_fails_before_sending() {
    let (server, client) = mock().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let error = client
        .prompts()
        .delete("prompt_1")
        .with_client_request_id("bad\nid")
        .await
        .unwrap_err();

    assert!(
        matches!(&error, Error::InvalidArgument(message) if message == "client request ID is not a valid header value"),
        "{error:?}"
    );
    assert_eq!(requests(&server).await.len(), 0);
}

#[tokio::test]
async fn a_refused_connection_hides_the_request_url() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let client = Client::builder()
        .api_key("test-key")
        .base_url(format!("http://127.0.0.1:{port}"))
        .build()
        .unwrap();

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    assert!(matches!(error, Error::Connection(_)), "{error:?}");
    assert_eq!(error.to_string(), "connection failed");
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(error.status(), None);
    assert_eq!(error.request_id(), None);
    assert_eq!(error.code(), None);
    let debug = format!("{error:?}");
    assert!(!debug.contains("/v1/prompts"), "{debug}");
}

#[tokio::test]
async fn a_stalled_connect_times_out() {
    // 192.0.2.1 (TEST-NET-1) is never routed, so the connect stalls.
    let client = Client::builder()
        .api_key("test-key")
        .base_url("http://192.0.2.1")
        .connect_timeout(Duration::from_millis(100))
        .build()
        .unwrap();

    let error = client
        .prompts()
        .delete("prompt_1")
        .without_timeout()
        .await
        .unwrap_err();

    assert!(matches!(error, Error::Timeout), "{error:?}");
}

#[tokio::test]
async fn a_truncated_download_fails_on_the_body_read() {
    let uri = truncated_server(200, "application/octet-stream", b"# partial").await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(uri)
        .build()
        .unwrap();

    let download = client
        .agent("ag_1")
        .skills()
        .read_file("skill_1", "SKILL.md")
        .await
        .unwrap();
    let debug = format!("{download:?}");
    assert!(debug.contains("200"), "{debug}");
    assert!(!debug.contains("partial"), "{debug}");

    let error = download.bytes().await.unwrap_err();
    let Error::Stream(stream_error) = &error else {
        panic!("expected a stream error, got {error:?}");
    };
    assert_eq!(stream_error.kind(), StreamErrorKind::Read);
    assert_eq!(error.status(), Some(200));
}

#[tokio::test]
async fn a_truncated_error_body_reports_the_status() {
    let uri = truncated_server(500, "application/json", br#"{"error":{"#).await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(uri)
        .build()
        .unwrap();

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    let Error::Stream(stream_error) = &error else {
        panic!("expected a stream error, got {error:?}");
    };
    assert_eq!(stream_error.kind(), StreamErrorKind::Read);
    assert_eq!(error.status(), Some(500));
}

#[tokio::test]
async fn a_truncated_success_body_reports_the_status() {
    let uri = truncated_server(200, "application/json", br#"{"id":"pro"#).await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(uri)
        .build()
        .unwrap();

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    let Error::Stream(stream_error) = &error else {
        panic!("expected a stream error, got {error:?}");
    };
    assert_eq!(stream_error.kind(), StreamErrorKind::Read);
    assert_eq!(error.status(), Some(200));
}

#[tokio::test]
async fn a_body_stalled_past_the_deadline_times_out() {
    for status in [200, 500] {
        let uri = slow_body_server(status, Duration::from_secs(5)).await;
        let client = Client::builder()
            .api_key("test-key")
            .base_url(uri)
            .build()
            .unwrap();

        let error = client
            .prompts()
            .get("prompt_1")
            .with_timeout(Duration::from_millis(100))
            .await
            .unwrap_err();

        assert!(matches!(error, Error::Timeout), "{status}: {error:?}");
    }
}

#[tokio::test]
async fn a_malformed_error_envelope_falls_back_to_the_default() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(
            400,
            json!({ "error": { "code": 123, "message": "bad" } }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_2"))
        .respond_with(json(
            400,
            json!({ "error": { "code": "gone", "message": 5, "param": "id" } }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_3"))
        .respond_with(json(400, json!({ "error": { "message": "bad" } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_4"))
        .respond_with(json(400, json!({ "error": { "code": "gone" } })))
        .mount(&server)
        .await;

    for id in ["prompt_1", "prompt_2", "prompt_3", "prompt_4"] {
        let error = client.prompts().get(id).await.unwrap_err();
        let Error::Api(api) = error else {
            panic!("expected an API error, got {error:?}");
        };
        assert!(
            matches!(api.code(), ErrorCode::Unrecognized(code) if code.as_str() == "invalid_response"),
            "{id}: {:?}",
            api.code()
        );
        assert_eq!(
            api.message(),
            "The server returned an invalid error response."
        );
        assert_eq!(api.details(), None);
        assert_eq!(api.param(), None);
    }
}

#[tokio::test]
async fn an_unparseable_retry_after_is_ignored() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(
            api_error(429, "rate_limited", "slow down").insert_header("retry-after", "soon"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_2"))
        .respond_with(
            api_error(429, "rate_limited", "slow down")
                .insert_header("retry-after", HeaderValue::from_bytes(&[0xFF]).unwrap()),
        )
        .mount(&server)
        .await;

    for id in ["prompt_1", "prompt_2"] {
        let error = client.prompts().get(id).await.unwrap_err();
        let Error::Api(api) = error else {
            panic!("expected an API error, got {error:?}");
        };
        assert_eq!(api.retry_after(), None, "{id}");
    }
}

#[tokio::test]
async fn submitted_credentials_are_redacted_inside_error_detail_arrays() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(json(
            400,
            json!({
                "error": {
                    "code": "validation_failed",
                    "message": "bad key",
                    "details": { "tried": ["sk-secret-abcdef", 3, true] }
                }
            }),
        ))
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "sk-secret-abcdef")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(
        api.details().unwrap()["tried"],
        json!(["[REDACTED]", 3, true])
    );
    assert!(!api.body().contains("sk-secret-abcdef"), "{}", api.body());
}

#[tokio::test]
async fn non_text_error_headers_are_redacted_in_full() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(
            api_error(400, "validation_failed", "bad")
                .insert_header("x-bin", HeaderValue::from_bytes(&[0xFF]).unwrap()),
        )
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "sk-secret-abcdef")
        .await
        .unwrap_err();

    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}");
    };
    assert_eq!(api.headers().get("x-bin").unwrap(), "[REDACTED]");
}

#[tokio::test]
async fn a_non_json_body_on_a_credential_safe_op_is_a_decode_error() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let error = client.providers().get("prv_1").await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::Decode {
                operation_id: "getProvider",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_submitted_key_echoed_in_a_plain_field_fails_the_call() {
    let (server, client) = mock().await;
    let mut echoed = provider("prv_7Tn4Kd9QwE2sLx5R");
    echoed["name"] = json!("sk-secret-abcdef");
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(json(201, echoed))
        .mount(&server)
        .await;

    let error = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "sk-secret-abcdef")
        .await
        .unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "createProvider",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_credential_field_nested_in_an_array_fails_the_call() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers"))
        .respond_with(json(
            200,
            json!({
                "providers": [{
                    "id": "prv_1",
                    "name": "OpenAI",
                    "providerType": "openai",
                    "createdAt": "2026-07-10T10:00:00.000Z",
                    "updatedAt": "2026-07-10T10:00:00.000Z",
                    "nested": [{ "botToken": "xoxb-leaked" }]
                }]
            }),
        ))
        .mount(&server)
        .await;

    let error = client.providers().list().await.unwrap_err();

    assert!(
        matches!(
            error,
            Error::CredentialEcho {
                operation_id: "listProviders",
                ..
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_short_secret_substring_does_not_fail_the_call() {
    let (server, client) = mock().await;
    let mut body = provider("prv_7Tn4Kd9QwE2sLx5R");
    body["name"] = json!("the key9f2c provider");
    Mock::given(method("POST"))
        .and(path("/v1/providers"))
        .respond_with(json(201, body))
        .mount(&server)
        .await;

    let created = client
        .providers()
        .create("OpenAI", ProviderType::OpenAi, "key9f2c")
        .await
        .unwrap();

    assert_eq!(created.name, "the key9f2c provider");
}

#[tokio::test]
async fn upload_file_sends_multipart_and_octet_stream_bodies() {
    let dir = temp_dir("upload");
    let avatar = dir.join("avatar.png");
    let skill = dir.join("SKILL.md");
    std::fs::write(&avatar, b"png-file-bytes").unwrap();
    std::fs::write(&skill, b"# streamed skill").unwrap();
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, agent()))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "SKILL.md"))
        .respond_with(json(200, skill_detail()))
        .mount(&server)
        .await;

    client
        .agents()
        .upload_avatar("ag_1", Upload::file(&avatar).await.unwrap())
        .await
        .unwrap();
    client
        .agent("ag_1")
        .skills()
        .replace_file("skill_1", "SKILL.md", Upload::file(&skill).await.unwrap())
        .await
        .unwrap();
    std::fs::remove_file(&avatar).unwrap();
    std::fs::remove_file(&skill).unwrap();
    std::fs::remove_dir(&dir).unwrap();

    let sent = requests(&server).await;
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(
        multipart.contains("name=\"file\"; filename=\"avatar.png\""),
        "{multipart}"
    );
    assert!(multipart.contains("Content-Type: image/png"), "{multipart}");
    assert!(multipart.contains("png-file-bytes"), "{multipart}");
    assert_eq!(
        header(&sent[1], "content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(sent[1].body, b"# streamed skill");
}

#[tokio::test]
async fn upload_file_reports_a_missing_path() {
    let dir = std::env::temp_dir().join(format!("ba-missing-{}", std::process::id()));
    let error = Upload::file(dir.join("gone.bin")).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[tokio::test]
async fn a_failing_page_ends_the_pages_stream_with_the_error() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_1")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param("cursor", "c2"))
        .respond_with(api_error(500, "internal", "page two exploded"))
        .mount(&server)
        .await;

    let mut pages = client.prompts().list().pages();

    assert_eq!(
        pages.try_next().await.unwrap().unwrap().data[0].id,
        PromptId::from("prompt_1")
    );
    let error = pages.try_next().await.unwrap_err();
    assert!(matches!(error, Error::Api(_)), "{error:?}");
    assert!(pages.try_next().await.unwrap().is_none());
}

#[tokio::test]
async fn upload_stream_without_a_length_sends_the_streamed_bytes() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, agent()))
        .mount(&server)
        .await;
    let chunks = stream::iter([
        Ok::<Bytes, io::Error>(Bytes::from_static(b"chunk-one-")),
        Ok(Bytes::from_static(b"chunk-two")),
    ]);

    client
        .agents()
        .upload_avatar("ag_1", Upload::stream(chunks, None).file_name("avatar.png"))
        .await
        .unwrap();
    client
        .agents()
        .upload_avatar("ag_1", Upload::bytes(b"raw".as_slice()))
        .await
        .unwrap();

    let sent = requests(&server).await;
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(multipart.contains("chunk-one-chunk-two"), "{multipart}");
    let multipart = String::from_utf8_lossy(&sent[1].body);
    assert!(multipart.contains("filename=\"file\""), "{multipart}");
    assert!(multipart.contains("raw"), "{multipart}");
}

#[tokio::test]
async fn upload_stream_as_an_octet_body_sends_the_streamed_bytes() {
    let (server, client) = mock().await;
    Mock::given(method("PUT"))
        .and(path("/v1/agents/ag_1/skills/skill_1/files"))
        .and(query_param("path", "SKILL.md"))
        .respond_with(json(200, skill_detail()))
        .mount(&server)
        .await;
    let chunks = stream::iter([Ok::<Bytes, io::Error>(Bytes::from_static(b"streamed-body"))]);

    client
        .agent("ag_1")
        .skills()
        .replace_file("skill_1", "SKILL.md", Upload::stream(chunks, Some(13)))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent[0].body, b"streamed-body");
}

#[tokio::test]
async fn upload_content_type_overrides_the_guess_and_validates() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, agent()))
        .expect(1)
        .mount(&server)
        .await;

    client
        .agents()
        .upload_avatar(
            "ag_1",
            Upload::bytes(b"not-really-webp".as_slice())
                .file_name("a.png")
                .content_type("image/webp"),
        )
        .await
        .unwrap();
    let error = client
        .agents()
        .upload_avatar(
            "ag_1",
            Upload::bytes(b"x".as_slice())
                .file_name("a.png")
                .content_type("not a mime"),
        )
        .await
        .unwrap_err();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    let multipart = String::from_utf8_lossy(&sent[0].body);
    assert!(
        multipart.contains("Content-Type: image/webp"),
        "{multipart}"
    );
    assert!(
        matches!(&error, Error::InvalidArgument(message) if message == "invalid MIME type not a mime"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "invalid argument: invalid MIME type not a mime"
    );
    assert_eq!(error.request_id(), None);
}

#[tokio::test]
async fn upload_content_type_is_guessed_from_the_file_extension() {
    let (server, client) = mock().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/avatar"))
        .respond_with(json(200, agent()))
        .mount(&server)
        .await;
    let cases = [
        ("a.png", "image/png"),
        ("a.jpg", "image/jpeg"),
        ("a.jpeg", "image/jpeg"),
        ("a.webp", "image/webp"),
        ("a.gif", "image/gif"),
        ("a.zip", "application/zip"),
        ("a.tar", "application/x-tar"),
        ("a.gz", "application/gzip"),
        ("a.tgz", "application/gzip"),
        ("a.md", "text/markdown"),
        ("a.txt", "text/plain"),
        ("a.json", "application/json"),
        ("a.pdf", "application/pdf"),
        ("PHOTO.JPG", "image/jpeg"),
        ("a.weird", "application/octet-stream"),
        ("no-extension", "application/octet-stream"),
    ];

    for (name, _) in cases {
        client
            .agents()
            .upload_avatar("ag_1", Upload::bytes(b"x".as_slice()).file_name(name))
            .await
            .unwrap();
    }

    let sent = requests(&server).await;
    for ((name, expected), request) in cases.iter().zip(&sent) {
        let multipart = String::from_utf8_lossy(&request.body);
        assert!(
            multipart.contains(&format!(
                "filename=\"{name}\"\r\nContent-Type: {expected}\r\n"
            )),
            "{name}: {multipart}"
        );
    }
}

#[test]
fn upload_debug_leaves_out_the_body() {
    let upload = Upload::bytes(b"secret-body".as_slice())
        .file_name("a.png")
        .content_type("image/png");
    let debug = format!("{upload:?}");
    assert!(debug.contains("a.png"), "{debug}");
    assert!(debug.contains("image/png"), "{debug}");
    assert!(!debug.contains("secret-body"), "{debug}");
}
