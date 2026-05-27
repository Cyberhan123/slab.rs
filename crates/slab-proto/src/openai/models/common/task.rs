use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskGroupItem {
    /// Identifier of the thread item.
    #[serde(rename = "id")]
    pub id: String,
    /// Type discriminator that is always `chatkit.thread_item`.
    #[serde(rename = "object")]
    pub object: TaskGroupItemObject,
    /// Unix timestamp (in seconds) for when the item was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Identifier of the parent thread.
    #[serde(rename = "thread_id")]
    pub thread_id: String,
    /// Type discriminator that is always `chatkit.task_group`.
    #[serde(rename = "type")]
    pub r#type: TaskGroupItemType,
    /// Tasks included in the group.
    #[serde(rename = "tasks")]
    pub tasks: Vec<models::TaskGroupTask>,
}

impl TaskGroupItem {
    /// Collection of workflow tasks grouped together in the thread.
    pub fn new(
        id: String,
        object: TaskGroupItemObject,
        created_at: i32,
        thread_id: String,
        r#type: TaskGroupItemType,
        tasks: Vec<models::TaskGroupTask>,
    ) -> TaskGroupItem {
        TaskGroupItem { id, object, created_at, thread_id, r#type, tasks }
    }
}
/// Type discriminator that is always `chatkit.thread_item`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TaskGroupItemObject {
    #[serde(rename = "chatkit.thread_item")]
    #[default]
    ChatkitThreadItem,
}

/// Type discriminator that is always `chatkit.task_group`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TaskGroupItemType {
    #[serde(rename = "chatkit.task_group")]
    #[default]
    ChatkitTaskGroup,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskGroupTask {
    /// Subtype for the grouped task.
    #[serde(rename = "type")]
    pub r#type: models::TaskType,
    /// Optional heading for the grouped task. Defaults to null when not provided.
    #[serde(rename = "heading", deserialize_with = "Option::deserialize")]
    pub heading: Option<String>,
    /// Optional summary that describes the grouped task. Defaults to null when omitted.
    #[serde(rename = "summary", deserialize_with = "Option::deserialize")]
    pub summary: Option<String>,
}

impl TaskGroupTask {
    /// TaskGroupTask entry that appears within a TaskGroup.
    pub fn new(
        r#type: models::TaskType,
        heading: Option<String>,
        summary: Option<String>,
    ) -> TaskGroupTask {
        TaskGroupTask { r#type, heading, summary }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskItem {
    /// Identifier of the thread item.
    #[serde(rename = "id")]
    pub id: String,
    /// Type discriminator that is always `chatkit.thread_item`.
    #[serde(rename = "object")]
    pub object: TaskItemObject,
    /// Unix timestamp (in seconds) for when the item was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Identifier of the parent thread.
    #[serde(rename = "thread_id")]
    pub thread_id: String,
    /// Type discriminator that is always `chatkit.task`.
    #[serde(rename = "type")]
    pub r#type: TaskItemType,
    /// Subtype for the task.
    #[serde(rename = "task_type")]
    pub task_type: models::TaskType,
    /// Optional heading for the task. Defaults to null when not provided.
    #[serde(rename = "heading", deserialize_with = "Option::deserialize")]
    pub heading: Option<String>,
    /// Optional summary that describes the task. Defaults to null when omitted.
    #[serde(rename = "summary", deserialize_with = "Option::deserialize")]
    pub summary: Option<String>,
}

impl TaskItem {
    /// Task emitted by the workflow to show progress and status updates.
    pub fn new(
        id: String,
        object: TaskItemObject,
        created_at: i32,
        thread_id: String,
        r#type: TaskItemType,
        task_type: models::TaskType,
        heading: Option<String>,
        summary: Option<String>,
    ) -> TaskItem {
        TaskItem { id, object, created_at, thread_id, r#type, task_type, heading, summary }
    }
}
/// Type discriminator that is always `chatkit.thread_item`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TaskItemObject {
    #[serde(rename = "chatkit.thread_item")]
    #[default]
    ChatkitThreadItem,
}

/// Type discriminator that is always `chatkit.task`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum TaskItemType {
    #[serde(rename = "chatkit.task")]
    #[default]
    ChatkitTask,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum TaskType {
    #[serde(rename = "custom")]
    #[default]
    Custom,
    #[serde(rename = "thought")]
    Thought,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Custom => write!(f, "custom"),
            Self::Thought => write!(f, "thought"),
        }
    }
}

