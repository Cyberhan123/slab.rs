//! Context-window length spec: an explicit token count, or `auto` (resolved at
//! load time to the largest context that fits in GPU VRAM).

use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Context window specification.
///
/// `Fixed(n)` is an explicit token count. `Auto` resolves at model-load time to
/// the largest context that fits in GPU VRAM (queried via all-smi, with a
/// buffer), capped at the model's native training context (`n_ctx_train`); with
/// no VRAM signal it falls back to a conservative default.
///
/// Serialized as a bare non-negative integer (`8192`) or the string `"auto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextLengthSpec {
    Fixed(u32),
    Auto,
}

impl fmt::Display for ContextLengthSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Fixed(value) => write!(formatter, "{value}"),
            Self::Auto => formatter.write_str("auto"),
        }
    }
}

impl ContextLengthSpec {
    /// `Some(n)` when fixed; `None` when `auto` (the concrete size is only known
    /// after the model loads). Used where a legacy `Option<u32>` is expected
    /// (proto wire, display fallbacks).
    pub fn as_fixed_u32(self) -> Option<u32> {
        match self {
            Self::Fixed(value) => Some(value),
            Self::Auto => None,
        }
    }
}

impl Serialize for ContextLengthSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Fixed(value) => serializer.serialize_u32(*value),
            Self::Auto => serializer.serialize_str("auto"),
        }
    }
}

impl<'de> Deserialize<'de> for ContextLengthSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ContextLengthVisitor;

        impl<'de> Visitor<'de> for ContextLengthVisitor {
            type Value = ContextLengthSpec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a context length: a non-negative integer or \"auto\"")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                u32::try_from(value)
                    .map(ContextLengthSpec::Fixed)
                    .map_err(|_| E::custom(format!("context length {value} out of u32 range")))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                if value < 0 {
                    return Err(E::custom(format!("context length {value} is negative")));
                }
                u32::try_from(value)
                    .map(ContextLengthSpec::Fixed)
                    .map_err(|_| E::custom(format!("context length {value} out of u32 range")))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.eq_ignore_ascii_case("auto") {
                    Ok(ContextLengthSpec::Auto)
                } else {
                    Err(E::custom(format!("expected \"auto\", got {value:?}")))
                }
            }
        }

        deserializer.deserialize_any(ContextLengthVisitor)
    }
}

impl JsonSchema for ContextLengthSpec {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ContextLengthSpec".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                { "type": "integer", "minimum": 0 },
                { "type": "string", "const": "auto" }
            ]
        })
    }

    /// Inline (like a primitive) rather than emitting a `$ref` definition.
    fn inline_schema() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_roundtrips_as_a_bare_integer() {
        let json = serde_json::to_string(&ContextLengthSpec::Fixed(8192)).unwrap();
        assert_eq!(json, "8192");
        let back: ContextLengthSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ContextLengthSpec::Fixed(8192));
    }

    #[test]
    fn auto_roundtrips_as_the_string_auto() {
        let json = serde_json::to_string(&ContextLengthSpec::Auto).unwrap();
        assert_eq!(json, "\"auto\"");
        let back: ContextLengthSpec = serde_json::from_str("\"auto\"").unwrap();
        assert_eq!(back, ContextLengthSpec::Auto);
    }

    #[test]
    fn rejects_unknown_strings_and_negatives() {
        assert!(serde_json::from_str::<ContextLengthSpec>("\"manual\"").is_err());
        assert!(serde_json::from_str::<ContextLengthSpec>("-1").is_err());
    }
}
