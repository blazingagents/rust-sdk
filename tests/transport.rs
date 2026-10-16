mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use blazing_agents::client::ResponseObservation;
use blazing_agents::error::{ConfigError, ErrorCode};
use blazing_agents::files::Upload;
use blazing_agents::providers::ProviderType;
use blazing_agents::skills::ArchiveType;
use blazing_agents::types::Timestamp;
use blazing_agents::{Client, Error};
use http::{HeaderName, HeaderValue};
use serde_json::json;
use support::{api_error, json, mock, requests};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn header<'a>(request: &'a wiremock::Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .get(name)
        .map(|value| value.to_str().unwrap())
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
        assert!(
            matches!(client.for_user(invalid), Err(ConfigError::InvalidUserId)),
            "{invalid:?} should be rejected"
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
    let error = Client::builder()
        .api_key("k")
        .default_header("bad header", "v")
        .build()
        .unwrap_err();
    assert!(matches!(error, ConfigError::InvalidHeader(_)), "{error:?}");
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
}

#[test]
fn debug_output_never_shows_request_bodies() {
    let client = Client::builder().api_key("k").build().unwrap();
    let call = client
        .providers()
        .create("n", ProviderType::OpenAi, "sk-supersecret-123");
    assert!(!format!("{call:?}").contains("sk-supersecret-123"));
}
