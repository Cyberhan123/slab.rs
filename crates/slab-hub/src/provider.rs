use std::str::FromStr;

use crate::endpoints::{DEFAULT_HF_ENDPOINT, HubEndpoints};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HubProvider {
    HfHub,
    ModelsCat,
    HuggingfaceHubRust,
}

impl HubProvider {
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::HfHub => "hf_hub",
            Self::ModelsCat => "models_cat",
            Self::HuggingfaceHubRust => "huggingface_hub_rust",
        }
    }

    pub(crate) fn base_url(self, endpoints: &HubEndpoints) -> &str {
        match self {
            Self::HfHub => endpoints.hf_endpoint.as_str(),
            Self::HuggingfaceHubRust => DEFAULT_HF_ENDPOINT,
            Self::ModelsCat => endpoints.models_cat_endpoint.as_str(),
        }
    }
}

impl std::fmt::Display for HubProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_config_value())
    }
}

impl FromStr for HubProvider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "hf" | "hf_hub" | "huggingface" => Ok(Self::HfHub),
            "models_cat" | "modelscope" => Ok(Self::ModelsCat),
            "huggingface_hub_rust" | "huggingface_hub" => Ok(Self::HuggingfaceHubRust),
            other => Err(format!(
                "unsupported hub provider '{other}'; expected one of hf_hub, models_cat, huggingface_hub_rust"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HubProviderPreference {
    #[default]
    Auto,
    Provider(HubProvider),
}

impl HubProviderPreference {
    pub fn from_optional_str(value: Option<&str>) -> Result<Self, String> {
        match value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase())
        {
            None => Ok(Self::Auto),
            Some(value) if value == "auto" => Ok(Self::Auto),
            Some(value) => HubProvider::from_str(&value).map(Self::Provider),
        }
    }
}
