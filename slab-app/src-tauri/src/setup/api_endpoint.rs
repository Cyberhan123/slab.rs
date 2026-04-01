#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEndpointConfig {
    pub gateway_bind: String,
    pub api_origin: String,
    pub connect_src: Vec<String>,
}

impl ApiEndpointConfig {
    pub fn desktop() -> Self {
        let api_origin = "http://127.0.0.1:3000".to_owned();
        Self {
            gateway_bind: "127.0.0.1:3000".to_owned(),
            connect_src: vec!["'self'".to_owned(), api_origin.clone()],
            api_origin,
        }
    }

    pub fn api_base_url(&self) -> String {
        format!("{}/", self.api_origin.trim_end_matches('/'))
    }

    pub fn health_url(&self) -> String {
        format!("{}/health", self.api_origin.trim_end_matches('/'))
    }
}

impl Default for ApiEndpointConfig {
    fn default() -> Self {
        Self::desktop()
    }
}

#[cfg(test)]
mod tests {
    use super::ApiEndpointConfig;

    #[test]
    fn desktop_endpoint_config_is_stable() {
        let config = ApiEndpointConfig::desktop();

        assert_eq!(config.gateway_bind, "127.0.0.1:3000");
        assert_eq!(config.api_origin, "http://127.0.0.1:3000");
        assert_eq!(config.api_base_url(), "http://127.0.0.1:3000/");
        assert_eq!(config.health_url(), "http://127.0.0.1:3000/health");
        assert_eq!(
            config.connect_src,
            vec!["'self'".to_owned(), "http://127.0.0.1:3000".to_owned()]
        );
    }
}
