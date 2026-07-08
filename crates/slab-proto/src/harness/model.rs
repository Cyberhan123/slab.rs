//! Model discovery (`model/list`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::harness::messages::ReasoningEffort;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelListResult {
    pub data: Vec<ModelInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    pub default_reasoning_effort: ReasoningEffort,
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningEffortOption {
    pub reasoning_effort: ReasoningEffort,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_info_round_trips() {
        let info = ModelInfo {
            id: "gpt-oss:latest".to_owned(),
            model: "gpt-oss".to_owned(),
            display_name: "GPT-OSS".to_owned(),
            description: "open weights".to_owned(),
            supported_reasoning_efforts: vec![ReasoningEffortOption {
                reasoning_effort: ReasoningEffort::Medium,
                description: "default".to_owned(),
            }],
            default_reasoning_effort: ReasoningEffort::Medium,
            is_default: true,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["displayName"], "GPT-OSS");
        assert_eq!(json["supportedReasoningEfforts"][0]["reasoningEffort"], "medium");
        let back: ModelInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info, back);
    }
}
