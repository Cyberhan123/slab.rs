pub(crate) const DEFAULT_HF_ENDPOINT: &str = "https://huggingface.co";

#[derive(Debug, Clone)]
pub struct HubEndpoints {
    pub hf_endpoint: String,
    pub models_cat_endpoint: String,
}

impl Default for HubEndpoints {
    fn default() -> Self {
        Self {
            hf_endpoint: DEFAULT_HF_ENDPOINT.to_owned(),
            models_cat_endpoint: "https://www.modelscope.cn".to_owned(),
        }
    }
}
