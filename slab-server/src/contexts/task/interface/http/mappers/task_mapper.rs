use crate::contexts::task::domain::TaskResult;
use crate::entities::TaskRecord;
use crate::schemas::v1::task::{TaskResponse, TaskResultPayload};

pub fn to_task_response(record: &TaskRecord) -> TaskResponse {
    TaskResponse {
        id: record.id.clone(),
        task_type: record.task_type.clone(),
        status: record.status.clone(),
        error_msg: record.error_msg.clone(),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

pub fn to_task_result_response(result: TaskResult) -> TaskResultPayload {
    TaskResultPayload {
        image: result.image,
        images: result.images,
        video_path: result.video_path,
        text: result.text,
    }
}
