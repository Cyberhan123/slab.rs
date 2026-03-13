use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Interrupted,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "interrupted" => Some(Self::Interrupted),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TaskDomainError {
    #[error("invalid task status transition from {from} to {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub model_id: Option<String>,
    pub input_data: Option<String>,
    pub result_data: Option<String>,
    pub error_msg: Option<String>,
    pub core_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskRecord {
    pub fn transition_to(
        &mut self,
        target: TaskStatus,
        result_data: Option<String>,
        error_msg: Option<String>,
    ) -> Result<(), TaskDomainError> {
        let current = TaskStatus::parse(&self.status).unwrap_or(TaskStatus::Pending);
        let allowed = matches!(
            (current, target),
            (TaskStatus::Pending, TaskStatus::Running)
                | (TaskStatus::Pending, TaskStatus::Failed)
                | (TaskStatus::Pending, TaskStatus::Interrupted)
                | (TaskStatus::Pending, TaskStatus::Cancelled)
                | (TaskStatus::Running, TaskStatus::Succeeded)
                | (TaskStatus::Running, TaskStatus::Failed)
                | (TaskStatus::Running, TaskStatus::Interrupted)
                | (TaskStatus::Running, TaskStatus::Cancelled)
                | (TaskStatus::Failed, TaskStatus::Pending)
                | (TaskStatus::Interrupted, TaskStatus::Pending)
                | (TaskStatus::Cancelled, TaskStatus::Pending)
        );

        if !allowed && current != target {
            return Err(TaskDomainError::InvalidTransition {
                from: current.as_str(),
                to: target.as_str(),
            });
        }

        self.status = target.as_str().to_owned();
        self.result_data = result_data;
        self.error_msg = error_msg;
        self.updated_at = Utc::now();
        Ok(())
    }
}
