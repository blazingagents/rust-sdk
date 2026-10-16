use serde::{Serialize, Serializer, ser::Error};

/// The state of a nullable field in a PATCH or PUT body. `Keep` leaves the field
/// out, `Clear` sends `null`, `Set` sends the value. Every field of this type
/// carries `#[serde(skip_serializing_if = "Patch::is_keep")]`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Patch<T> {
    #[default]
    Keep,
    Clear,
    Set(T),
}

impl<T> Patch<T> {
    pub(crate) fn is_keep(&self) -> bool {
        matches!(self, Patch::Keep)
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Patch::Keep => Err(S::Error::custom(
                "Patch::Keep must be skipped with skip_serializing_if = \"Patch::is_keep\"",
            )),
            Patch::Clear => serializer.serialize_none(),
            Patch::Set(value) => value.serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;

    use super::Patch;

    #[derive(Serialize)]
    struct Body {
        #[serde(skip_serializing_if = "Patch::is_keep")]
        field: Patch<String>,
    }

    #[test]
    fn keep_is_the_default_and_leaves_the_field_out() {
        assert_eq!(Patch::<String>::default(), Patch::Keep);
        assert_eq!(
            serde_json::to_value(Body { field: Patch::Keep }).unwrap(),
            json!({})
        );
    }

    #[test]
    fn clear_sends_null_and_set_sends_the_value() {
        assert_eq!(
            serde_json::to_value(Body {
                field: Patch::Clear
            })
            .unwrap(),
            json!({ "field": null })
        );
        assert_eq!(
            serde_json::to_value(Body {
                field: Patch::Set("x".to_owned())
            })
            .unwrap(),
            json!({ "field": "x" })
        );
    }

    #[test]
    fn a_bare_keep_fails_to_serialize() {
        let error = serde_json::to_value(Patch::<String>::Keep).unwrap_err();
        assert!(
            error.to_string().contains("Patch::Keep must be skipped"),
            "{error}"
        );
    }
}
