use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical backend identifiers exposed on the runtime boundary today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackendId {
    GgmlLlama,
    GgmlWhisper,
    GgmlDiffusion,
}

impl RuntimeBackendId {
    /// Return the canonical backend identifier used by runtime and server wiring.
    pub const fn canonical_id(self) -> &'static str {
        match self {
            Self::GgmlLlama => "ggml.llama",
            Self::GgmlWhisper => "ggml.whisper",
            Self::GgmlDiffusion => "ggml.diffusion",
        }
    }

    /// Return the short human-friendly alias for the backend.
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::GgmlLlama => "llama",
            Self::GgmlWhisper => "whisper",
            Self::GgmlDiffusion => "diffusion",
        }
    }
}

impl fmt::Display for RuntimeBackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_id())
    }
}

impl FromStr for RuntimeBackendId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ggml.llama" | "llama" => Ok(Self::GgmlLlama),
            "ggml.whisper" | "whisper" => Ok(Self::GgmlWhisper),
            "ggml.diffusion" | "diffusion" => Ok(Self::GgmlDiffusion),
            other => Err(format!("unknown runtime backend id: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeBackendId;
    use std::str::FromStr;

    #[test]
    fn parses_canonical_backend_ids() {
        assert_eq!(
            RuntimeBackendId::from_str("ggml.llama").unwrap(),
            RuntimeBackendId::GgmlLlama
        );
        assert_eq!(
            RuntimeBackendId::from_str("ggml.whisper").unwrap(),
            RuntimeBackendId::GgmlWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("ggml.diffusion").unwrap(),
            RuntimeBackendId::GgmlDiffusion
        );
    }

    #[test]
    fn parses_short_backend_aliases() {
        assert_eq!(
            RuntimeBackendId::from_str("llama").unwrap(),
            RuntimeBackendId::GgmlLlama
        );
        assert_eq!(
            RuntimeBackendId::from_str("whisper").unwrap(),
            RuntimeBackendId::GgmlWhisper
        );
        assert_eq!(
            RuntimeBackendId::from_str("diffusion").unwrap(),
            RuntimeBackendId::GgmlDiffusion
        );
    }
}
