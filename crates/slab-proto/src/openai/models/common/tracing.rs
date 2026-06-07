use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TracingConfiguration {
    /// Default tracing mode for the session.
    String(String),
    TracingConfiguration(Box<models::TracingConfiguration>),
}

impl Default for TracingConfiguration {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

pub type TracingConfiguration1 = TracingConfiguration;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TracingConfiguration2 {
    /// Enables tracing and sets default values for tracing configuration options. Always `auto`.
    Auto(String),
    TracingConfiguration2(Box<models::TracingConfiguration2>),
}

impl Default for TracingConfiguration2 {
    fn default() -> Self {
        Self::Auto(Default::default())
    }
}

pub type TracingConfiguration3 = TracingConfiguration2;
