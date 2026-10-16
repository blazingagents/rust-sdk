use blazing_agents::messages::{UserMessage, UserPart};
use serde_json::json;

fn decode(part: serde_json::Value) -> Result<UserMessage, serde_json::Error> {
    serde_json::from_value(json!({ "id": "m1", "role": "user", "parts": [part] }))
}

#[test]
fn a_text_part_decodes() {
    let message = decode(json!({ "type": "text", "text": "hi" })).unwrap();

    match message.parts().next() {
        Some(UserPart::Text { text, .. }) => assert_eq!(text, "hi"),
        other => panic!("expected a text part, got {other:?}"),
    }
    assert_eq!(message.parts().count(), 1);
}

#[test]
fn a_file_part_decodes() {
    let message = decode(
        json!({ "type": "file", "mediaType": "image/png", "url": "data:image/png;base64,x" }),
    )
    .unwrap();

    match message.parts().next() {
        Some(UserPart::File {
            media_type, url, ..
        }) => {
            assert_eq!(media_type, "image/png");
            assert_eq!(url, "data:image/png;base64,x");
        }
        other => panic!("expected a file part, got {other:?}"),
    }
}

#[test]
fn a_known_part_type_with_a_malformed_body_is_an_error() {
    for part in [
        json!({ "type": "text" }),
        json!({ "type": "text", "text": 5 }),
        json!({ "type": "file", "url": "https://x" }),
    ] {
        assert!(decode(part.clone()).is_err(), "{part} must not decode");
    }
}

#[test]
fn a_part_without_a_string_type_is_an_error() {
    for part in [json!({ "text": "hi" }), json!({ "type": 7 }), json!("hi")] {
        assert!(decode(part.clone()).is_err(), "{part} must not decode");
    }
}

#[test]
fn an_unknown_part_type_round_trips_unchanged() {
    let part = json!({ "type": "reasoning", "text": "t", "extra": 1 });
    let message = decode(part.clone()).unwrap();

    match message.parts().next() {
        Some(UserPart::Unrecognized(raw)) => assert_eq!(raw.as_value(), &part),
        other => panic!("expected an unrecognized part, got {other:?}"),
    }
    let wire = serde_json::to_value(&message).unwrap();
    assert_eq!(wire["parts"][0], part);
}

#[test]
fn unmodeled_part_fields_round_trip_unchanged() {
    let input = json!({
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
    let message = serde_json::from_value::<UserMessage>(input.clone()).unwrap();

    let mut parts = message.parts();
    let Some(UserPart::Text { text, extra, .. }) = parts.next() else {
        panic!("expected a text part, got {:?}", message.parts().next())
    };
    assert_eq!(text, "hi");
    assert_eq!(extra["state"], json!("done"));
    assert_eq!(
        extra["providerMetadata"],
        json!({ "openai": { "itemId": "x" } })
    );
    for key in ["type", "text"] {
        assert!(!extra.contains_key(key), "{key} leaked into extra");
    }
    let Some(UserPart::File {
        media_type,
        url,
        extra,
        ..
    }) = parts.next()
    else {
        panic!("expected a file part")
    };
    assert_eq!(media_type, "image/png");
    assert_eq!(url, "data:image/png;base64,AAAA");
    assert_eq!(extra["filename"], json!("a.png"));
    assert_eq!(
        extra["providerMetadata"],
        json!({ "anthropic": { "cache": true } })
    );
    for key in ["type", "mediaType", "url"] {
        assert!(!extra.contains_key(key), "{key} leaked into extra");
    }

    assert_eq!(serde_json::to_value(&message).unwrap(), input);
}

#[test]
fn extra_cannot_override_the_modeled_keys() {
    let mut part = UserPart::text("real");
    let UserPart::Text { extra, .. } = &mut part else {
        unreachable!()
    };
    extra.insert("type".to_owned(), json!("bogus"));
    extra.insert("text".to_owned(), json!("x"));

    let wire = serde_json::to_value(UserMessage::new("m1", part)).unwrap();
    assert_eq!(wire["parts"][0], json!({ "type": "text", "text": "real" }));
}
