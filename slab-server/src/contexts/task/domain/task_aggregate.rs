use crate::entities::TaskRecord;

#[derive(Debug, Clone)]
pub struct TaskAggregate {
    record: TaskRecord,
}

impl TaskAggregate {
    pub fn from_record(record: TaskRecord) -> Self {
        Self { record }
    }

    pub fn record(&self) -> &TaskRecord {
        &self.record
    }

    pub fn into_record(self) -> TaskRecord {
        self.record
    }

    pub fn is_cancellable(&self) -> bool {
        matches!(self.record.status.as_str(), "pending" | "running")
    }

    pub fn is_restartable(&self) -> bool {
        matches!(
            self.record.status.as_str(),
            "failed" | "cancelled" | "interrupted"
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::TaskAggregate;
    use crate::entities::TaskRecord;

    fn make_record(status: &str) -> TaskRecord {
        TaskRecord {
            id: "task-1".to_owned(),
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
    fn cancellable_statuses_follow_domain_rules() {
        assert!(TaskAggregate::from_record(make_record("pending")).is_cancellable());
        assert!(TaskAggregate::from_record(make_record("running")).is_cancellable());
        assert!(!TaskAggregate::from_record(make_record("succeeded")).is_cancellable());
    }

    #[test]
    fn restartable_statuses_follow_domain_rules() {
        assert!(TaskAggregate::from_record(make_record("failed")).is_restartable());
        assert!(TaskAggregate::from_record(make_record("cancelled")).is_restartable());
        assert!(TaskAggregate::from_record(make_record("interrupted")).is_restartable());
        assert!(!TaskAggregate::from_record(make_record("running")).is_restartable());
    }
}
