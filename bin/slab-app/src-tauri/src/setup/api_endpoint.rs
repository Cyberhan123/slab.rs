use slab_types::{DESKTOP_API_BIND, DESKTOP_API_ORIGIN};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEndpointConfig {
    pub gateway_bind: String,
    pub api_origin: String,
    pub connect_src: Vec<String>,
}

impl ApiEndpointConfig {
    pub fn desktop() -> Self {
        let api_origin = DESKTOP_API_ORIGIN.to_owned();
        Self {
            gateway_bind: DESKTOP_API_BIND.to_owned(),
            connect_src: vec!["'self'".to_owned(), api_origin.clone()],
            api_origin,
        }
    }

    pub fn api_base_url(&self) -> String {
        format!("{}/", self.api_origin.trim_end_matches('/'))
    }

    #[cfg_attr(not(test), allow(dead_code))]
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
    use slab_types::{DESKTOP_API_BIND, DESKTOP_API_ORIGIN};

    #[test]
    fn desktop_endpoint_config_is_stable() {
        let config = ApiEndpointConfig::desktop();

        assert_eq!(config.gateway_bind, DESKTOP_API_BIND);
        assert_eq!(config.api_origin, DESKTOP_API_ORIGIN);
        assert_eq!(config.api_base_url(), format!("{DESKTOP_API_ORIGIN}/"));
        assert_eq!(config.health_url(), format!("{DESKTOP_API_ORIGIN}/health"));
        assert_eq!(config.connect_src, vec!["'self'".to_owned(), DESKTOP_API_ORIGIN.to_owned()]);
    }
}
