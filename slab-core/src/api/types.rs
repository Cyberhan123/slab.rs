use strum::{Display, EnumString};

#[derive(Debug, Display, EnumString)]
pub enum Event {
    /// Load the dynamic library (dylib only, no model).
    #[strum(serialize = "lib.load")]
    LoadLibrary,
    /// Reload / replace the dynamic library (drops current model and lib first).
    #[strum(serialize = "lib.reload")]
    ReloadLibrary,
    /// Load a model into the already-loaded library.
    #[strum(serialize = "model.load")]
    LoadModel,
    /// Unload the current model (keeps lib loaded).
    #[strum(serialize = "model.unload")]
    UnloadModel,
    #[strum(serialize = "inference")]
    Inference,
    #[strum(serialize = "inference.stream")]
    InferenceStream,
    #[strum(serialize = "inference.image")]
    InferenceImage,
}

#[derive(Debug, Display, EnumString)]
pub enum Backend {
    #[strum(serialize = "ggml.llama")]
    GGMLLama,
    #[strum(serialize = "ggml.whisper")]
    GGMLWhisper,
    #[strum(serialize = "ggml.diffusion")]
    GGMLDiffusion,
}