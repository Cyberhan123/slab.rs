use strum::{Display, EnumString};

#[derive(Debug, Display, EnumString)]
pub enum Event {
    #[strum(serialize = "lib.load")]
    LoadLibrary,
    #[strum(serialize = "lib.reload")]
    UnloadLibrary,
    #[strum(serialize = "model.load")]
    LoadModel,
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
    GGMLLLama,
    #[strum(serialize = "ggml.whisper")]
    GGMLWhisper,
    #[strum(serialize = "ggml.diffusion")]
    GGMLDiffusion,
}