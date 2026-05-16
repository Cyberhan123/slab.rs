use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use crate::error::SlabTypeError;

/// Canonical backend identifiers exposed on the runtime boundary today.
#[non_exhaustive]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    Display,
    EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(
    parse_err_ty = SlabTypeError,
    parse_err_fn = parse_runtime_backend_id_error
)]
pub enum RuntimeBackendId {
    #[strum(
        to_string = "ggml.llama",
        serialize = "ggml.llama",
        serialize = "ggml-llama",
        serialize = "llama",
        ascii_case_insensitive
    )]
    GgmlLlama,
    #[strum(
        to_string = "ggml.whisper",
        serialize = "ggml.whisper",
        serialize = "ggml-whisper",
        serialize = "whisper",
        ascii_case_insensitive
    )]
    GgmlWhisper,
    #[strum(
        to_string = "ggml.diffusion",
        serialize = "ggml.diffusion",
        serialize = "ggml-diffusion",
        serialize = "diffusion",
        ascii_case_insensitive
    )]
    GgmlDiffusion,
    #[strum(
        to_string = "candle.llama",
        serialize = "candle.llama",
        serialize = "candle-llama",
        ascii_case_insensitive
    )]
    CandleLlama,
    #[strum(
        to_string = "candle.whisper",
        serialize = "candle.whisper",
        serialize = "candle-whisper",
        ascii_case_insensitive
    )]
    CandleWhisper,
    #[strum(
        to_string = "candle.diffusion",
        serialize = "candle.diffusion",
        serialize = "candle-diffusion",
        ascii_case_insensitive
    )]
    CandleDiffusion,
    #[strum(to_string = "onnx", serialize = "onnx", ascii_case_insensitive)]
    Onnx,
}

impl RuntimeBackendId {
    /// Backends that are currently exposed as runnable by the app.
    ///
    /// Candle and ONNX identifiers remain parseable for compatibility, but they are not included
    /// here because those runtimes are not available in the current desktop distribution.
    pub const ALL: [Self; 3] = [Self::GgmlLlama, Self::GgmlWhisper, Self::GgmlDiffusion];

    /// Return the canonical backend identifier used by runtime and server wiring.
    pub const fn canonical_id(self) -> &'static str {
        match self {
            Self::GgmlLlama => "ggml.llama",
            Self::GgmlWhisper => "ggml.whisper",
            Self::GgmlDiffusion => "ggml.diffusion",
            Self::CandleLlama => "candle.llama",
            Self::CandleWhisper => "candle.whisper",
            Self::CandleDiffusion => "candle.diffusion",
            Self::Onnx => "onnx",
        }
    }

    /// Return the short human-friendly alias for the backend.
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::GgmlLlama => "ggml-llama",
            Self::GgmlWhisper => "ggml-whisper",
            Self::GgmlDiffusion => "ggml-diffusion",
            Self::CandleLlama => "candle-llama",
            Self::CandleWhisper => "candle-whisper",
            Self::CandleDiffusion => "candle-diffusion",
            Self::Onnx => "onnx",
        }
    }

    pub const fn is_runtime_worker_backend(self) -> bool {
        matches!(self, Self::GgmlLlama | Self::GgmlWhisper | Self::GgmlDiffusion)
    }
}

fn parse_runtime_backend_id_error(value: &str) -> SlabTypeError {
    let normalized = value.trim().to_ascii_lowercase();
    SlabTypeError::Parse(format!("unknown runtime backend id: {normalized}"))
}

#[cfg(test)]
mod tests {
    use super::RuntimeBackendId;
    use std::str::FromStr;

    #[test]
    fn parses_canonical_backend_ids() {
        assert_eq!(RuntimeBackendId::from_str("ggml.llama").unwrap(), RuntimeBackendId::GgmlLlama);
        assert_eq!(
            RuntimeBackendId::from_str("ggml.whisper").unwrap(),
            RuntimeBackendId::GgmlWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("ggml.diffusion").unwrap(),
            RuntimeBackendId::GgmlDiffusion
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle.llama").unwrap(),
            RuntimeBackendId::CandleLlama
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle.whisper").unwrap(),
            RuntimeBackendId::CandleWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle.diffusion").unwrap(),
            RuntimeBackendId::CandleDiffusion
        );
        assert_eq!(RuntimeBackendId::from_str("onnx").unwrap(), RuntimeBackendId::Onnx);
    }

    #[test]
    fn parses_short_backend_aliases() {
        assert_eq!(RuntimeBackendId::from_str("ggml-llama").unwrap(), RuntimeBackendId::GgmlLlama);
        assert_eq!(
            RuntimeBackendId::from_str("ggml-whisper").unwrap(),
            RuntimeBackendId::GgmlWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("ggml-diffusion").unwrap(),
            RuntimeBackendId::GgmlDiffusion
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle-llama").unwrap(),
            RuntimeBackendId::CandleLlama
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle-whisper").unwrap(),
            RuntimeBackendId::CandleWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("candle-diffusion").unwrap(),
            RuntimeBackendId::CandleDiffusion
        );
    }

    #[test]
    fn parses_legacy_unqualified_aliases() {
        // Legacy aliases map to GGML variants for backward compatibility.
        assert_eq!(RuntimeBackendId::from_str("llama").unwrap(), RuntimeBackendId::GgmlLlama);
        assert_eq!(RuntimeBackendId::from_str("whisper").unwrap(), RuntimeBackendId::GgmlWhisper);
        assert_eq!(
            RuntimeBackendId::from_str("diffusion").unwrap(),
            RuntimeBackendId::GgmlDiffusion
        );
    }

    #[test]
    fn exposes_available_backends_once() {
        assert_eq!(
            RuntimeBackendId::ALL,
            [
                RuntimeBackendId::GgmlLlama,
                RuntimeBackendId::GgmlWhisper,
                RuntimeBackendId::GgmlDiffusion,
            ]
        );
        assert!(!RuntimeBackendId::ALL.contains(&RuntimeBackendId::CandleLlama));
        assert!(!RuntimeBackendId::ALL.contains(&RuntimeBackendId::CandleWhisper));
        assert!(!RuntimeBackendId::ALL.contains(&RuntimeBackendId::CandleDiffusion));
        assert!(!RuntimeBackendId::ALL.contains(&RuntimeBackendId::Onnx));
    }

    #[test]
    fn returns_parse_error_for_unknown_backend() {
        let err = RuntimeBackendId::from_str("unknown-backend").unwrap_err();
        assert!(err.to_string().contains("unknown runtime backend id"));
    }
}
