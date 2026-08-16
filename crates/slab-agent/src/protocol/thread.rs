//! Per-thread aggregate.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::turn::Turn;

#[derive(TS, Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitInfo {
    pub branch: String,
    pub sha: String,
    pub is_dirty: bool,
}

#[derive(TS, Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Thread {
    pub id: String,
    pub preview: String,
    pub model_provider: String,
    /// Unix epoch milliseconds.
    #[ts(type = "number")]
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_info: Option<GitInfo>,
    #[serde(default)]
    pub turns: Vec<Turn>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_serializes_camel_case_fields() {
        let thread = Thread {
            id: "t1".to_owned(),
            preview: "hi".to_owned(),
            model_provider: "openai".to_owned(),
            created_at: 1_700_000_000_000,
            turns: vec![],
            ..Default::default()
        };
        let json = serde_json::to_value(&thread).unwrap();
        assert_eq!(json["modelProvider"], "openai");
        assert_eq!(json["createdAt"], 1_700_000_000_000_i64);
    }
}
