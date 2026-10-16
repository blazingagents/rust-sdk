mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use blazing_agents::error::StreamErrorKind;
use blazing_agents::functions::{Functions, FunctionsBuilder, FunctionsError};
use blazing_agents::messages::UserMessage;
use futures_util::TryStreamExt;
use http::{HeaderName, HeaderValue};
use serde_json::json;
use support::{api_error, json, json_body, requests};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SESSION: &str = "ss_0123456789abcdef";
const TURN: &str = "turn_0123456789abcdef";
const CALL: &str = "fc_0123456789abcdef";

fn call_path(action: &str) -> String {
    format!("/v1/agents/ag_1/sessions/{SESSION}/function-calls/{CALL}/{action}")
}

fn call_event(name: &str, input: serde_json::Value, deadline_at: &str) -> String {
    json!({
        "type": "data-ba-function-call",
        "data": { "id": CALL, "name": name, "input": input, "deadlineAt": deadline_at },
        "transient": true
    })
    .to_string()
}

/// A new-session chat response whose body is `start`, `call`, `finish`.
fn chat_with(call: &str) -> ResponseTemplate {
    let frames = [r#"{"type":"start"}"#, call, r#"{"type":"finish"}"#]
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect::<Vec<_>>();
    chat_frames(&frames.iter().map(String::as_str).collect::<Vec<_>>())
}

/// A new-session chat response whose body is `frames` byte-for-byte.
fn chat_frames(frames: &[&str]) -> ResponseTemplate {
    ResponseTemplate::new(201)
        .set_body_raw(frames.concat().into_bytes(), "text/event-stream")
        .insert_header("x-ba-turn-id", TURN)
        .insert_header(
            "location",
            format!("/v1/agents/ag_1/sessions/{SESSION}").as_str(),
        )
}

async fn mount_chat_with(server: &MockServer, call: &str) {
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(chat_with(call))
        .mount(server)
        .await;
}

/// Mounts a new-session chat response whose body is `frames` byte-for-byte.
async fn mount_chat_frames(server: &MockServer, frames: &[&str]) {
    Mock::given(method("POST"))
        .and(path("/v1/agents/ag_1/sessions"))
        .respond_with(chat_frames(frames))
        .mount(server)
        .await;
}

async fn mount_chat(server: &MockServer, name: &str, input: serde_json::Value) {
    mount_chat_with(server, &call_event(name, input, "2099-01-01T00:00:00Z")).await;
}

/// An RFC 3339 instant `from_now` in the future.
fn deadline_in(from_now: Duration) -> String {
    let unix = (std::time::SystemTime::now() + from_now)
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let seconds = unix.as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since the epoch (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60,
        unix.subsec_millis()
    )
}

async fn mount_claim(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(json(200, json!({ "claimed": true })))
        .mount(server)
        .await;
}

async fn mount_result(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(call_path("result")))
        .respond_with(json(200, json!({ "accepted": true })))
        .mount(server)
        .await;
}

fn echo_functions() -> Functions {
    Functions::builder()
        .function_json(
            "lookup_order",
            "Looks up an order.",
            json!({ "type": "object" }),
            |input, _| async move { Ok::<_, std::convert::Infallible>(input) },
        )
        .build()
        .unwrap()
}

/// Waits until the server has seen `count` requests, then returns them.
async fn wait_for_requests(server: &MockServer, count: usize) -> Vec<wiremock::Request> {
    wait_for(
        server,
        |sent| sent.len() >= count,
        &format!("{count} requests"),
    )
    .await
}

/// Waits until the received requests satisfy `done`, then returns them.
async fn wait_for(
    server: &MockServer,
    done: impl Fn(&[wiremock::Request]) -> bool,
    what: &str,
) -> Vec<wiremock::Request> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let sent = requests(server).await;
        if done(&sent) {
            return sent;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "waited 5 s for {what}; got {} requests",
            sent.len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Runs a chat with `functions`, drains it, and returns the posted result outcome.
async fn result_outcome(server: &MockServer, functions: Functions) -> serde_json::Value {
    let (_, client) = mock_at(server).await;
    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    let is_result = |request: &wiremock::Request| request.url.path() == call_path("result");
    let sent = wait_for(server, |sent| sent.iter().any(is_result), "a result").await;
    drop(stream);
    let result = sent.iter().find(|request| is_result(request)).unwrap();
    json_body(result)["outcome"].clone()
}

async fn mock_at(server: &MockServer) -> (&MockServer, blazing_agents::Client) {
    let client = blazing_agents::Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .build()
        .unwrap();
    (server, client)
}

#[test]
fn build_rejects_what_the_platform_rejects() {
    let valid = json!({ "type": "object" });
    let add =
        |builder: FunctionsBuilder, name: &str, description: &str, schema: serde_json::Value| {
            builder.function_json(name, description, schema, |input, _| async move {
                Ok::<_, std::convert::Infallible>(input)
            })
        };

    type Case = (FunctionsBuilder, fn(&FunctionsError) -> bool);
    let cases: Vec<Case> = vec![
        (
            add(Functions::builder(), "1bad", "d", valid.clone()),
            |error| matches!(error, FunctionsError::InvalidName(name) if name == "1bad"),
        ),
        (
            add(Functions::builder(), &"a".repeat(65), "d", valid.clone()),
            |error| matches!(error, FunctionsError::InvalidName(_)),
        ),
        (
            add(Functions::builder(), "web_search", "d", valid.clone()),
            |error| matches!(error, FunctionsError::ReservedName(name) if name == "web_search"),
        ),
        (
            add(Functions::builder(), "activate_skill", "d", valid.clone()),
            |error| matches!(error, FunctionsError::ReservedName(_)),
        ),
        (
            add(
                Functions::builder(),
                "mcp__github_search",
                "d",
                valid.clone(),
            ),
            |error| matches!(error, FunctionsError::ReservedName(_)),
        ),
        (
            add(
                add(Functions::builder(), "twice", "d", valid.clone()),
                "twice",
                "d",
                valid.clone(),
            ),
            |error| matches!(error, FunctionsError::Duplicate(name) if name == "twice"),
        ),
        (
            add(Functions::builder(), "blank", "  ", valid.clone()),
            |error| matches!(error, FunctionsError::EmptyDescription(name) if name == "blank"),
        ),
        (
            add(
                Functions::builder(),
                "array",
                "d",
                json!({ "type": "array" }),
            ),
            |error| matches!(error, FunctionsError::NotAnObjectSchema(name) if name == "array"),
        ),
        (
            add(
                Functions::builder(),
                "big",
                "d",
                json!({ "type": "object", "description": "x".repeat(64 * 1024) }),
            ),
            |error| matches!(error, FunctionsError::DefinitionsTooLarge),
        ),
        (
            (0..33).fold(Functions::builder(), |builder, index| {
                add(builder, &format!("f{index}"), "d", valid.clone())
            }),
            |error| matches!(error, FunctionsError::TooMany),
        ),
    ];
    for (builder, expected) in cases {
        let error = builder.build().expect_err("build must fail");
        assert!(expected(&error), "unexpected error {error}");
    }

    let at_limit = (0..32).fold(Functions::builder(), |builder, index| {
        add(builder, &format!("f{index}"), "d", valid.clone())
    });
    assert!(at_limit.build().is_ok());
    let mut dashes = Functions::builder();
    dashes = add(dashes, "Get-Order_v2", "Finds an order.", valid);
    assert!(dashes.build().is_ok());
}

#[test]
fn debug_lists_only_the_function_names() {
    let functions = Functions::builder()
        .function_json(
            "b_fn",
            "Second.",
            json!({ "type": "object" }),
            |input, _| async move { Ok::<_, std::convert::Infallible>(input) },
        )
        .function_json(
            "a_fn",
            "First.",
            json!({ "type": "object" }),
            |input, _| async move { Ok::<_, std::convert::Infallible>(input) },
        )
        .build()
        .unwrap();

    assert_eq!(format!("{functions:?}"), r#"["a_fn", "b_fn"]"#);
}

#[tokio::test]
async fn functions_are_sent_keyed_by_name() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;

    result_outcome(&server, echo_functions()).await;

    let sent = requests(&server).await;
    assert_eq!(
        json_body(&sent[0])["functions"],
        json!({ "lookup_order": { "description": "Looks up an order.", "inputSchema": { "type": "object" } } })
    );
}

#[tokio::test]
async fn per_call_headers_reach_claim_and_result() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .with_header(
            HeaderName::from_static("x-trace"),
            HeaderValue::from_static("abc"),
        )
        .with_client_request_id("req-1")
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    let sent = wait_for_requests(&server, 3).await;
    drop(stream);

    for action in ["claim", "result"] {
        let request = sent
            .iter()
            .find(|request| request.url.path() == call_path(action))
            .unwrap_or_else(|| panic!("no {action} request"));
        assert_eq!(request.headers["x-trace"], "abc");
        assert_eq!(request.headers["x-client-request-id"], "req-1");
    }
}

#[tokio::test]
async fn a_claim_is_retried_after_a_503_and_the_result_is_then_posted() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({ "orderId": "o-1" })).await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(api_error(503, "unavailable", "Try again."))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_claim(&server).await;
    mount_result(&server).await;

    let outcome = result_outcome(&server, echo_functions()).await;

    assert_eq!(
        outcome,
        json!({ "kind": "output", "value": { "orderId": "o-1" } })
    );
    let sent = requests(&server).await;
    let claims: Vec<_> = sent
        .iter()
        .filter(|request| request.url.path() == call_path("claim"))
        .map(json_body)
        .collect();
    assert_eq!(claims.len(), 2, "the claim is retried once");
    assert_eq!(
        claims[0], claims[1],
        "the retry reuses the claim request ID"
    );
}

#[tokio::test]
async fn retry_after_sets_the_delay_of_a_429_with_a_250ms_floor() {
    for (retry_after, at_least, at_most) in [
        ("1", Duration::from_secs(1), Duration::from_secs(2)),
        ("0", Duration::from_millis(250), Duration::from_secs(1)),
    ] {
        let server = MockServer::start().await;
        mount_chat(&server, "lookup_order", json!({})).await;
        Mock::given(method("POST"))
            .and(path(call_path("claim")))
            .respond_with(
                api_error(429, "rate_limited", "Slow down.")
                    .insert_header("retry-after", retry_after),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        mount_claim(&server).await;
        mount_result(&server).await;
        let started = tokio::time::Instant::now();

        result_outcome(&server, echo_functions()).await;

        let elapsed = started.elapsed();
        assert!(
            elapsed >= at_least && elapsed < at_most,
            "Retry-After: {retry_after} waited {elapsed:?}"
        );
    }
}

#[tokio::test]
async fn claims_stop_retrying_at_the_deadline() {
    let server = MockServer::start().await;
    mount_chat_with(
        &server,
        &call_event(
            "lookup_order",
            json!({}),
            &deadline_in(Duration::from_secs(1)),
        ),
    )
    .await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(api_error(503, "unavailable", "Try again."))
        .mount(&server)
        .await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let claims = requests(&server).await.len() - 1;
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert!(
        (2..=4).contains(&claims),
        "{claims} claims within a 1 s budget"
    );
    assert_eq!(
        requests(&server).await.len() - 1,
        claims,
        "no claim after the deadline"
    );
    assert!(
        stream.try_next().await.unwrap().is_none(),
        "an expired claim is not an error"
    );
}

#[tokio::test]
async fn a_handler_still_running_at_the_deadline_is_cancelled_and_posts_nothing() {
    let server = MockServer::start().await;
    mount_chat_with(
        &server,
        &call_event(
            "lookup_order",
            json!({}),
            &deadline_in(Duration::from_secs(1)),
        ),
    )
    .await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel::<bool>();
    let cancelled_tx = std::sync::Mutex::new(Some(cancelled_tx));
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |_, context| {
                let report = cancelled_tx.lock().unwrap().take();
                async move {
                    let _guard = ReportCancelled(report, context.clone());
                    std::future::pending::<()>().await;
                    Ok::<_, std::convert::Infallible>(json!(null))
                }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    let saw_cancel = tokio::time::timeout(Duration::from_secs(5), cancelled_rx)
        .await
        .expect("the handler was dropped at the deadline")
        .unwrap();

    assert!(
        saw_cancel,
        "the context was cancelled before the handler was dropped"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        requests(&server)
            .await
            .iter()
            .all(|request| request.url.path() != call_path("result")),
        "no result is posted after the deadline"
    );
    assert!(stream.try_next().await.unwrap().is_none());
}

/// Reports, when dropped, whether the handler's context was cancelled by then.
struct ReportCancelled(
    Option<tokio::sync::oneshot::Sender<bool>>,
    blazing_agents::functions::FunctionContext,
);

impl Drop for ReportCancelled {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(self.1.is_cancelled());
        }
    }
}

#[tokio::test]
async fn a_malformed_private_event_fails_the_stream() {
    for frame in [
        r#"{"type":"data-ba-function-call","data":{"id":"fc_0123456789abcdef","name":"lookup_order","in"#,
        r#"{"type":"data-ba-function-call","data":{"id":"nope","name":"lookup_order","input":{},"deadlineAt":"2099-01-01T00:00:00Z"},"transient":true}"#,
        r#"{"type":"data-ba-function-call","data":{"id":"fc_0123456789abcdef","name":"web_search","input":{},"deadlineAt":"2099-01-01T00:00:00Z"},"transient":true}"#,
    ] {
        let server = MockServer::start().await;
        mount_chat_with(&server, frame).await;
        let (_, client) = mock_at(&server).await;

        let mut stream = client
            .chat("ag_1", UserMessage::text("hi"))
            .functions(echo_functions())
            .await
            .unwrap();
        stream.try_next().await.unwrap().unwrap();
        let error = stream.try_next().await.unwrap_err();

        let blazing_agents::Error::Stream(error) = error else {
            panic!("expected a stream error for {frame}, got {error:?}");
        };
        assert_eq!(
            error.kind(),
            StreamErrorKind::MalformedFunctionCall,
            "{frame}"
        );
    }
}

#[tokio::test]
async fn a_refused_claim_is_skipped_silently() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(api_error(409, "function_call_claimed", "Already claimed."))
        .mount(&server)
        .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |input, _| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<_, std::convert::Infallible>(input) }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    let mut frames = 0;
    while stream.try_next().await.unwrap().is_some() {
        frames += 1;
    }

    assert_eq!(frames, 2, "start and finish reach the consumer");
    wait_for_requests(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the handler never ran");
    assert_eq!(requests(&server).await.len(), 2, "no result was posted");
}

#[tokio::test]
async fn a_permanent_claim_failure_is_the_next_stream_item() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(api_error(400, "validation_failed", "Bad claim."))
        .mount(&server)
        .await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    let start = stream.try_next().await.unwrap().unwrap();
    assert_eq!(start, "data: {\"type\":\"start\"}\n\n".as_bytes());
    let finish = stream.try_next().await.unwrap().unwrap();
    assert_eq!(finish, "data: {\"type\":\"finish\"}\n\n".as_bytes());
    wait_for_requests(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let error = stream.try_next().await.unwrap_err();

    assert_eq!(error.status(), Some(400));
    assert_eq!(
        error.code().map(|code| code.as_str()),
        Some("validation_failed")
    );
    assert_eq!(requests(&server).await.len(), 2, "a 400 is not retried");
}

#[tokio::test]
async fn handler_failures_become_sanitized_outcomes() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let failing = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            |_, _| async {
                Err::<serde_json::Value, _>(std::io::Error::other(
                    "secret database password leaked",
                ))
            },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, failing).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function execution failed." })
    );

    server.reset().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let panicking = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            |_, _| async {
                panic!("handler bug");
                #[allow(unreachable_code)]
                Ok::<serde_json::Value, std::convert::Infallible>(json!(null))
            },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, panicking).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function execution failed." })
    );

    server.reset().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let oversize = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            |_, _| async { Ok::<_, std::convert::Infallible>(json!("x".repeat(256 * 1024))) },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, oversize).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function returned an invalid result." })
    );

    server.reset().await;
    mount_chat(&server, "other_function", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;

    let outcome = result_outcome(&server, echo_functions()).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function other_function is not available." })
    );
}

#[cfg(feature = "schemars")]
#[tokio::test]
async fn typed_handlers_reject_invalid_input_without_running() {
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct Lookup {
        order_id: String,
    }
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({ "order_id": 7 })).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let functions = Functions::builder()
        .function::<Lookup, _, _, _, std::convert::Infallible>(
            "lookup_order",
            "d",
            |input, _| async move { Ok(json!({ "found": input.order_id })) },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, functions).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Invalid function input." })
    );

    server.reset().await;
    mount_chat(&server, "lookup_order", json!({ "order_id": "o-1" })).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let tuple_keys = Functions::builder()
        .function::<Lookup, _, _, _, std::convert::Infallible>("lookup_order", "d", |_, _| async {
            Ok(std::collections::HashMap::from([((1, 2), "pair")]))
        })
        .build()
        .unwrap();

    let outcome = result_outcome(&server, tuple_keys).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function returned an invalid result." })
    );
}

#[tokio::test]
async fn dropping_the_stream_cancels_running_handlers() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel::<bool>();
    let started_tx = std::sync::Mutex::new(Some(started_tx));
    let dropped_tx = std::sync::Mutex::new(Some(dropped_tx));
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |_, context| {
                let started = started_tx.lock().unwrap().take();
                let dropped = dropped_tx.lock().unwrap().take();
                async move {
                    let _guard = ReportCancelled(dropped, context.clone());
                    if let Some(started) = started {
                        let _ = started.send(());
                    }
                    context.cancelled().await;
                    Ok::<_, std::convert::Infallible>(
                        json!({ "cancelled": context.is_cancelled() }),
                    )
                }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    stream.try_next().await.unwrap().unwrap();
    stream.try_next().await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(5), started_rx)
        .await
        .expect("the handler started")
        .unwrap();
    drop(stream);

    let saw_cancel = tokio::time::timeout(Duration::from_secs(5), dropped_rx)
        .await
        .expect("the handler was dropped after the stream")
        .unwrap();
    assert!(
        saw_cancel,
        "the context was cancelled before the handler was dropped"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        requests(&server)
            .await
            .iter()
            .all(|request| request.url.path() != call_path("result")),
        "no result is posted for a cancelled handler"
    );
}

#[tokio::test]
async fn the_context_exposes_the_deadline_and_the_call_id() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            |_, context| async move {
                Ok::<_, std::convert::Infallible>(json!({
                    "deadline": context.deadline().to_string(),
                    "key": context.idempotency_key().as_str(),
                }))
            },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, functions).await;

    assert_eq!(
        outcome,
        json!({
            "kind": "output",
            "value": { "deadline": "2099-01-01T00:00:00Z", "key": CALL }
        })
    );
}

#[tokio::test]
async fn cancelled_resolves_when_the_deadline_passes() {
    let server = MockServer::start().await;
    mount_chat_with(
        &server,
        &call_event(
            "lookup_order",
            json!({}),
            &deadline_in(Duration::from_secs(1)),
        ),
    )
    .await;
    mount_claim(&server).await;
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel::<()>();
    let cancelled_tx = std::sync::Mutex::new(Some(cancelled_tx));
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |_, context| {
                let cancelled = cancelled_tx.lock().unwrap().take();
                async move {
                    // The handler itself never observes the resolution: it is
                    // dropped at the deadline. A detached task does.
                    tokio::spawn(async move {
                        context.cancelled().await;
                        if let Some(cancelled) = cancelled {
                            let _ = cancelled.send(());
                        }
                    });
                    std::future::pending::<()>().await;
                    Ok::<_, std::convert::Infallible>(json!(null))
                }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    tokio::time::timeout(Duration::from_secs(5), cancelled_rx)
        .await
        .expect("cancelled() resolves at the deadline")
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(
        requests(&server).await.len(),
        2,
        "chat and claim, but no result"
    );
    assert!(stream.try_next().await.unwrap().is_none());
}

#[tokio::test]
async fn a_claim_body_that_is_not_claimed_true_fails_the_stream() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(json(200, json!({ "claimed": false })))
        .mount(&server)
        .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |input, _| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<_, std::convert::Infallible>(input) }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    stream.try_next().await.unwrap().unwrap();
    stream.try_next().await.unwrap().unwrap();
    wait_for_requests(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let error = stream.try_next().await.unwrap_err();

    let blazing_agents::Error::Decode { source, .. } = error else {
        panic!("expected a decode error, got {error:?}");
    };
    assert_eq!(source.to_string(), "expected claimed: true");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the handler never ran");
    assert_eq!(
        requests(&server).await.len(),
        2,
        "a decode failure is not retried and no result is posted"
    );
}

#[tokio::test]
async fn a_frame_without_data_lines_passes_through() {
    let server = MockServer::start().await;
    mount_chat_frames(
        &server,
        &[
            "data: {\"type\":\"start\"}\n\n",
            ": keep-alive\n\n",
            "data: {\"type\":\"finish\"}\n\n",
        ],
    )
    .await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = stream.try_next().await.unwrap() {
        frames.push(frame);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let frames: Vec<&[u8]> = frames.iter().map(AsRef::as_ref).collect();
    assert_eq!(
        frames,
        [
            "data: {\"type\":\"start\"}\n\n".as_bytes(),
            ": keep-alive\n\n".as_bytes(),
            "data: {\"type\":\"finish\"}\n\n".as_bytes(),
        ]
    );
    assert_eq!(requests(&server).await.len(), 1, "a comment claims nothing");
}

#[tokio::test]
async fn a_frame_whose_data_mentions_the_event_type_passes_through() {
    let server = MockServer::start().await;
    let delta = "data: {\"type\":\"text-delta\",\"delta\":\"data-ba-function-call\"}\n\n";
    mount_chat_frames(
        &server,
        &[
            "data: {\"type\":\"start\"}\n\n",
            delta,
            "data: {\"type\":\"finish\"}\n\n",
        ],
    )
    .await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = stream.try_next().await.unwrap() {
        frames.push(frame);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let frames: Vec<&[u8]> = frames.iter().map(AsRef::as_ref).collect();
    assert_eq!(
        frames,
        [
            "data: {\"type\":\"start\"}\n\n".as_bytes(),
            delta.as_bytes(),
            "data: {\"type\":\"finish\"}\n\n".as_bytes(),
        ]
    );
    assert_eq!(
        requests(&server).await.len(),
        1,
        "a text-delta claims nothing"
    );
}

#[tokio::test]
async fn a_duplicate_call_id_is_claimed_once() {
    let server = MockServer::start().await;
    let call = format!(
        "data: {}\n\n",
        call_event("lookup_order", json!({}), "2099-01-01T00:00:00Z")
    );
    mount_chat_frames(
        &server,
        &[
            "data: {\"type\":\"start\"}\n\n",
            call.as_str(),
            call.as_str(),
            "data: {\"type\":\"finish\"}\n\n",
        ],
    )
    .await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |input, _| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<_, std::convert::Infallible>(input) }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    let mut frames = 0;
    while stream.try_next().await.unwrap().is_some() {
        frames += 1;
    }
    let sent = wait_for_requests(&server, 3).await;
    drop(stream);

    assert_eq!(frames, 2, "start and finish reach the consumer");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the handler ran once");
    assert_eq!(
        sent.iter()
            .filter(|request| request.url.path() == call_path("claim"))
            .count(),
        1,
        "the duplicate event is not claimed again"
    );
}

#[tokio::test]
async fn a_claim_timeout_is_retried() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({ "orderId": "o-1" })).await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(json(200, json!({ "claimed": true })).set_delay(Duration::from_secs(1)))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_claim(&server).await;
    mount_result(&server).await;
    let client = blazing_agents::Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .timeout(Some(Duration::from_millis(200)))
        .build()
        .unwrap();

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    while stream.try_next().await.unwrap().is_some() {}
    let is_result = |request: &wiremock::Request| request.url.path() == call_path("result");
    let sent = wait_for(&server, |sent| sent.iter().any(is_result), "a result").await;
    drop(stream);

    let claims: Vec<_> = sent
        .iter()
        .filter(|request| request.url.path() == call_path("claim"))
        .map(json_body)
        .collect();
    assert_eq!(claims.len(), 2, "a timed-out claim is retried");
    assert_eq!(
        claims[0], claims[1],
        "the retry reuses the claim request ID"
    );
    assert_eq!(
        json_body(sent.iter().find(|request| is_result(request)).unwrap())["outcome"],
        json!({ "kind": "output", "value": { "orderId": "o-1" } })
    );
}

#[tokio::test]
async fn a_call_past_its_deadline_is_skipped() {
    let server = MockServer::start().await;
    mount_chat_with(
        &server,
        &call_event("lookup_order", json!({}), "2000-01-01T00:00:00Z"),
    )
    .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |input, _| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<_, std::convert::Infallible>(input) }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    let mut frames = 0;
    while stream.try_next().await.unwrap().is_some() {
        frames += 1;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(frames, 2, "start and finish reach the consumer");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the handler never ran");
    assert_eq!(
        requests(&server).await.len(),
        1,
        "an expired call is never claimed"
    );
}

#[tokio::test]
async fn a_claim_accepted_after_the_deadline_posts_nothing() {
    let server = MockServer::start().await;
    mount_chat_with(
        &server,
        &call_event(
            "lookup_order",
            json!({}),
            &deadline_in(Duration::from_secs(1)),
        ),
    )
    .await;
    Mock::given(method("POST"))
        .and(path(call_path("claim")))
        .respond_with(json(200, json!({ "claimed": true })).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;
    mount_result(&server).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let functions = Functions::builder()
        .function_json(
            "lookup_order",
            "d",
            json!({ "type": "object" }),
            move |input, _| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<_, std::convert::Infallible>(input) }
            },
        )
        .build()
        .unwrap();
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(functions)
        .await
        .unwrap();
    let mut frames = 0;
    while stream.try_next().await.unwrap().is_some() {
        frames += 1;
    }
    wait_for_requests(&server, 2).await;
    // The claim answers 2 s in; the deadline check runs once it arrives.
    tokio::time::sleep(Duration::from_millis(2_500)).await;

    assert_eq!(frames, 2, "start and finish reach the consumer");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the handler never ran");
    assert_eq!(
        requests(&server).await.len(),
        2,
        "a claim accepted after the deadline posts no result"
    );
    assert!(stream.try_next().await.unwrap().is_none());
}

#[tokio::test]
async fn a_permanent_result_failure_is_the_next_stream_item() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    Mock::given(method("POST"))
        .and(path(call_path("result")))
        .respond_with(api_error(400, "validation_failed", "Bad result."))
        .mount(&server)
        .await;
    let (_, client) = mock_at(&server).await;

    let mut stream = client
        .chat("ag_1", UserMessage::text("hi"))
        .functions(echo_functions())
        .await
        .unwrap();
    let start = stream.try_next().await.unwrap().unwrap();
    assert_eq!(start, "data: {\"type\":\"start\"}\n\n".as_bytes());
    let finish = stream.try_next().await.unwrap().unwrap();
    assert_eq!(finish, "data: {\"type\":\"finish\"}\n\n".as_bytes());
    wait_for_requests(&server, 3).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let error = stream.try_next().await.unwrap_err();

    assert_eq!(error.status(), Some(400));
    assert_eq!(
        error.code().map(|code| code.as_str()),
        Some("validation_failed")
    );
    assert_eq!(requests(&server).await.len(), 3, "a 400 is not retried");
}

#[cfg(feature = "schemars")]
#[tokio::test]
async fn a_result_that_is_not_plain_json_is_an_invalid_result() {
    let server = MockServer::start().await;
    mount_chat(&server, "lookup_order", json!({})).await;
    mount_claim(&server).await;
    mount_result(&server).await;
    // Serializes fine, but serde_json's parser recursion limit rejects the
    // decode back that checks the result is wire-safe.
    let deep = (0..200).fold(json!(null), |value, _| json!([value]));
    let functions = Functions::builder()
        .function::<std::collections::HashMap<String, serde_json::Value>, _, _, _, std::convert::Infallible>(
            "lookup_order",
            "d",
            move |_, _| {
                let deep = deep.clone();
                async move { Ok(deep) }
            },
        )
        .build()
        .unwrap();

    let outcome = result_outcome(&server, functions).await;

    assert_eq!(
        outcome,
        json!({ "kind": "error", "message": "Function returned an invalid result." })
    );
}
