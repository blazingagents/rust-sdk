/// Defines an enum for a value set the server may grow. Unknown values decode as
/// `Unrecognized`, and `#[non_exhaustive]` lets the SDK add variants later.
///
/// ```ignore
/// crate::open_enum! {
///     /// Whether an agent accepts turns.
///     pub enum AgentStatus {
///         /// `active`
///         Active = "active",
///         /// `disabled`
///         Disabled = "disabled",
///     }
/// }
/// ```
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident = $value:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        $vis enum $name {
            $( $(#[$variant_meta])* $variant, )+
            /// A value this version of the SDK does not know.
            Unrecognized($crate::types::Unrecognized),
        }

        impl $name {
            /// The wire value.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )+
                    Self::Unrecognized(other) => other.as_str(),
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = ::core::convert::Infallible;

            fn from_str(value: &str) -> ::core::result::Result<Self, Self::Err> {
                Ok(match value {
                    $( $value => Self::$variant, )+
                    other => Self::Unrecognized($crate::types::Unrecognized::new(other)),
                })
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                let Ok(parsed) = value.parse();
                parsed
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::from(value.as_str())
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> ::core::result::Result<Self, D::Error> {
                let value = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::from(value.as_str()))
            }
        }
    };
}

pub(crate) use open_enum;
