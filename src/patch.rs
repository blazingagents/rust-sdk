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
