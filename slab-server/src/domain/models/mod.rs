mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod model;
pub mod pmid;
mod session;
mod settings;
mod settings_jsonschema;
mod setup;
mod system;
mod task;
mod video;

pub use audio::{AudioTranscriptionCommand, TranscribeDecodeOptions, TranscribeVadOptions};
pub use backend::{
    BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand, ReloadBackendLibCommand,
};
pub use chat::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatModelOption,
    ChatModelSource, ChatResultChoice, ChatStreamChunk, ConversationMessage,
};
pub use ffmpeg::FfmpegConvertCommand;
pub use image::{ImageGenerationCommand, ImageGenerationMode};
pub use model::{
    AvailableModelsQuery, AvailableModelsView, CreateModelCommand, DeletedModelView,
    DownloadModelCommand, ListModelsFilter, ModelCatalogItemView, ModelCatalogStatus,
    ModelLoadCommand, ModelStatus, UpdateModelCommand,
};
pub use pmid::{
    CHAT_PROVIDERS_PMID, DIFFUSION_CLIP_G_PATH_PMID, DIFFUSION_CLIP_L_PATH_PMID,
    DIFFUSION_FLASH_ATTN_PMID, DIFFUSION_KEEP_CLIP_ON_CPU_PMID, DIFFUSION_KEEP_VAE_ON_CPU_PMID,
    DIFFUSION_LORA_MODEL_DIR_PMID, DIFFUSION_MODEL_PATH_PMID, DIFFUSION_NUM_WORKERS_PMID,
    DIFFUSION_OFFLOAD_PARAMS_TO_CPU_PMID, DIFFUSION_T5XXL_PATH_PMID, DIFFUSION_TAESD_PATH_PMID,
    DIFFUSION_VAE_PATH_PMID, LLAMA_CONTEXT_LENGTH_PMID, LLAMA_NUM_WORKERS_PMID,
    MODEL_AUTO_UNLOAD_ENABLED_PMID, MODEL_AUTO_UNLOAD_IDLE_MINUTES_PMID, MODEL_CACHE_DIR_PMID,
    SETUP_BACKENDS_DIFFUSION_ASSET_PMID, SETUP_BACKENDS_DIFFUSION_TAG_PMID,
    SETUP_BACKENDS_DIR_PMID, SETUP_BACKENDS_LLAMA_ASSET_PMID, SETUP_BACKENDS_LLAMA_TAG_PMID,
    SETUP_BACKENDS_WHISPER_ASSET_PMID, SETUP_BACKENDS_WHISPER_TAG_PMID,
    SETUP_FFMPEG_AUTO_DOWNLOAD_PMID, SETUP_FFMPEG_DIR_PMID, SETUP_INITIALIZED_PMID,
    WHISPER_NUM_WORKERS_PMID,
};
pub use session::{CreateSessionCommand, SessionMessageView, SessionView};
pub use settings::{
    embedded_settings_schema, CloudProviderModelSettingValue, CloudProviderSettingValue,
    SettingDefinition, SettingPropertySchema, SettingPropertyView, SettingValidationErrorData,
    SettingValueType, SettingsDocumentView, SettingsSchema, SettingsSectionView,
    SettingsSubsectionView, SettingsValuesFile, UpdateSettingCommand, UpdateSettingOperation,
};
pub use setup::{CompleteSetupCommand, ComponentStatus, EnvironmentStatus};
pub use system::{GpuDeviceSnapshot, GpuStatusSnapshot};
pub use task::{AcceptedOperation, TaskResult, TaskView};
pub use video::VideoGenerationCommand;
