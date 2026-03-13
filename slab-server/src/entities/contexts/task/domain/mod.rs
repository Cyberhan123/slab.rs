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
                | (TaskStatus::Pending, TaskStatus::Succeeded)
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

#[cfg(test)]
mod tests {
    use super::{TaskDomainError, TaskRecord, TaskStatus};
    use chrono::Utc;

    fn make_record(status: &str) -> TaskRecord {
        TaskRecord {
            id: "test-1".to_owned(),
            task_type: "image".to_owned(),
            status: status.to_owned(),
            model_id: None,
            input_data: None,
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn pending_to_running_allowed() {
        let mut r = make_record("pending");
        assert!(r.transition_to(TaskStatus::Running, None, None).is_ok());
        assert_eq!(r.status, "running");
    }

    #[test]
    fn pending_to_succeeded_allowed() {
        let mut r = make_record("pending");
        assert!(r.transition_to(TaskStatus::Succeeded, None, None).is_ok());
        assert_eq!(r.status, "succeeded");
    }

    #[test]
    fn pending_to_failed_allowed() {
        let mut r = make_record("pending");
        assert!(r
            .transition_to(TaskStatus::Failed, None, Some("err".to_owned()))
            .is_ok());
        assert_eq!(r.status, "failed");
    }

    #[test]
    fn pending_to_cancelled_allowed() {
        let mut r = make_record("pending");
        assert!(r.transition_to(TaskStatus::Cancelled, None, None).is_ok());
        assert_eq!(r.status, "cancelled");
    }

    #[test]
    fn pending_to_interrupted_allowed() {
        let mut r = make_record("pending");
        assert!(r.transition_to(TaskStatus::Interrupted, None, None).is_ok());
        assert_eq!(r.status, "interrupted");
    }

    #[test]
    fn running_to_succeeded_allowed() {
        let mut r = make_record("running");
        assert!(r
            .transition_to(TaskStatus::Succeeded, Some("result".to_owned()), None)
            .is_ok());
        assert_eq!(r.status, "succeeded");
        assert_eq!(r.result_data.as_deref(), Some("result"));
    }

    #[test]
    fn running_to_failed_allowed() {
        let mut r = make_record("running");
        assert!(r
            .transition_to(TaskStatus::Failed, None, Some("err".to_owned()))
            .is_ok());
        assert_eq!(r.status, "failed");
    }

    #[test]
    fn running_to_cancelled_allowed() {
        let mut r = make_record("running");
        assert!(r.transition_to(TaskStatus::Cancelled, None, None).is_ok());
        assert_eq!(r.status, "cancelled");
    }

    #[test]
    fn terminal_to_pending_allowed() {
        for terminal in &["failed", "interrupted", "cancelled"] {
            let mut r = make_record(terminal);
            assert!(
                r.transition_to(TaskStatus::Pending, None, None).is_ok(),
                "expected {terminal} -> pending to be allowed"
            );
            assert_eq!(r.status, "pending");
        }
    }

    #[test]
    fn succeeded_to_running_denied() {
        let mut r = make_record("succeeded");
        let err = r
            .transition_to(TaskStatus::Running, None, None)
            .unwrap_err();
        assert!(matches!(err, TaskDomainError::InvalidTransition { .. }));
    }

    #[test]
    fn pending_to_succeeded_updates_result_data() {
        let mut r = make_record("pending");
        r.transition_to(TaskStatus::Succeeded, Some("data".to_owned()), None)
            .unwrap();
        assert_eq!(r.result_data.as_deref(), Some("data"));
    }

    #[test]
    fn idempotent_same_status_allowed() {
        let mut r = make_record("running");
        assert!(r.transition_to(TaskStatus::Running, None, None).is_ok());
    }
}
