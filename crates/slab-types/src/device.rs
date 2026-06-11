use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::SlabTypeError;

/// Runtime device preference passed across model load boundaries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub enum RuntimeDevicePreference {
    #[default]
    Auto,
    Cpu,
    Cuda {
        ordinal: usize,
    },
    Metal {
        ordinal: usize,
    },
}

impl std::fmt::Display for RuntimeDevicePreference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Cpu => f.write_str("cpu"),
            Self::Cuda { ordinal } => write!(f, "cuda:{ordinal}"),
            Self::Metal { ordinal } => write!(f, "metal:{ordinal}"),
        }
    }
}

impl FromStr for RuntimeDevicePreference {
    type Err = SlabTypeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "auto" => return Ok(Self::Auto),
            "cpu" => return Ok(Self::Cpu),
            "cuda" => return Ok(Self::Cuda { ordinal: 0 }),
            "metal" => return Ok(Self::Metal { ordinal: 0 }),
            _ => {}
        }

        if let Some(raw_ordinal) = normalized.strip_prefix("cuda:") {
            return Ok(Self::Cuda { ordinal: parse_ordinal(raw_ordinal, value)? });
        }
        if let Some(raw_ordinal) = normalized.strip_prefix("metal:") {
            return Ok(Self::Metal { ordinal: parse_ordinal(raw_ordinal, value)? });
        }

        Err(SlabTypeError::Parse(format!(
            "invalid runtime device preference '{value}'; expected auto, cpu, cuda[:ordinal], or metal[:ordinal]"
        )))
    }
}

impl TryFrom<String> for RuntimeDevicePreference {
    type Error = SlabTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl From<RuntimeDevicePreference> for String {
    fn from(value: RuntimeDevicePreference) -> Self {
        value.to_string()
    }
}

fn parse_ordinal(raw: &str, original: &str) -> Result<usize, SlabTypeError> {
    if raw.trim().is_empty() {
        return Err(SlabTypeError::Parse(format!(
            "invalid runtime device preference '{original}'; device ordinal must not be empty"
        )));
    }
    raw.parse::<usize>().map_err(|error| {
        SlabTypeError::Parse(format!(
            "invalid runtime device preference '{original}'; device ordinal is invalid: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::RuntimeDevicePreference;
    use std::str::FromStr;

    #[test]
    fn parses_runtime_device_preferences() {
        assert_eq!(
            RuntimeDevicePreference::from_str("auto").unwrap(),
            RuntimeDevicePreference::Auto
        );
        assert_eq!(RuntimeDevicePreference::from_str("cpu").unwrap(), RuntimeDevicePreference::Cpu);
        assert_eq!(
            RuntimeDevicePreference::from_str("cuda").unwrap(),
            RuntimeDevicePreference::Cuda { ordinal: 0 }
        );
        assert_eq!(
            RuntimeDevicePreference::from_str("cuda:1").unwrap(),
            RuntimeDevicePreference::Cuda { ordinal: 1 }
        );
        assert_eq!(
            RuntimeDevicePreference::from_str("metal").unwrap(),
            RuntimeDevicePreference::Metal { ordinal: 0 }
        );
        assert_eq!(
            RuntimeDevicePreference::from_str("metal:2").unwrap(),
            RuntimeDevicePreference::Metal { ordinal: 2 }
        );
    }

    #[test]
    fn rejects_invalid_runtime_device_preferences() {
        assert!(RuntimeDevicePreference::from_str("cuda:").is_err());
        assert!(RuntimeDevicePreference::from_str("gpu").is_err());
    }
}
