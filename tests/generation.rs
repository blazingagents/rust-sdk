mod support;

use blazing_agents::Error;
use blazing_agents::error::ObjectErrorKind;
use blazing_agents::generation::{ObjectSchema, SavedPrompt};
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
