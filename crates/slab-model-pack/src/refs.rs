use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ModelPackError;

const REF_SCHEME: &str = "ref://";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct ConfigRef(String);

impl ConfigRef {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelPackError> {
        let value = value.into();
        let trimmed = value.trim();

        if !trimmed.starts_with(REF_SCHEME) {
            return Err(ModelPackError::InvalidConfigRef {
                value,
                reason: "references must start with ref://".into(),
            });
        }

        let path = &trimmed[REF_SCHEME.len()..];
        if path.is_empty() {
            return Err(ModelPackError::InvalidConfigRef {
                value,
                reason: "reference path must not be empty".into(),
            });
        }
        if path.starts_with('/') || path.contains("\\") {
            return Err(ModelPackError::InvalidConfigRef {
                value,
                reason: "reference path must be relative and use '/' separators".into(),
            });
        }
        if path.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(ModelPackError::InvalidConfigRef {
                value,
                reason: "reference path must not contain empty, '.' or '..' segments".into(),
            });
        }

        Ok(Self(format!("{REF_SCHEME}{path}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn path(&self) -> &str {
        self.0.strip_prefix(REF_SCHEME).unwrap_or(&self.0)
    }
}

impl TryFrom<String> for ConfigRef {
    type Error = ModelPackError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<ConfigRef> for String {
    fn from(value: ConfigRef) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigRef;

    #[test]
    fn parses_valid_ref_path() {
        let config_ref = ConfigRef::parse("ref://models/llama/qwen/variant.json").unwrap();
        assert_eq!(config_ref.path(), "models/llama/qwen/variant.json");
    }

    #[test]
    fn rejects_parent_segments() {
        let error = ConfigRef::parse("ref://models/../variant.json").unwrap_err();
        assert!(error.to_string().contains(".."));
    }
}