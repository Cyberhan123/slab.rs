use std::str::FromStr;

use crate::endpoints::{DEFAULT_HF_ENDPOINT, HubEndpoints};
use strum::{Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[strum(parse_err_ty = String, parse_err_fn = parse_hub_provider_error)]
pub enum HubProvider {
    #[strum(
        to_string = "hf_hub",
        serialize = "hf_hub",
        serialize = "hf",
        serialize = "huggingface",
        ascii_case_insensitive
    )]
    HfHub,
    #[strum(
        to_string = "models_cat",
        serialize = "models_cat",
        serialize = "modelscope",
        ascii_case_insensitive
    )]
    ModelsCat,
    #[strum(
        to_string = "huggingface_hub_rust",
        serialize = "huggingface_hub_rust",
        serialize = "huggingface_hub",
        ascii_case_insensitive
    )]
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

fn parse_hub_provider_error(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    format!(
        "unsupported hub provider '{normalized}'; expected one of hf_hub, models_cat, huggingface_hub_rust"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HubProviderPreference {
    #[default]
    Auto,
    Provider(HubProvider),
}

impl HubProviderPreference {
    pub fn from_optional_str(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::Auto),
            Some(value) if value.eq_ignore_ascii_case("auto") => Ok(Self::Auto),
            Some(value) => HubProvider::from_str(value).map(Self::Provider),
        }
    }
}
