use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct VadConfig {
    /// Must be set to `server_vad` to enable manual chunking using server side VAD.
    #[serde(rename = "type")]
    pub r#type: VadConfigType,
    /// Amount of audio to include before the VAD detected speech (in  milliseconds).
    #[serde(rename = "prefix_padding_ms", skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<i32>,
    /// Duration of silence to detect speech stop (in milliseconds). With shorter values the model will respond more quickly,  but may jump in on short pauses from the user.
    #[serde(rename = "silence_duration_ms", skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<i32>,
    /// Sensitivity threshold (0.0 to 1.0) for voice activity detection. A  higher threshold will require louder audio to activate the model, and  thus might perform better in noisy environments.
    #[serde(rename = "threshold", skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
}

impl VadConfig {
    pub fn new(r#type: VadConfigType) -> VadConfig {
        VadConfig { r#type, prefix_padding_ms: None, silence_duration_ms: None, threshold: None }
    }
}
/// Must be set to `server_vad` to enable manual chunking using server side VAD.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum VadConfigType {
    #[serde(rename = "server_vad")]
    ServerVad,
}

impl Default for VadConfigType {
    fn default() -> VadConfigType {
        Self::ServerVad
    }
}

use super::misc::Eagerness;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticVad {
    /// VadConfigType of turn detection, `semantic_vad` to turn on Semantic VAD.
    #[serde(rename = "type")]
    pub r#type: SemanticVadType,
    /// Used only for `semantic_vad` mode. The eagerness of the model to respond. `low` will wait longer for the user to continue speaking, `high` will respond more quickly. `auto` is the default and is equivalent to `medium`. `low`, `medium`, and `high` have max timeouts of 8s, 4s, and 2s respectively.
    #[serde(rename = "eagerness", skip_serializing_if = "Option::is_none")]
    pub eagerness: Option<Eagerness>,
    /// Whether or not to automatically generate a response when a VAD stop event occurs.
    #[serde(rename = "create_response", skip_serializing_if = "Option::is_none")]
    pub create_response: Option<bool>,
    /// Whether or not to automatically interrupt any ongoing response with output to the default conversation (i.e. `conversation` of `auto`) when a VAD start event occurs.
    #[serde(rename = "interrupt_response", skip_serializing_if = "Option::is_none")]
    pub interrupt_response: Option<bool>,
}

impl SemanticVad {
    /// Server-side semantic turn detection which uses a model to determine when the user has finished speaking.
    pub fn new(r#type: SemanticVadType) -> SemanticVad {
        SemanticVad { r#type, eagerness: None, create_response: None, interrupt_response: None }
    }
}
/// Type of turn detection, `semantic_vad` to turn on Semantic VAD.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum SemanticVadType {
    #[serde(rename = "semantic_vad")]
    SemanticVad,
}

impl Default for SemanticVadType {
    fn default() -> SemanticVadType {
        Self::SemanticVad
    }
}
// Used only for `semantic_vad` mode. The eagerness of the model to respond. `low` will wait longer for the user to continue speaking, `high` will respond more quickly. `auto` is the default and is equivalent to `medium`. `low`, `medium`, and `high` have max timeouts of 8s, 4s, and 2s respectively.

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerVad {
    /// SemanticVadType of turn detection, `server_vad` to turn on simple Server VAD.
    #[serde(rename = "type")]
    pub r#type: ServerVadType,
    /// Used only for `server_vad` mode. Activation threshold for VAD (0.0 to 1.0), this defaults to 0.5. A higher threshold will require louder audio to activate the model, and thus might perform better in noisy environments.
    #[serde(rename = "threshold", skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Used only for `server_vad` mode. Amount of audio to include before the VAD detected speech (in milliseconds). Defaults to 300ms.
    #[serde(rename = "prefix_padding_ms", skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<i32>,
    /// Used only for `server_vad` mode. Duration of silence to detect speech stop (in milliseconds). Defaults to 500ms. With shorter values the model will respond more quickly, but may jump in on short pauses from the user.
    #[serde(rename = "silence_duration_ms", skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<i32>,
    /// Whether or not to automatically generate a response when a VAD stop event occurs. If `interrupt_response` is set to `false` this may fail to create a response if the model is already responding.  If both `create_response` and `interrupt_response` are set to `false`, the model will never respond automatically but VAD events will still be emitted.
    #[serde(rename = "create_response", skip_serializing_if = "Option::is_none")]
    pub create_response: Option<bool>,
    /// Whether or not to automatically interrupt (cancel) any ongoing response with output to the default conversation (i.e. `conversation` of `auto`) when a VAD start event occurs. If `true` then the response will be cancelled, otherwise it will continue until complete.  If both `create_response` and `interrupt_response` are set to `false`, the model will never respond automatically but VAD events will still be emitted.
    #[serde(rename = "interrupt_response", skip_serializing_if = "Option::is_none")]
    pub interrupt_response: Option<bool>,
    /// Optional timeout after which a model response will be triggered automatically. This is useful for situations in which a long pause from the user is unexpected, such as a phone call. The model will effectively prompt the user to continue the conversation based on the current context.  The timeout value will be applied after the last model response's audio has finished playing, i.e. it's set to the `response.done` time plus audio playback duration.  An `input_audio_buffer.timeout_triggered` event (plus events associated with the Response) will be emitted when the timeout is reached. Idle timeout is currently only supported for `server_vad` mode.
    #[serde(
        rename = "idle_timeout_ms",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub idle_timeout_ms: Option<Option<i32>>,
}

impl ServerVad {
    /// Server-side voice activity detection (VAD) which flips on when user speech is detected and off after a period of silence.
    pub fn new(r#type: ServerVadType) -> ServerVad {
        ServerVad {
            r#type,
            threshold: None,
            prefix_padding_ms: None,
            silence_duration_ms: None,
            create_response: None,
            interrupt_response: None,
            idle_timeout_ms: None,
        }
    }
}
/// Type of turn detection, `server_vad` to turn on simple Server VAD.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ServerVadType {
    #[serde(rename = "server_vad")]
    ServerVad,
}

impl Default for ServerVadType {
    fn default() -> ServerVadType {
        Self::ServerVad
    }
}
