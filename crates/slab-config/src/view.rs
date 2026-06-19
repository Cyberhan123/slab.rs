use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Number, Value};
use slab_types::I18nPayload;
use utoipa::{PartialSchema, ToSchema};

use crate::ConfigError;

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(untagged)]
pub enum SettingValue {
    #[default]
    Null,
    Boolean(bool),
    Integer(i64),
    Unsigned(u64),
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

impl Serialize for SettingValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_none(),
            Self::Boolean(value) => serializer.serialize_bool(*value),
            Self::Integer(value) => serializer.serialize_i64(*value),
            Self::Unsigned(value) => serializer.serialize_u64(*value),
            Self::Number(value) if value.is_finite() => serializer.serialize_f64(*value),
            Self::Number(_) => Err(serde::ser::Error::custom("setting value must be finite")),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => values.serialize(serializer),
            Self::Object(values) => {
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (key, value) in values {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl SettingValue {
    pub fn into_json_value(self) -> Value {
        self.try_into_json_value().unwrap_or(Value::Null)
    }

    pub fn try_into_json_value(self) -> Result<Value, ConfigError> {
        match self {
            Self::Null => Ok(Value::Null),
            Self::Boolean(value) => Ok(Value::Bool(value)),
            Self::Integer(value) => Ok(Value::Number(Number::from(value))),
            Self::Unsigned(value) => Ok(Value::Number(Number::from(value))),
            Self::Number(value) => Number::from_f64(value)
                .map(Value::Number)
                .ok_or_else(|| ConfigError::BadRequest("setting value must be finite".to_owned())),
            Self::String(value) => Ok(Value::String(value)),
            Self::Array(values) => values
                .into_iter()
                .map(Self::try_into_json_value)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Self::Object(values) => values
                .into_iter()
                .map(|(key, value)| value.try_into_json_value().map(|value| (key, value)))
                .collect::<Result<serde_json::Map<_, _>, _>>()
                .map(Value::Object),
        }
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
                } else if let Some(value) = value.as_u64() {
                    Self::Unsigned(value)
                } else {
                    value.as_f64().map(Self::Number).unwrap_or_default()
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
        value.try_into_json_value().expect("setting value must be finite")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    Boolean,
    Integer,
    Unsigned,
    Float,
    #[default]
    String,
    Array,
    Object,
    TaggedUnion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingChangeEffect {
    #[default]
    None,
    Live,
    NeedsRestart,
    NeedsModelReload,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SettingOverrideSource {
    Env { var_name: String, var_value_present: bool },
    Parent { pmid: String },
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
    #[serde(default)]
    pub change_effect: SettingChangeEffect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<SettingOverrideSource>,
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
