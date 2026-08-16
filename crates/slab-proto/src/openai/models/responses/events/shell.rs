use crate::openai::models;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// response.shell_call_command.added
// -----------------------------------------------------------------------

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ShellCallCommandAddedType {
    #[serde(rename = "response.shell_call_command.added")]
    #[default]
    ResponseShellCallCommandAdded,
}

/// Emitted when a shell command starts streaming. The `command` field begins
/// empty (`""`) and is filled in by the subsequent `delta` events.
///
/// Fixture shape (`openai-shell-{skills,container,tool}.1.chunks.txt`):
/// `{"type":"response.shell_call_command.added","command":"","command_index":0,
/// "output_index":0,"sequence_number":3}`.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseShellCallCommandAddedEvent {
    /// The type of the event. Always `response.shell_call_command.added`.
    #[serde(rename = "type")]
    pub r#type: ShellCallCommandAddedType,
    /// The command text. Empty string when the command begins streaming.
    #[serde(rename = "command")]
    pub command: String,
    /// The index of the command within the shell call action's `commands` array.
    #[serde(rename = "command_index")]
    pub command_index: i32,
    /// The index of the output item in the response that the command belongs to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseShellCallCommandAddedEvent {
    /// Emitted when a shell command begins streaming.
    pub fn new(
        r#type: ShellCallCommandAddedType,
        command: String,
        command_index: i32,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseShellCallCommandAddedEvent {
        ResponseShellCallCommandAddedEvent {
            r#type,
            command,
            command_index,
            output_index,
            sequence_number,
        }
    }
}

// ---------------------------------------------------------------------------
// response.shell_call_command.delta
// -----------------------------------------------------------------------

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ShellCallCommandDeltaType {
    #[serde(rename = "response.shell_call_command.delta")]
    #[default]
    ResponseShellCallCommandDelta,
}

/// Emitted for each partial chunk of a streaming shell command.
///
/// Fixture shape (`openai-shell-skills.1.chunks.txt`):
/// `{"type":"response.shell_call_command.delta","command_index":0,"delta":"ls",
/// "obfuscation":"kEEr7KRKL2XZg0","output_index":0,"sequence_number":4}`.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseShellCallCommandDeltaEvent {
    /// The type of the event. Always `response.shell_call_command.delta`.
    #[serde(rename = "type")]
    pub r#type: ShellCallCommandDeltaType,
    /// The index of the command within the shell call action's `commands` array.
    #[serde(rename = "command_index")]
    pub command_index: i32,
    /// The partial command text delta streamed by the model.
    #[serde(rename = "delta")]
    pub delta: String,
    /// An opaque token emitted alongside the delta used to align obfuscated
    /// command streaming across events.
    #[serde(rename = "obfuscation")]
    pub obfuscation: String,
    /// The index of the output item in the response that the command belongs to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseShellCallCommandDeltaEvent {
    /// Emitted when a partial command delta is streamed.
    pub fn new(
        r#type: ShellCallCommandDeltaType,
        command_index: i32,
        delta: String,
        obfuscation: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseShellCallCommandDeltaEvent {
        ResponseShellCallCommandDeltaEvent {
            r#type,
            command_index,
            delta,
            obfuscation,
            output_index,
            sequence_number,
        }
    }
}

// ---------------------------------------------------------------------------
// response.shell_call_command.done
// -----------------------------------------------------------------------

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ShellCallCommandDoneType {
    #[serde(rename = "response.shell_call_command.done")]
    #[default]
    ResponseShellCallCommandDone,
}

/// Emitted when a shell command is finalized.
///
/// Fixture shape (`openai-shell-container.1.chunks.txt`):
/// `{"type":"response.shell_call_command.done","command":"echo 'Hello from
/// container!' && uname -a","command_index":0,"output_index":0,
/// "sequence_number":13}`.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseShellCallCommandDoneEvent {
    /// The type of the event. Always `response.shell_call_command.done`.
    #[serde(rename = "type")]
    pub r#type: ShellCallCommandDoneType,
    /// The finalized command text.
    #[serde(rename = "command")]
    pub command: String,
    /// The index of the command within the shell call action's `commands` array.
    #[serde(rename = "command_index")]
    pub command_index: i32,
    /// The index of the output item in the response that the command belongs to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseShellCallCommandDoneEvent {
    /// Emitted when a shell command is finalized.
    pub fn new(
        r#type: ShellCallCommandDoneType,
        command: String,
        command_index: i32,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseShellCallCommandDoneEvent {
        ResponseShellCallCommandDoneEvent {
            r#type,
            command,
            command_index,
            output_index,
            sequence_number,
        }
    }
}

// ---------------------------------------------------------------------------
// response.shell_call_output_content.delta
// -----------------------------------------------------------------------

/// A partial chunk of shell call output content. Only the streams the fixture
/// emits are modeled; `stderr` is included as an optional companion to the
/// `stdout` stream so a stderr-bearing delta (should one arrive) round-trips
/// without adding a field the fixtures never emit on the wire (`stderr` is
/// skipped when `None`, preserving the `{"stdout":"..."}` fixture shape).
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellCallOutputContentDelta {
    /// The partial standard output that was captured.
    #[serde(rename = "stdout", skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// The partial standard error output that was captured.
    #[serde(rename = "stderr", skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

impl ShellCallOutputContentDelta {
    /// Build a shell call output content delta.
    pub fn new(stdout: Option<String>, stderr: Option<String>) -> ShellCallOutputContentDelta {
        ShellCallOutputContentDelta { stdout, stderr }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ShellCallOutputContentDeltaType {
    #[serde(rename = "response.shell_call_output_content.delta")]
    #[default]
    ResponseShellCallOutputContentDelta,
}

/// Emitted for each partial chunk of shell call output content.
///
/// Fixture shape (`openai-shell-skills.1.chunks.txt`):
/// `{"type":"response.shell_call_output_content.delta","command_index":0,
/// "delta":{"stdout":"..."},"item_id":"sho_...","output_index":1,
/// "sequence_number":39}`.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseShellCallOutputContentDeltaEvent {
    /// The type of the event. Always `response.shell_call_output_content.delta`.
    #[serde(rename = "type")]
    pub r#type: ShellCallOutputContentDeltaType,
    /// The index of the command within the shell call action's `commands` array.
    #[serde(rename = "command_index")]
    pub command_index: i32,
    /// The partial shell output content delta streamed by the environment.
    #[serde(rename = "delta")]
    pub delta: ShellCallOutputContentDelta,
    /// The ID of the shell call output item that the delta was added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item in the response that the output belongs to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseShellCallOutputContentDeltaEvent {
    /// Emitted when a partial shell call output content delta is streamed.
    pub fn new(
        r#type: ShellCallOutputContentDeltaType,
        command_index: i32,
        delta: ShellCallOutputContentDelta,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseShellCallOutputContentDeltaEvent {
        ResponseShellCallOutputContentDeltaEvent {
            r#type,
            command_index,
            delta,
            item_id,
            output_index,
            sequence_number,
        }
    }
}

// ---------------------------------------------------------------------------
// response.shell_call_output_content.done
// -----------------------------------------------------------------------

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ShellCallOutputContentDoneType {
    #[serde(rename = "response.shell_call_output_content.done")]
    #[default]
    ResponseShellCallOutputContentDone,
}

/// Emitted when shell call output content is finalized.
///
/// Fixture shape (`openai-shell-skills.1.chunks.txt`):
/// `{"type":"response.shell_call_output_content.done","command_index":0,
/// "item_id":"sho_...","output":[{"outcome":{"type":"exit","exit_code":0},
/// "stderr":"","stdout":"..."}],"output_index":1,"sequence_number":40}`.
///
/// The `output` element shape is identical to [`models::FunctionShellCallOutputContent`],
/// so that domain type is reused directly.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseShellCallOutputContentDoneEvent {
    /// The type of the event. Always `response.shell_call_output_content.done`.
    #[serde(rename = "type")]
    pub r#type: ShellCallOutputContentDoneType,
    /// The index of the command within the shell call action's `commands` array.
    #[serde(rename = "command_index")]
    pub command_index: i32,
    /// The ID of the shell call output item that is finalized.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The finalized shell call output content entries.
    #[serde(rename = "output")]
    pub output: Vec<models::FunctionShellCallOutputContent>,
    /// The index of the output item in the response that the output belongs to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseShellCallOutputContentDoneEvent {
    /// Emitted when shell call output content is finalized.
    pub fn new(
        r#type: ShellCallOutputContentDoneType,
        command_index: i32,
        item_id: String,
        output: Vec<models::FunctionShellCallOutputContent>,
        output_index: i32,
        sequence_number: i32,
    ) -> ResponseShellCallOutputContentDoneEvent {
        ResponseShellCallOutputContentDoneEvent {
            r#type,
            command_index,
            item_id,
            output,
            output_index,
            sequence_number,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each new event must serialize to JSON whose `type` field equals the
    /// fixture-derived event name, and a key payload field must round-trip.
    /// The adapter's downstream fixture tests validate end-to-end; this test
    /// guards the wire name + payload shape inside slab-proto.
    #[test]
    fn shell_call_command_events_round_trip() {
        let added = ResponseShellCallCommandAddedEvent::new(
            ShellCallCommandAddedType::ResponseShellCallCommandAdded,
            String::new(),
            0,
            0,
            3,
        );
        let added_value = serde_json::to_value(&added).expect("added serializes");
        assert_eq!(added_value["type"], "response.shell_call_command.added");
        assert_eq!(added_value["command_index"], 0);

        let delta = ResponseShellCallCommandDeltaEvent::new(
            ShellCallCommandDeltaType::ResponseShellCallCommandDelta,
            0,
            "ls".to_string(),
            "kEEr7KRKL2XZg0".to_string(),
            0,
            4,
        );
        let delta_value = serde_json::to_value(&delta).expect("delta serializes");
        assert_eq!(delta_value["type"], "response.shell_call_command.delta");
        assert_eq!(delta_value["delta"], "ls");
        assert_eq!(delta_value["obfuscation"], "kEEr7KRKL2XZg0");

        let done = ResponseShellCallCommandDoneEvent::new(
            ShellCallCommandDoneType::ResponseShellCallCommandDone,
            "ls -a ~/Desktop".to_string(),
            0,
            0,
            9,
        );
        let done_value = serde_json::to_value(&done).expect("done serializes");
        assert_eq!(done_value["type"], "response.shell_call_command.done");
        assert_eq!(done_value["command"], "ls -a ~/Desktop");
    }

    #[test]
    fn shell_call_output_content_events_round_trip() {
        let delta = ResponseShellCallOutputContentDeltaEvent::new(
            ShellCallOutputContentDeltaType::ResponseShellCallOutputContentDelta,
            0,
            ShellCallOutputContentDelta::new(Some("Hello\n".to_string()), None),
            "sho_abc".to_string(),
            1,
            39,
        );
        let delta_value = serde_json::to_value(&delta).expect("output delta serializes");
        assert_eq!(delta_value["type"], "response.shell_call_output_content.delta");
        assert_eq!(delta_value["delta"]["stdout"], "Hello\n");
        // stderr is None -> must be absent to match the fixture wire shape.
        assert!(delta_value["delta"].get("stderr").is_none());
        assert_eq!(delta_value["item_id"], "sho_abc");

        let done = ResponseShellCallOutputContentDoneEvent::new(
            ShellCallOutputContentDoneType::ResponseShellCallOutputContentDone,
            0,
            "sho_abc".to_string(),
            vec![models::FunctionShellCallOutputContent::new(
                "Hello\n".to_string(),
                String::new(),
                models::ShellCallOutcome::FunctionShellCallOutputExitOutcome(Box::new(
                    models::FunctionShellCallOutputExitOutcome::new(0),
                )),
            )],
            1,
            40,
        );
        let done_value = serde_json::to_value(&done).expect("output done serializes");
        assert_eq!(done_value["type"], "response.shell_call_output_content.done");
        assert_eq!(done_value["output"][0]["stdout"], "Hello\n");
        assert_eq!(done_value["output"][0]["outcome"]["type"], "exit");
        assert_eq!(done_value["output"][0]["outcome"]["exit_code"], 0);
    }
}
