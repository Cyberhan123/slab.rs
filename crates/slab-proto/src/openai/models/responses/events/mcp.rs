use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpCallArgumentsDeltaEvent {
    /// The type of the event. Always 'response.mcp_call_arguments.delta'.
    #[serde(rename = "type")]
    pub r#type: McpCallArgumentsDeltaType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the MCP tool call item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// A JSON string containing the partial update to the arguments for the MCP tool call.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpCallArgumentsDeltaEvent {
    /// Emitted when there is a delta (partial update) to the arguments of an MCP tool call.
    pub fn new(
        r#type: McpCallArgumentsDeltaType,
        output_index: i32,
        item_id: String,
        delta: String,
        sequence_number: i32,
    ) -> ResponseMcpCallArgumentsDeltaEvent {
        ResponseMcpCallArgumentsDeltaEvent { r#type, output_index, item_id, delta, sequence_number }
    }
}
pub mod mcp_call_arguments_delta_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_call_arguments.delta'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_call_arguments.delta")]
        #[default]
        ResponseMcpCallArgumentsDelta,
    }
    
}
pub use mcp_call_arguments_delta_type::Type as McpCallArgumentsDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpCallArgumentsDoneEvent {
    /// The type of the event. Always 'response.mcp_call_arguments.done'.
    #[serde(rename = "type")]
    pub r#type: McpCallArgumentsDoneType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the MCP tool call item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// A JSON string containing the finalized arguments for the MCP tool call.
    #[serde(rename = "arguments")]
    pub arguments: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpCallArgumentsDoneEvent {
    /// Emitted when the arguments for an MCP tool call are finalized.
    pub fn new(
        r#type: McpCallArgumentsDoneType,
        output_index: i32,
        item_id: String,
        arguments: String,
        sequence_number: i32,
    ) -> ResponseMcpCallArgumentsDoneEvent {
        ResponseMcpCallArgumentsDoneEvent {
            r#type,
            output_index,
            item_id,
            arguments,
            sequence_number,
        }
    }
}
pub mod mcp_call_arguments_done_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_call_arguments.done'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_call_arguments.done")]
        #[default]
        ResponseMcpCallArgumentsDone,
    }
    
}
pub use mcp_call_arguments_done_type::Type as McpCallArgumentsDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpCallCompletedEvent {
    /// The type of the event. Always 'response.mcp_call.completed'.
    #[serde(rename = "type")]
    pub r#type: McpCallCompletedType,
    /// The ID of the MCP tool call item that completed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that completed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpCallCompletedEvent {
    /// Emitted when an MCP  tool call has completed successfully.
    pub fn new(
        r#type: McpCallCompletedType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseMcpCallCompletedEvent {
        ResponseMcpCallCompletedEvent { r#type, item_id, output_index, sequence_number }
    }
}
pub mod mcp_call_completed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_call.completed'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_call.completed")]
        #[default]
        ResponseMcpCallCompleted,
    }
    
}
pub use mcp_call_completed_type::Type as McpCallCompletedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpCallFailedEvent {
    /// The type of the event. Always 'response.mcp_call.failed'.
    #[serde(rename = "type")]
    pub r#type: McpCallFailedType,
    /// The ID of the MCP tool call item that failed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that failed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpCallFailedEvent {
    /// Emitted when an MCP  tool call has failed.
    pub fn new(
        r#type: McpCallFailedType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseMcpCallFailedEvent {
        ResponseMcpCallFailedEvent { r#type, item_id, output_index, sequence_number }
    }
}
pub mod mcp_call_failed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_call.failed'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_call.failed")]
        #[default]
        ResponseMcpCallFailed,
    }
    
}
pub use mcp_call_failed_type::Type as McpCallFailedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpCallInProgressEvent {
    /// The type of the event. Always 'response.mcp_call.in_progress'.
    #[serde(rename = "type")]
    pub r#type: McpCallInProgressType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the MCP tool call item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
}

impl ResponseMcpCallInProgressEvent {
    /// Emitted when an MCP  tool call is in progress.
    pub fn new(
        r#type: McpCallInProgressType,
        sequence_number: i32,
        output_index: i32,
        item_id: String,
    ) -> ResponseMcpCallInProgressEvent {
        ResponseMcpCallInProgressEvent { r#type, sequence_number, output_index, item_id }
    }
}
pub mod mcp_call_in_progress_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_call.in_progress'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_call.in_progress")]
        #[default]
        ResponseMcpCallInProgress,
    }
    
}
pub use mcp_call_in_progress_type::Type as McpCallInProgressType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpListToolsCompletedEvent {
    /// The type of the event. Always 'response.mcp_list_tools.completed'.
    #[serde(rename = "type")]
    pub r#type: McpListToolsCompletedType,
    /// The ID of the MCP tool call item that produced this output.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that was processed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpListToolsCompletedEvent {
    /// Emitted when the list of available MCP tools has been successfully retrieved.
    pub fn new(
        r#type: McpListToolsCompletedType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseMcpListToolsCompletedEvent {
        ResponseMcpListToolsCompletedEvent { r#type, item_id, output_index, sequence_number }
    }
}
pub mod mcp_list_tools_completed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_list_tools.completed'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_list_tools.completed")]
        #[default]
        ResponseMcpListToolsCompleted,
    }
    
}
pub use mcp_list_tools_completed_type::Type as McpListToolsCompletedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpListToolsFailedEvent {
    /// The type of the event. Always 'response.mcp_list_tools.failed'.
    #[serde(rename = "type")]
    pub r#type: McpListToolsFailedType,
    /// The ID of the MCP tool call item that failed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that failed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpListToolsFailedEvent {
    /// Emitted when the attempt to list available MCP tools has failed.
    pub fn new(
        r#type: McpListToolsFailedType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseMcpListToolsFailedEvent {
        ResponseMcpListToolsFailedEvent { r#type, item_id, output_index, sequence_number }
    }
}
pub mod mcp_list_tools_failed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_list_tools.failed'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_list_tools.failed")]
        #[default]
        ResponseMcpListToolsFailed,
    }
    
}
pub use mcp_list_tools_failed_type::Type as McpListToolsFailedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseMcpListToolsInProgressEvent {
    /// The type of the event. Always 'response.mcp_list_tools.in_progress'.
    #[serde(rename = "type")]
    pub r#type: McpListToolsInProgressType,
    /// The ID of the MCP tool call item that is being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that is being processed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseMcpListToolsInProgressEvent {
    /// Emitted when the system is in the process of retrieving the list of available MCP tools.
    pub fn new(
        r#type: McpListToolsInProgressType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseMcpListToolsInProgressEvent {
        ResponseMcpListToolsInProgressEvent { r#type, item_id, output_index, sequence_number }
    }
}
pub mod mcp_list_tools_in_progress_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.mcp_list_tools.in_progress'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.mcp_list_tools.in_progress")]
        #[default]
        ResponseMcpListToolsInProgress,
    }
    
}
pub use mcp_list_tools_in_progress_type::Type as McpListToolsInProgressType;
