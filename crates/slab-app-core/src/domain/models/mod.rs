mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod media_task;
mod model;
mod plugin;
mod pmid;
mod session;
mod settings;
mod setup;
mod subtitle;
mod system;
mod task;
mod ui_state;
mod video;
mod workspace;

pub use audio::{AudioTranscriptionCommand, TranscribeDecodeOptions, TranscribeVadOptions};
pub use backend::{BackendStatusQuery, BackendStatusView};
#[allow(unused_imports)]
pub use chat::StructuredOutputJsonSchema;
pub use chat::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatModelCapabilities,
    ChatModelOption, ChatModelSource, ChatReasoningEffort, ChatResultChoice, ChatStreamChunk,
    ChatStreamOptions, ChatVerbosity, CloudChatParams, CommonChatParams, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction, JsonOptions, LocalChatParams, StructuredOutput,
    TextCompletionCommand, TextCompletionOutput, TextCompletionResult, TextGenerationChunk,
    TextGenerationResponse, TextGenerationUsage, TextPromptTokensDetails, TextResultChoice,
    assistant_message_from_parts, assistant_message_from_text_response,
    deserialize_session_message, serialize_session_message,
};
pub use ffmpeg::FfmpegConvertCommand;
pub use image::{DecodedImageInput, ImageGenerationCommand, ImageGenerationMode};
pub use media_task::{
    AUDIO_TRANSCRIPTION_TASK_TYPE, AudioTranscriptionRequestData, AudioTranscriptionResultData,
    AudioTranscriptionTaskView, IMAGE_GENERATION_TASK_TYPE, ImageGenerationRequestData,
    ImageGenerationResultData, ImageGenerationTaskView, VIDEO_GENERATION_TASK_TYPE,
    VideoGenerationRequestData, VideoGenerationResultData, VideoGenerationTaskView,
};
pub use model::{
    AvailableModelsQuery, AvailableModelsView, CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
    CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION, CreateModelCommand, DeletedModelView,
    DownloadModelCommand, ListModelsFilter, ManagedModelBackendId, ModelConfigDocument,
    ModelConfigFieldScope, ModelConfigFieldView, ModelConfigOrigin, ModelConfigPresetOption,
    ModelConfigSectionView, ModelConfigSelectionView, ModelConfigSourceArtifact,
    ModelConfigSourceSummary, ModelConfigValueType, ModelConfigVariantOption,
    ModelEnhancementPresetOption, ModelEnhancementVariantOption, ModelEnhancementView,
    ModelLoadCommand, ModelPackSelection, ModelSpec, ModelStatus, Pricing, RuntimePresets,
    SelectedModelDownloadSource, StoredModelConfig, UnifiedModel, UnifiedModelKind,
    UnifiedModelStatus, UpdateModelCommand, UpdateModelConfigSelectionCommand,
    UpdateModelEnhancementCommand, default_model_capabilities, normalize_model_capabilities,
    validate_stored_model_config,
};
pub use plugin::{InstallPluginCommand, PluginView};
pub use pmid::PMID;
pub use session::{CreateSessionCommand, DeleteSessionView, SessionMessageView, SessionView};
pub use settings::{
    SettingPropertySchema, SettingPropertyView, SettingValidationErrorData, SettingValue,
    SettingValueType, SettingsDocumentView, SettingsSectionView, SettingsSubsectionView,
    UpdateSettingCommand, UpdateSettingOperation,
};
pub use setup::{CompleteSetupCommand, ComponentStatus, EnvironmentStatus};
pub use subtitle::{
    RenderSubtitleCommand, RenderSubtitleEntry, RenderSubtitleResult, SubtitleVariant,
};
pub use system::{GpuDeviceSnapshot, GpuStatusSnapshot};
pub(crate) use task::task_progress_from_payload;
pub use task::{
    AcceptedOperation, TaskPayloadEnvelope, TaskProgress, TaskResult, TaskStatus, TaskView,
    TimedTextSegment,
};
pub use ui_state::{DeleteUiStateView, UiStateValueView, UpdateUiStateCommand};
pub use video::{DecodedVideoInitImage, VideoGenerationCommand};
pub use workspace::{
    WorkspaceConsoleOutput, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileEntry,
    WorkspaceFileKind, WorkspaceFileSearchView, WorkspaceGitCommitCommand, WorkspaceGitDiffCommand,
    WorkspaceGitDiffView, WorkspaceGitFileStatus, WorkspaceGitOperationView,
    WorkspaceGitPathCommand, WorkspaceGitStatusEntry, WorkspaceGitStatusSummary,
    WorkspaceGitStatusView, WorkspacePathMetadata, WorkspacePathView, WorkspaceRenamePathCommand,
    WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
