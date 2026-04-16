use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SlabTypeError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssetRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "$path")]
    pub path: Option<String>,
}

pub type TemplateAssetRef = AssetRef;
pub type GbnfAssetRef = AssetRef;

impl AssetRef {
    pub fn is_empty(&self) -> bool {
        self.id.as_deref().is_none_or(|value| value.trim().is_empty())
            && self.name.as_deref().is_none_or(|value| value.trim().is_empty())
            && self.path.as_deref().is_none_or(|value| value.trim().is_empty())
    }

    pub fn normalized(&self) -> Option<Self> {
        let normalized = Self {
            id: normalize_optional_text(self.id.as_deref()),
            name: normalize_optional_text(self.name.as_deref()),
            path: normalize_optional_text(self.path.as_deref()),
        };
        (!normalized.is_empty()).then_some(normalized)
    }

    pub fn validate_configured(&self, field: &str) -> Result<Option<Self>, SlabTypeError> {
        let Some(normalized) = self.normalized() else {
            return Ok(None);
        };
        if normalized.path.is_none() {
            return Err(SlabTypeError::Validation {
                path: field.to_owned(),
                message: "non-empty asset ref requires a non-empty $path".to_owned(),
            });
        }
        Ok(Some(normalized))
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::AssetRef;

    #[test]
    fn empty_asset_ref_is_treated_as_unset() {
        let value = AssetRef::default();

        assert!(value.validate_configured("chat_template").unwrap().is_none());
    }

    #[test]
    fn non_empty_asset_ref_requires_path() {
        let value = AssetRef { id: Some("template-1".into()), name: None, path: None };

        let error = value.validate_configured("chat_template").unwrap_err();
        assert!(error.to_string().contains("$path"));
    }

    #[test]
    fn asset_ref_normalizes_blank_fields() {
        let value = AssetRef {
            id: Some("  ".into()),
            name: Some("  Example ".into()),
            path: Some(" ref://config/chat_template.jinja ".into()),
        };

        let normalized = value.validate_configured("chat_template").unwrap().expect("configured");
        assert_eq!(normalized.id, None);
        assert_eq!(normalized.name.as_deref(), Some("Example"));
        assert_eq!(normalized.path.as_deref(), Some("ref://config/chat_template.jinja"));
    }

    #[test]
    fn asset_ref_rejects_unknown_fields() {
        let error = serde_json::from_value::<AssetRef>(serde_json::json!({
            "id": "template-1",
            "$path": "ref://config/chat_template.jinja",
            "inline": "nope",
        }))
        .expect_err("unknown asset-ref field must fail");

        assert!(error.to_string().contains("unknown field"));
    }
}
