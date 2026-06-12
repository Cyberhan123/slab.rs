use crate::infra::db::TaskRecord;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{I18nPayload, ServerI18nKey};
use std::collections::BTreeMap;
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

    pub(crate) fn from_stored(raw: &str, context: &str) -> Self {
        raw.parse().unwrap_or_else(|_| {
            tracing::warn!(
                status = %raw,
                context,
                "unknown task status stored in repository; defaulting to failed"
            );
            Self::Failed
        })
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i18n: Option<I18nPayload>,
    pub current: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TaskView {
    pub id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub i18n: Option<I18nPayload>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedTextSegment {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
        let error_msg = record.error_msg.clone();
        Self {
            id: record.id.clone(),
            task_type: record.task_type.clone(),
            status: record.status,
            progress: task_progress_from_payload(record.result_data.as_deref()),
            error_msg: error_msg.clone(),
            i18n: task_error_i18n(&record.task_type, error_msg.as_deref()),
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn task_progress_from_payload(raw: Option<&str>) -> Option<TaskProgress> {
    let raw = raw?;
    let payload: Value = serde_json::from_str(raw).ok()?;
    let progress = payload.get("progress")?;
    serde_json::from_value(progress.clone()).ok()
}

fn task_error_i18n(task_type: &str, error_msg: Option<&str>) -> Option<I18nPayload> {
    let error_msg = error_msg?;
    let key = match task_type {
        "setup_provision" => ServerI18nKey::TaskSetupFailedBeforeFinish,
        "ffmpeg" => ServerI18nKey::TaskFfmpegConversionFailed,
        _ => return None,
    };
    Some(I18nPayload::with_field_params(
        "error_msg",
        key,
        BTreeMap::from([("detail".to_owned(), Value::String(error_msg.to_owned()))]),
    ))
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
                message: None,
                i18n: None,
                current: 512,
                total: Some(1024),
                unit: Some("bytes".to_owned()),
                step: Some(1),
                step_count: Some(2),
                logs: None,
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

    #[test]
    fn task_view_reads_progress_i18n_payload() {
        let now = Utc::now();
        let record = TaskRecord {
            id: "task-1".to_owned(),
            task_type: "setup_provision".to_owned(),
            status: TaskStatus::Running,
            model_id: None,
            input_data: None,
            result_data: Some(
                r#"{"progress":{"label":"Downloading payload","message":"Step detail","i18n":{"label":{"key":"server.tasks.setup.downloadingPayload","params":{"file_name":"runtime.cab"}},"message":{"key":"server.tasks.setup.checkingFfmpeg"}},"current":1,"total":2}}"#
                    .to_owned(),
            ),
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };

        let progress = TaskView::from(&record).progress.expect("progress payload");

        assert_eq!(progress.label.as_deref(), Some("Downloading payload"));
        assert_eq!(progress.message.as_deref(), Some("Step detail"));
        let i18n = progress.i18n.expect("progress i18n");
        assert_eq!(
            i18n.0.get("label").map(|message| message.key),
            Some(ServerI18nKey::TaskSetupDownloadingPayload)
        );
        assert_eq!(
            i18n.0.get("message").map(|message| message.key),
            Some(ServerI18nKey::TaskSetupCheckingFfmpeg)
        );
    }

    #[test]
    fn task_view_adds_i18n_for_known_task_errors() {
        let now = Utc::now();
        let record = TaskRecord {
            id: "task-1".to_owned(),
            task_type: "setup_provision".to_owned(),
            status: TaskStatus::Failed,
            model_id: None,
            input_data: None,
            result_data: None,
            error_msg: Some("download failed".to_owned()),
            core_task_id: None,
            created_at: now,
            updated_at: now,
        };

        let view = TaskView::from(&record);
        assert_eq!(view.error_msg.as_deref(), Some("download failed"));
        let i18n = view.i18n.expect("task error i18n");
        assert_eq!(
            i18n.0.get("error_msg").map(|message| message.key),
            Some(ServerI18nKey::TaskSetupFailedBeforeFinish)
        );
    }

    #[test]
    fn stored_unknown_task_status_defaults_to_failed() {
        assert_eq!(TaskStatus::from_stored("unknown", "test"), TaskStatus::Failed);
    }
}
