use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use slab_types::I18nPayload;
use utoipa::{PartialSchema, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(untagged)]
pub enum SettingValue {
    #[default]
    Null,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Array(Vec<SettingValue>),
    Object(BTreeMap<String, SettingValue>),
}

impl PartialSchema for SettingValue {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::schema::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::AnyValue)
            .into()
    }
}

impl ToSchema for SettingValue {}

impl SettingValue {
    pub fn into_json_value(self) -> Value {
        self.into()
    }
}

impl From<Value> for SettingValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Boolean(value),
            Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    Self::Integer(value)
                } else if let Some(value) =
                    value.as_u64().and_then(|value| i64::try_from(value).ok())
                {
                    Self::Integer(value)
                } else {
                    Self::Number(value.as_f64().unwrap_or_default())
                }
            }
            Value::String(value) => Self::String(value),
            Value::Array(values) => Self::Array(values.into_iter().map(Self::from).collect()),
            Value::Object(values) => Self::Object(
                values.into_iter().map(|(key, value)| (key, Self::from(value))).collect(),
            ),
        }
    }
}

impl From<SettingValue> for Value {
    fn from(value: SettingValue) -> Self {
        match value {
            SettingValue::Null => Value::Null,
            SettingValue::Boolean(value) => Value::Bool(value),
            SettingValue::Integer(value) => Value::Number(Number::from(value)),
            SettingValue::Number(value) => {
                Number::from_f64(value).map_or(Value::Null, Value::Number)
            }
            SettingValue::String(value) => Value::String(value),
            SettingValue::Array(values) => {
                Value::Array(values.into_iter().map(Value::from).collect())
            }
            SettingValue::Object(values) => Value::Object(
                values.into_iter().map(|(key, value)| (key, Value::from(value))).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    Boolean,
    Integer,
    #[default]
    String,
    Array,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SettingPropertySchema {
    #[serde(rename = "type")]
    pub value_type: SettingValueType,
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<Value>,
    #[serde(default)]
    pub default_value: SettingValue,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingPropertyView {
    pub pmid: String,
    pub label: String,
    #[serde(default)]
    pub description_md: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i18n: Option<I18nPayload>,
    pub editable: bool,
    pub schema: SettingPropertySchema,
    pub effective_value: SettingValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_value: Option<SettingValue>,
    pub is_overridden: bool,
    pub search_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSubsectionView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description_md: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i18n: Option<I18nPayload>,
    pub properties: Vec<SettingPropertyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsSectionView {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description_md: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i18n: Option<I18nPayload>,
    pub subsections: Vec<SettingsSubsectionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsDocumentView {
    pub schema_version: u32,
    pub settings_path: String,
    pub warnings: Vec<String>,
    pub sections: Vec<SettingsSectionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSettingOperation {
    Set,
    Unset,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettingCommand {
    pub op: UpdateSettingOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<SettingValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingValidationErrorData {
    #[serde(rename = "type")]
    pub error_type: String,
    pub pmid: String,
    pub path: String,
    pub message: String,
}
