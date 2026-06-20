use std::fs;
use std::net::IpAddr;

use crate::error::AppCoreError;
use crate::infra::plugin_runtime::{authorize_slab_api_request, execute_plugin_api_request};
use slab_plugin::{is_path_within_root, normalize_relative_path};
use slab_types::plugin::{PluginNetworkManifest, PluginNetworkMode};
use slab_types::{PluginApiRequest, PluginApiResponse, desktop_dev_allowed_origins};

use super::PluginService;

#[derive(Debug, Clone)]
pub struct PluginUiAsset {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub csp: Option<String>,
}

impl PluginService {
    pub async fn plugin_ui_asset(
        &self,
        plugin_id: &str,
        raw_asset_path: &str,
    ) -> Result<PluginUiAsset, AppCoreError> {
        self.ensure_plugin_state(plugin_id).await?;

        let registry = self.plugin_registry()?;
        registry.refresh().map_err(AppCoreError::Internal)?;
        let plugin = registry.get_plugin(plugin_id).map_err(AppCoreError::NotFound)?;
        let normalized_asset_path =
            normalize_relative_path(raw_asset_path).map_err(AppCoreError::BadRequest)?;

        if !plugin.files_sha256.contains_key(&normalized_asset_path) {
            return Err(AppCoreError::BadRequest(
                "asset is not declared in integrity.filesSha256".to_owned(),
            ));
        }

        let asset_path = plugin.root_dir.join(&normalized_asset_path);
        if !is_path_within_root(&plugin.root_dir, &asset_path) {
            return Err(AppCoreError::BadRequest("path traversal detected".to_owned()));
        }

        let bytes = fs::read(&asset_path).map_err(|error| {
            AppCoreError::NotFound(format!("failed to read plugin asset: {error}"))
        })?;
        let content_type = content_type_for_path(&normalized_asset_path).to_owned();
        let csp = if content_type.starts_with("text/html") {
            Some(build_plugin_csp(
                &plugin.manifest.permissions.network,
                &self.plugin_api_base_url(),
                self.state.config().cors_allowed_origins.as_deref(),
            ))
        } else {
            None
        };

        Ok(PluginUiAsset { bytes, content_type, csp })
    }

    pub async fn plugin_api_request(
        &self,
        caller_plugin_id: Option<&str>,
        target_plugin_id: &str,
        request: PluginApiRequest,
    ) -> Result<PluginApiResponse, AppCoreError> {
        // Defense in depth: a Slab API request must be proxied on behalf of a
        // concrete plugin, and that caller must be the same plugin targeted by the
        // route. The HTTP handler enforces this with a 403; this check guards any
        // other caller and keeps the core contract explicit so a plugin can never
        // spend another plugin's declared permissions.
        let caller_plugin_id = caller_plugin_id.ok_or_else(|| {
            AppCoreError::BadRequest("plugin api request requires a caller plugin id".to_string())
        })?;
        if caller_plugin_id != target_plugin_id {
            return Err(AppCoreError::BadRequest(format!(
                "plugin api request caller `{caller_plugin_id}` does not match target plugin `{target_plugin_id}`"
            )));
        }

        self.ensure_plugin_state(target_plugin_id).await?;

        let registry = self.plugin_registry()?;
        registry.refresh().map_err(AppCoreError::Internal)?;
        let plugin = registry.get_plugin(target_plugin_id).map_err(AppCoreError::NotFound)?;
        authorize_slab_api_request(&plugin.manifest.permissions.slab_api, &request)
            .map_err(AppCoreError::BadRequest)?;
        execute_plugin_api_request(&self.plugin_api_base_url(), &request)
            .await
            .map_err(AppCoreError::BadRequest)
    }
}

fn content_type_for_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn build_plugin_csp(
    network: &PluginNetworkManifest,
    api_base_url: &str,
    configured_frame_ancestors: Option<&str>,
) -> String {
    let mut connect_src = Vec::new();

    if network.mode == PluginNetworkMode::Allowlist {
        for host in &network.allow_hosts {
            let trimmed = host.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.contains("://") {
                push_plugin_connect_src(&mut connect_src, trimmed.to_owned(), api_base_url);
            } else {
                push_plugin_connect_src(
                    &mut connect_src,
                    format!("https://{trimmed}"),
                    api_base_url,
                );
                push_plugin_connect_src(
                    &mut connect_src,
                    format!("http://{trimmed}"),
                    api_base_url,
                );
            }
        }
    }

    connect_src.sort();
    connect_src.dedup();
    let connect_src =
        if connect_src.is_empty() { "'none'".to_owned() } else { connect_src.join(" ") };

    let mut frame_ancestors = vec!["'self'".to_owned()];
    frame_ancestors.extend(desktop_dev_allowed_origins().iter().map(|origin| (*origin).to_owned()));
    frame_ancestors.extend(parse_configured_frame_ancestors(configured_frame_ancestors));
    frame_ancestors.sort();
    frame_ancestors.dedup();

    format!(
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src {}; base-uri 'none'; frame-ancestors {};",
        connect_src,
        frame_ancestors.join(" ")
    )
}

fn parse_configured_frame_ancestors(configured_origins: Option<&str>) -> Vec<String> {
    configured_origins
        .into_iter()
        .flat_map(|origins| origins.split(','))
        .filter_map(|origin| {
            let origin = origin.trim();
            if origin.is_empty() || reqwest::Url::parse(origin).is_err() {
                return None;
            }
            Some(origin.to_owned())
        })
        .collect()
}

fn push_plugin_connect_src(connect_src: &mut Vec<String>, candidate: String, api_base_url: &str) {
    if !is_api_endpoint_origin(&candidate, api_base_url) {
        connect_src.push(candidate);
    }
}

fn is_api_endpoint_origin(candidate: &str, api_base_url: &str) -> bool {
    let Ok(candidate_url) = reqwest::Url::parse(candidate) else {
        return false;
    };
    let Ok(api_url) = reqwest::Url::parse(api_base_url) else {
        return false;
    };

    let Some(candidate_host) = candidate_url.host_str() else {
        return false;
    };
    let Some(api_host) = api_url.host_str() else {
        return false;
    };

    hosts_match_local_api(candidate_host, api_host)
        && candidate_url.port_or_known_default() == api_url.port_or_known_default()
}

fn hosts_match_local_api(candidate_host: &str, api_host: &str) -> bool {
    candidate_host.eq_ignore_ascii_case(api_host)
        || (is_loopback_host(candidate_host) && is_loopback_host(api_host))
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip_address| ip_address.is_loopback())
}

#[cfg(test)]
mod tests {
    use slab_types::plugin::{PluginNetworkManifest, PluginNetworkMode};

    use super::{build_plugin_csp, content_type_for_path};

    #[test]
    fn content_type_uses_basic_plugin_asset_mimes() {
        assert_eq!(content_type_for_path("ui/index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_for_path("ui/app.mjs"), "text/javascript; charset=utf-8");
        assert_eq!(content_type_for_path("ui/app.wasm"), "application/wasm");
    }

    #[test]
    fn browser_csp_blocks_direct_local_api_access() {
        let csp = build_plugin_csp(
            &PluginNetworkManifest {
                mode: PluginNetworkMode::Allowlist,
                allow_hosts: vec![
                    "example.com".to_owned(),
                    "127.0.0.1:3000".to_owned(),
                    "localhost:3000".to_owned(),
                ],
            },
            "http://127.0.0.1:3000/",
            None,
        );

        assert!(csp.contains("https://example.com"));
        assert!(csp.contains("http://example.com"));
        assert!(csp.contains("connect-src http://example.com https://example.com"));
        assert!(!csp.contains("http://127.0.0.1:3000"));
        assert!(!csp.contains("https://127.0.0.1:3000"));
        assert!(!csp.contains("http://localhost:3000"));
        assert!(!csp.contains("https://localhost:3000"));
    }

    #[test]
    fn browser_csp_allows_browser_app_frame_ancestors() {
        let csp = build_plugin_csp(
            &PluginNetworkManifest { mode: PluginNetworkMode::Blocked, allow_hosts: Vec::new() },
            "http://127.0.0.1:3000/",
            None,
        );

        assert!(csp.contains("connect-src 'none'"));
        assert!(csp.contains("frame-ancestors"));
        assert!(csp.contains("http://localhost:1420"));
        assert!(csp.contains("http://127.0.0.1:1420"));
    }

    #[test]
    fn browser_csp_allows_configured_frame_ancestors() {
        let csp = build_plugin_csp(
            &PluginNetworkManifest { mode: PluginNetworkMode::Blocked, allow_hosts: Vec::new() },
            "http://127.0.0.1:3000/",
            Some("http://127.0.0.1:54321, http://localhost:54321, "),
        );

        assert!(csp.contains("http://127.0.0.1:54321"));
        assert!(csp.contains("http://localhost:54321"));
    }
}
