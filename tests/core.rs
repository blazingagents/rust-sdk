mod support;

use std::time::{Duration, UNIX_EPOCH};

use blazing_agents::Error;
use blazing_agents::error::{ConfigError, ErrorCode};
use blazing_agents::ids::{AgentId, PromptId, UserId};
use blazing_agents::pagination::Cursor;
use blazing_agents::types::{Metadata, Timestamp};
use futures_util::StreamExt;
use serde_json::json;
use support::{api_error, json, mock, requests};
use wiremock::Mock;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};

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

#[tokio::test]
async fn api_error_exposes_the_envelope_fields() {
    let (server, client) = mock().await;
    let envelope = json!({
        "error": {
            "code": "validation_failed",
            "message": "Name is required.",
            "param": "name",
            "details": { "field": "name" }
        }
    });
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(422, envelope.clone()).insert_header("x-request-id", "req-1"))
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    assert_eq!(error.status(), Some(422));
    assert_eq!(error.code(), Some(&ErrorCode::ValidationFailed));
    assert_eq!(error.request_id(), Some("req-1"));
    assert_eq!(error.to_string(), "[validation_failed] Name is required.");
    assert!(std::error::Error::source(&error).is_none());
    let Error::Api(api) = error else {
        panic!("expected an API error, got {error:?}")
    };
    assert_eq!(api.status(), 422);
    assert_eq!(api.code(), &ErrorCode::ValidationFailed);
    assert_eq!(api.message(), "Name is required.");
    assert_eq!(api.param(), Some("name"));
    assert_eq!(
        serde_json::to_value(api.details().unwrap()).unwrap(),
        json!({ "field": "name" })
    );
    assert_eq!(api.headers().get("x-request-id").unwrap(), "req-1");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(api.body()).unwrap(),
        envelope
    );
    assert_eq!(api.retry_after(), None);
}

#[tokio::test]
async fn a_2xx_body_that_does_not_match_the_model_is_a_decode_error() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts/prompt_1"))
        .respond_with(json(200, json!({ "id": 5 })).insert_header("x-request-id", "req-7"))
        .mount(&server)
        .await;

    let error = client.prompts().get("prompt_1").await.unwrap_err();

    match &error {
        Error::Decode {
            operation_id,
            request_id,
            ..
        } => {
            assert_eq!(*operation_id, "getPrompt");
            assert_eq!(request_id.as_deref(), Some("req-7"));
        }
        other => panic!("expected a decode error, got {other:?}"),
    }
    assert_eq!(error.request_id(), Some("req-7"));
    assert_eq!(error.status(), None);
    assert_eq!(error.code(), None);
    assert_eq!(error.to_string(), "unexpected response body for getPrompt");
    assert!(std::error::Error::source(&error).is_some());
}

#[tokio::test]
async fn a_credential_echo_reports_the_operation_and_request_id() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/providers/prv_1"))
        .respond_with(
            json(200, json!({ "id": "prv_1", "apiKey": "leaked" }))
                .insert_header("x-request-id", "req-9"),
        )
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
    assert_eq!(error.request_id(), Some("req-9"));
    assert_eq!(error.status(), None);
    assert_eq!(
        error.to_string(),
        "the server returned credential material in a response to getProvider"
    );
}

#[tokio::test]
async fn pages_yields_each_page_and_ends_after_the_last() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param_is_missing("cursor"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_1"), prompt("prompt_2")], "nextCursor": "c2" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(
            200,
            json!({ "data": [prompt("prompt_3")], "nextCursor": null }),
        ))
        .mount(&server)
        .await;

    let mut pages = client.prompts().list().pages();
    assert_eq!(format!("{pages:?}"), "Pages");
    assert!(requests(&server).await.is_empty());

    let first = pages.next().await.unwrap().unwrap();
    assert_eq!(first.data.len(), 2);
    assert_eq!(first.next_cursor, Some(Cursor::from("c2")));
    assert_eq!(first.next_cursor.as_ref().unwrap().as_str(), "c2");
    let second = pages.next().await.unwrap().unwrap();
    assert_eq!(second.data.len(), 1);
    assert_eq!(second.next_cursor, None);
    assert!(pages.next().await.is_none());

    assert_eq!(requests(&server).await.len(), 2);
}

#[tokio::test]
async fn cursor_resumes_from_a_saved_cursor() {
    let (server, client) = mock().await;
    Mock::given(method("GET"))
        .and(path("/v1/prompts"))
        .and(query_param("cursor", "c2"))
        .respond_with(json(200, json!({ "data": [], "nextCursor": null })))
        .mount(&server)
        .await;

    client
        .prompts()
        .list()
        .cursor(Cursor::from(String::from("c2")))
        .await
        .unwrap();

    let sent = requests(&server).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].url.query(), Some("cursor=c2"));
}

#[tokio::test]
async fn a_failing_page_ends_the_stream_with_its_error() {
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
        .respond_with(api_error(400, "invalid_cursor", "Bad cursor."))
        .mount(&server)
        .await;

    let mut items = client.prompts().list().items();
    assert_eq!(format!("{items:?}"), "Items");

    let first = items.next().await.unwrap().unwrap();
    assert_eq!(first.id, PromptId::from("prompt_1"));
    let error = items.next().await.unwrap().unwrap_err();
    assert_eq!(error.code(), Some(&ErrorCode::InvalidCursor));
    assert!(items.next().await.is_none());
}

#[test]
fn ids_convert_from_any_string_form_and_serialize_transparently() {
    let id = AgentId::from("ag_1");
    assert_eq!(id, AgentId::from(String::from("ag_1")));
    assert_eq!(id, AgentId::from(&String::from("ag_1")));
    assert_eq!(id, AgentId::from(&id));
    assert_eq!(id.as_str(), "ag_1");
    let text: &str = id.as_ref();
    assert_eq!(text, "ag_1");
    assert_eq!(id.to_string(), "ag_1");
    assert_eq!(id.clone().into_string(), "ag_1");
    assert_eq!(serde_json::to_value(&id).unwrap(), json!("ag_1"));
    assert_eq!(
        serde_json::from_value::<AgentId>(json!("ag_1")).unwrap(),
        id
    );
}

#[test]
fn user_ids_validate_their_shape() {
    for valid in ["a", "a b", "~!", &"x".repeat(256)] {
        assert!(UserId::new(valid).is_ok(), "{valid:?} should be accepted");
    }
    for invalid in ["", &"x".repeat(257), " a", "a ", "a\tb", "é", "\u{7f}"] {
        assert!(
            matches!(UserId::new(invalid), Err(ConfigError::InvalidUserId)),
            "{invalid:?} should be rejected"
        );
    }

    let id = "customer_1".parse::<UserId>().unwrap();
    assert_eq!(id.as_str(), "customer_1");
    let text: &str = id.as_ref();
    assert_eq!(text, "customer_1");
    assert_eq!(id.to_string(), "customer_1");

    let error = UserId::new("").unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid user ID: 1 to 256 printable ASCII characters without surrounding spaces"
    );
}

#[test]
fn timestamps_parse_display_and_convert() {
    let parsed: Timestamp = "2026-07-10T10:00:00.000+02:00".parse().unwrap();
    assert_eq!(parsed.to_string(), "2026-07-10T08:00:00Z");
    assert_eq!(parsed.as_unix_millis(), 1_783_670_400_000);
    assert_eq!(
        parsed.to_system_time(),
        UNIX_EPOCH + Duration::from_millis(1_783_670_400_000)
    );
    assert_eq!(
        serde_json::to_value(parsed).unwrap(),
        json!("2026-07-10T08:00:00Z")
    );
    assert_eq!(
        serde_json::from_value::<Timestamp>(json!("2026-07-10T08:00:00Z")).unwrap(),
        parsed
    );

    let error = "yesterday".parse::<Timestamp>().unwrap_err();
    assert_eq!(error.to_string(), "invalid RFC 3339 timestamp: yesterday");
    assert!(serde_json::from_value::<Timestamp>(json!("nope")).is_err());
    assert!(serde_json::from_value::<Timestamp>(json!(5)).is_err());

    let half_second = Timestamp::try_from(UNIX_EPOCH + Duration::from_millis(1500)).unwrap();
    assert_eq!(half_second.to_string(), "1970-01-01T00:00:01.5Z");
    assert!(Timestamp::try_from(UNIX_EPOCH + Duration::from_secs(400_000_000_000)).is_err());

    let earlier: Timestamp = "2020-01-01T00:00:00Z".parse().unwrap();
    let later: Timestamp = "2020-01-01T00:00:01Z".parse().unwrap();
    assert!(earlier < later);
}

#[test]
fn metadata_wraps_a_json_object() {
    let metadata = Metadata::new().insert("a", 1).insert("b", "x");
    let object = json!({ "a": 1, "b": "x" }).as_object().unwrap().clone();

    assert_eq!(metadata.get("a"), Some(&json!(1)));
    assert_eq!(metadata.get("missing"), None);
    assert_eq!(metadata.as_map(), &object);
    assert_eq!(metadata.clone().into_map(), object.clone());
    assert_eq!(
        metadata,
        [("a", json!(1)), ("b", json!("x"))]
            .into_iter()
            .collect::<Metadata>()
    );
    assert_eq!(metadata, Metadata::from(object));
    assert_eq!(
        Metadata::try_from(json!({ "a": 1, "b": "x" })).unwrap(),
        metadata
    );
    let error = Metadata::try_from(json!([1])).unwrap_err();
    assert_eq!(error.to_string(), "metadata must be a JSON object");
    assert_eq!(
        serde_json::to_value(&metadata).unwrap(),
        json!({ "a": 1, "b": "x" })
    );
}

#[test]
fn error_codes_are_an_open_enum() {
    assert_eq!("not_found".parse::<ErrorCode>(), Ok(ErrorCode::NotFound));
    assert_eq!(ErrorCode::from("not_found"), ErrorCode::NotFound);
    assert_eq!(
        ErrorCode::from(String::from("not_found")),
        ErrorCode::NotFound
    );
    assert_eq!(ErrorCode::NotFound.as_str(), "not_found");
    assert_eq!(ErrorCode::NotFound.to_string(), "not_found");

    let unknown = "code_added_next_year".parse::<ErrorCode>().unwrap();
    let ErrorCode::Unrecognized(code) = &unknown else {
        panic!("expected an unrecognized code, got {unknown:?}")
    };
    assert_eq!(code.as_str(), "code_added_next_year");
    assert_eq!(code.to_string(), "code_added_next_year");
    assert_eq!(unknown.as_str(), "code_added_next_year");
    assert_eq!(unknown.to_string(), "code_added_next_year");
    assert_eq!(format!("{:?}", ErrorCode::NotFound), "NotFound");
    assert!(format!("{unknown:?}").contains("code_added_next_year"));

    for code in [ErrorCode::NotFound, unknown] {
        assert_eq!(
            serde_json::from_value::<ErrorCode>(serde_json::to_value(&code).unwrap()).unwrap(),
            code
        );
    }
    assert_eq!(
        serde_json::to_value(&ErrorCode::NotFound).unwrap(),
        json!("not_found")
    );
}
