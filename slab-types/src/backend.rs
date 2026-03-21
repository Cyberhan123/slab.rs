use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SlabTypeError;

/// Canonical backend identifiers exposed on the runtime boundary today.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackendId {
    GgmlLlama,
    GgmlWhisper,
    GgmlDiffusion,
    CandleLlama,
    CandleWhisper,
    CandleDiffusion,
    Onnx,
}

impl RuntimeBackendId {
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
}

impl fmt::Display for RuntimeBackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_id())
    }
}

impl FromStr for RuntimeBackendId {
    type Err = SlabTypeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ggml.llama" | "ggml-llama" | "llama" => Ok(Self::GgmlLlama),
            "ggml.whisper" | "ggml-whisper" | "whisper" => Ok(Self::GgmlWhisper),
            "ggml.diffusion" | "ggml-diffusion" | "diffusion" => Ok(Self::GgmlDiffusion),
            "candle.llama" | "candle-llama" => Ok(Self::CandleLlama),
            "candle.whisper" | "candle-whisper" => Ok(Self::CandleWhisper),
            "candle.diffusion" | "candle-diffusion" => Ok(Self::CandleDiffusion),
            "onnx" => Ok(Self::Onnx),
            other => Err(SlabTypeError::Parse(format!("unknown runtime backend id: {other}"))),
        }
    }
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
        assert_eq!(
            RuntimeBackendId::from_str("ggml-llama").unwrap(),
            RuntimeBackendId::GgmlLlama
        );
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
    fn returns_parse_error_for_unknown_backend() {
        let err = RuntimeBackendId::from_str("unknown-backend").unwrap_err();
        assert!(err.to_string().contains("unknown runtime backend id"));
    }
}
