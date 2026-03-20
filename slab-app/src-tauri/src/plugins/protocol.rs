use std::collections::HashSet;
use std::fs;

use tauri::http::{self, header, Method, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

use super::registry::{
    is_path_within_root, normalize_relative_path, LoadedPlugin, PluginRegistryState,
};
use super::types::{PluginNetworkManifest, PluginNetworkMode};

pub fn register_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(
        "slab-plugin",
        |context, request, responder| {
            let app_handle = context.app_handle().clone();
            std::thread::spawn(move || {
                let response = handle_protocol_request(&app_handle, request);
                responder.respond(response);
            });
        },
    )
}

pub fn handle_protocol_request<R: Runtime>(
    app_handle: &AppHandle<R>,
    request: http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return build_text_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
            Some("text/plain; charset=utf-8"),
            None,
        );
    }

    let registry = app_handle.state::<PluginRegistryState>();
    let request_path = request.uri().path().trim_start_matches('/').to_string();
    let (plugin_id, raw_asset_path) = match request_path.split_once('/') {
        Some((plugin_id, asset_path)) if !plugin_id.is_empty() && !asset_path.is_empty() => {
            (plugin_id.to_string(), asset_path.to_string())
        }
        _ => {
            return build_text_response(
                StatusCode::BAD_REQUEST,
                "invalid plugin asset path",
                Some("text/plain; charset=utf-8"),
                None,
            )
        }
    };

    let normalized_asset_path = match normalize_relative_path(&raw_asset_path) {
        Ok(path) => path,
        Err(error) => {
            return build_text_response(
                StatusCode::BAD_REQUEST,
                &error,
                Some("text/plain; charset=utf-8"),
                None,
            )
        }
    };

    let plugin = match registry.get_plugin(&plugin_id) {
        Ok(plugin) => plugin,
        Err(error) => {
            return build_text_response(
                StatusCode::NOT_FOUND,
                &error,
                Some("text/plain; charset=utf-8"),
                None,
            )
        }
    };

    if !plugin.files_sha256.contains_key(&normalized_asset_path) {
        return build_text_response(
            StatusCode::FORBIDDEN,
            "asset is not declared in integrity.filesSha256",
            Some("text/plain; charset=utf-8"),
            None,
        );
    }

    let asset_path = plugin.root_dir.join(&normalized_asset_path);
    if !is_path_within_root(&plugin.root_dir, &asset_path) {
        return build_text_response(
            StatusCode::FORBIDDEN,
            "path traversal detected",
            Some("text/plain; charset=utf-8"),
            None,
        );
    }

    let bytes = match fs::read(&asset_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return build_text_response(
                StatusCode::NOT_FOUND,
                &format!("failed to read asset: {e}"),
                Some("text/plain; charset=utf-8"),
                None,
            )
        }
    };

    let content_type = mime_guess::from_path(&asset_path).first_or_octet_stream().to_string();
    let csp = if content_type.starts_with("text/html") {
        Some(build_plugin_csp(&plugin.manifest.network))
    } else {
        None
    };

    build_bytes_response(StatusCode::OK, bytes, &content_type, csp.as_deref())
}

pub fn plugin_ui_url(plugin: &LoadedPlugin) -> String {
    format!("slab-plugin://localhost/{}/{}", plugin.manifest.id, plugin.ui_entry)
}

pub fn collect_navigation_allow_hosts(network: &PluginNetworkManifest) -> HashSet<String> {
    if network.mode != PluginNetworkMode::Allowlist {
        return HashSet::new();
    }

    network
        .allow_hosts
        .iter()
        .filter_map(|entry| normalize_allow_host(entry))
        .collect::<HashSet<_>>()
}

pub fn is_allowed_navigation(url: &tauri::Url, allow_hosts: &HashSet<String>) -> bool {
    if url.scheme() == "slab-plugin" {
        return true;
    }

    if (url.scheme() == "http" || url.scheme() == "https")
        && url.host_str() == Some("slab-plugin.localhost")
    {
        return true;
    }

    let Some(host) = url.host_str() else {
        return false;
    };

    allow_hosts.contains(&host.to_ascii_lowercase())
}

fn normalize_allow_host(host: &str) -> Option<String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains("://") {
        let parsed = tauri::Url::parse(trimmed).ok()?;
        return parsed.host_str().map(|host| host.to_ascii_lowercase());
    }

    let without_path = trimmed.split('/').next().unwrap_or(trimmed);
    if without_path.is_empty() {
        return None;
    }

    Some(without_path.to_ascii_lowercase())
}

fn build_bytes_response(
    status: StatusCode,
    body: Vec<u8>,
    content_type: &str,
    csp: Option<&str>,
) -> http::Response<Vec<u8>> {
    let mut response = http::Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header("X-Content-Type-Options", "nosniff");

    response = if cfg!(debug_assertions) {
        response.header(header::CACHE_CONTROL, "no-store")
    } else {
        response.header(header::CACHE_CONTROL, "public, max-age=3600")
    };

    if let Some(csp_value) = csp {
        response = response.header("Content-Security-Policy", csp_value);
    }

    response.body(body).unwrap_or_else(|_| http::Response::new(Vec::new()))
}

fn build_text_response(
    status: StatusCode,
    body: &str,
    content_type: Option<&str>,
    csp: Option<&str>,
) -> http::Response<Vec<u8>> {
    build_bytes_response(
        status,
        body.as_bytes().to_vec(),
        content_type.unwrap_or("text/plain; charset=utf-8"),
        csp,
    )
}

fn build_plugin_csp(network: &PluginNetworkManifest) -> String {
    let mut connect_src = vec![
        "'self'".to_string(),
        "http://127.0.0.1:3000".to_string(),
        "http://localhost:3000".to_string(),
    ];

    if network.mode == PluginNetworkMode::Allowlist {
        for host in &network.allow_hosts {
            let trimmed = host.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.contains("://") {
                connect_src.push(trimmed.to_string());
            } else {
                connect_src.push(format!("https://{trimmed}"));
                connect_src.push(format!("http://{trimmed}"));
            }
        }
    }

    connect_src.sort();
    connect_src.dedup();

    format!(
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src {}; base-uri 'none'; frame-ancestors 'none';",
        connect_src.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_blocks_external_by_default() {
        let csp = build_plugin_csp(&PluginNetworkManifest {
            mode: PluginNetworkMode::Blocked,
            allow_hosts: Vec::new(),
        });
        assert!(csp.contains("default-src 'none'"));
        assert!(!csp.contains("https://example.com"));
    }
}
