mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod model;
mod pmid;
mod session;
mod settings;
mod settings_jsonschema;
mod setup;
mod system;
mod task;
mod video;

pub use audio::{AudioTranscriptionCommand, TranscribeDecodeOptions, TranscribeVadOptions};
pub use backend::{BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand};
#[allow(unused_imports)]
pub use chat::StructuredOutputJsonSchema;
pub use chat::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatModelCapabilities,
    ChatModelOption, ChatModelSource, ChatReasoningEffort, ChatResultChoice, ChatStreamChunk,
    ChatStreamOptions, ChatVerbosity, CloudChatParams, CommonChatParams, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction, LocalChatParams, StructuredOutput, TextCompletionCommand,
    TextCompletionOutput, TextCompletionResult, TextResultChoice, assistant_message_from_parts,
    assistant_message_from_text_response, deserialize_session_message, serialize_session_message,
};
pub use ffmpeg::FfmpegConvertCommand;
pub use image::{DecodedImageInput, ImageGenerationCommand, ImageGenerationMode};
pub use model::{
    AvailableModelsQuery, AvailableModelsView, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
    CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION, CreateModelCommand, DeletedModelView,
    DownloadModelCommand, ListModelsFilter, ManagedModelBackendId, ModelConfigDocument,
    ModelConfigFieldScope, ModelConfigFieldView, ModelConfigOrigin, ModelConfigPresetOption,
    ModelConfigSectionView, ModelConfigSelectionView, ModelConfigSourceArtifact,
    ModelConfigSourceSummary, ModelConfigValueType, ModelConfigVariantOption,
    ModelEnhancementPresetOption, ModelEnhancementVariantOption, ModelEnhancementView,
    ModelLoadCommand, ModelPackSelection, ModelSpec, ModelStatus, Pricing, RuntimePresets,
    StoredModelConfig, UnifiedModel, UnifiedModelKind, UnifiedModelStatus, UpdateModelCommand,
    UpdateModelConfigSelectionCommand, UpdateModelEnhancementCommand, default_model_capabilities,
    normalize_model_capabilities, upgrade_stored_model_config,
};
pub use pmid::PMID;
pub use session::{CreateSessionCommand, SessionMessageView, SessionView};
pub use settings::{
    SettingDefinition, SettingPropertySchema, SettingPropertyView, SettingValidationErrorData,
    SettingValueType, SettingsDocumentView, SettingsSchema, SettingsSectionView,
    SettingsSubsectionView, SettingsValuesFile, UpdateSettingCommand, UpdateSettingOperation,
    embedded_settings_schema,
};
pub use setup::{CompleteSetupCommand, ComponentStatus, EnvironmentStatus};
pub use system::{GpuDeviceSnapshot, GpuStatusSnapshot};
pub use task::{AcceptedOperation, TaskPayloadEnvelope, TaskResult, TaskStatus, TaskView};
pub use video::{DecodedVideoInitImage, VideoGenerationCommand};
