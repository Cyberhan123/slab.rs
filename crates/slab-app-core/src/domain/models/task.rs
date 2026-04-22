use crate::infra::db::TaskRecord;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct AcceptedOperation {
    pub operation_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn is_cancellable(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }

    pub fn is_restartable(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled | Self::Interrupted)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskPayloadEnvelope {
    pub kind: String,
    pub version: u32,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskProgress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub current: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TaskView {
    pub id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedTextSegment {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub image: Option<String>,
    pub images: Option<Vec<String>>,
    pub video_path: Option<String>,
    pub output_path: Option<String>,
    pub text: Option<String>,
    pub segments: Option<Vec<TimedTextSegment>>,
}

impl From<&TaskRecord> for TaskView {
    fn from(record: &TaskRecord) -> Self {
        Self {
            id: record.id.clone(),
            task_type: record.task_type.clone(),
            status: record.status,
            progress: task_progress_from_payload(record.result_data.as_deref()),
            error_msg: record.error_msg.clone(),
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}

fn task_progress_from_payload(raw: Option<&str>) -> Option<TaskProgress> {
    let raw = raw?;
    let payload: Value = serde_json::from_str(raw).ok()?;
    let progress = payload.get("progress")?;
    serde_json::from_value(progress.clone()).ok()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn task_view_reads_embedded_progress_payload() {
        let now = Utc::now();
        let record = TaskRecord {
            id: "task-1".to_owned(),
            task_type: "model_download".to_owned(),
            status: TaskStatus::Running,
            model_id: Some("model-a".to_owned()),
            input_data: None,
            result_data: Some(
                r#"{"progress":{"label":"model.gguf","current":512,"total":1024,"unit":"bytes","step":1,"step_count":2}}"#
                    .to_owned(),
            ),
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };

        let view = TaskView::from(&record);
        assert_eq!(
            view.progress,
            Some(TaskProgress {
                label: Some("model.gguf".to_owned()),
                current: 512,
                total: Some(1024),
                unit: Some("bytes".to_owned()),
                step: Some(1),
                step_count: Some(2),
            })
        );
    }

    #[test]
    fn task_view_ignores_non_progress_payloads() {
        let now = Utc::now();
        let record = TaskRecord {
            id: "task-1".to_owned(),
            task_type: "model_download".to_owned(),
            status: TaskStatus::Succeeded,
            model_id: Some("model-a".to_owned()),
            input_data: None,
            result_data: Some(r#"{"local_path":"C:/models/model.gguf"}"#.to_owned()),
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };

        assert!(TaskView::from(&record).progress.is_none());
    }
}
