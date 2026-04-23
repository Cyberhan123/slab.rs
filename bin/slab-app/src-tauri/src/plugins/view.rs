use std::collections::HashMap;
use std::sync::Mutex;

use tauri::utils::config::WebviewUrl;
use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, State, Window};

use super::protocol::{collect_navigation_allow_hosts, is_allowed_navigation, plugin_ui_url};
use super::registry::PluginRegistryState;
use super::types::{
    PluginMountViewRequest, PluginMountViewResponse, PluginUnmountViewRequest,
    PluginUpdateViewBoundsRequest, PluginViewBounds,
};

const PLUGIN_WEBVIEW_PREFIX: &str = "plugin-";

#[derive(Default)]
pub struct PluginViewManager {
    plugin_to_webview: Mutex<HashMap<String, String>>,
}

impl PluginViewManager {
    fn set_label(&self, plugin_id: &str, label: &str) -> Result<(), String> {
        let mut guard = self
            .plugin_to_webview
            .lock()
            .map_err(|_| "failed to lock plugin view manager".to_string())?;
        guard.insert(plugin_id.to_string(), label.to_string());
        Ok(())
    }

    fn get_label(&self, plugin_id: &str) -> Result<Option<String>, String> {
        let guard = self
            .plugin_to_webview
            .lock()
            .map_err(|_| "failed to lock plugin view manager".to_string())?;
        Ok(guard.get(plugin_id).cloned())
    }

    fn remove_label(&self, plugin_id: &str) -> Result<Option<String>, String> {
        let mut guard = self
            .plugin_to_webview
            .lock()
            .map_err(|_| "failed to lock plugin view manager".to_string())?;
        Ok(guard.remove(plugin_id))
    }
}

pub fn mount_plugin_view(
    app_handle: AppHandle,
    window: Window,
    registry: State<'_, PluginRegistryState>,
    view_manager: State<'_, PluginViewManager>,
    request: PluginMountViewRequest,
) -> Result<PluginMountViewResponse, String> {
    validate_bounds(&request.bounds)?;

    let plugin = registry.get_plugin(&request.plugin_id)?;
    let webview_label = plugin_webview_label(&request.plugin_id);
    let plugin_url = plugin_ui_url(&plugin);

    if let Some(existing_webview) = app_handle.get_webview(&webview_label) {
        apply_bounds_to_webview(&existing_webview, &request.bounds)?;
        existing_webview
            .show()
            .map_err(|e| format!("failed to show existing plugin webview: {e}"))?;
        view_manager.set_label(&request.plugin_id, &webview_label)?;
        return Ok(PluginMountViewResponse {
            plugin_id: request.plugin_id,
            webview_label,
            url: plugin_url,
        });
    }

    let navigation_allow_hosts =
        collect_navigation_allow_hosts(&plugin.manifest.permissions.network);
    let plugin_id = request.plugin_id.clone();
    let webview_builder = WebviewBuilder::new(
        webview_label.clone(),
        WebviewUrl::CustomProtocol(
            tauri::Url::parse(&plugin_url)
                .map_err(|e| format!("invalid plugin URL generated for `{plugin_id}`: {e}"))?,
        ),
    )
    .on_navigation(move |url| is_allowed_navigation(url, &navigation_allow_hosts))
    .on_new_window(|_, _| NewWindowResponse::Deny);

    window
        .add_child(
            webview_builder,
            LogicalPosition::new(request.bounds.x, request.bounds.y),
            LogicalSize::new(request.bounds.width, request.bounds.height),
        )
        .map_err(|e| format!("failed to mount plugin webview: {e}"))?;

    view_manager.set_label(&request.plugin_id, &webview_label)?;

    Ok(PluginMountViewResponse { plugin_id: request.plugin_id, webview_label, url: plugin_url })
}

pub fn update_plugin_view_bounds(
    app_handle: AppHandle,
    view_manager: State<'_, PluginViewManager>,
    request: PluginUpdateViewBoundsRequest,
) -> Result<(), String> {
    validate_bounds(&request.bounds)?;
    let webview_label = view_manager
        .get_label(&request.plugin_id)?
        .ok_or_else(|| format!("plugin `{}` has no mounted webview", request.plugin_id))?;

    let webview = app_handle
        .get_webview(&webview_label)
        .ok_or_else(|| format!("webview `{webview_label}` is not found"))?;
    apply_bounds_to_webview(&webview, &request.bounds)?;
    Ok(())
}

pub fn unmount_plugin_view(
    app_handle: AppHandle,
    view_manager: State<'_, PluginViewManager>,
    request: PluginUnmountViewRequest,
) -> Result<(), String> {
    let webview_label = if let Some(label) = view_manager.remove_label(&request.plugin_id)? {
        label
    } else {
        return Ok(());
    };

    if let Some(webview) = app_handle.get_webview(&webview_label) {
        webview
            .close()
            .map_err(|e| format!("failed to close plugin webview `{webview_label}`: {e}"))?;
    }

    Ok(())
}

pub fn plugin_webview_label(plugin_id: &str) -> String {
    format!("{PLUGIN_WEBVIEW_PREFIX}{plugin_id}")
}

pub fn plugin_id_from_webview_label(label: &str) -> Option<String> {
    label
        .strip_prefix(PLUGIN_WEBVIEW_PREFIX)
        .and_then(|plugin_id| if plugin_id.is_empty() { None } else { Some(plugin_id.to_string()) })
}

fn validate_bounds(bounds: &PluginViewBounds) -> Result<(), String> {
    if !bounds.x.is_finite()
        || !bounds.y.is_finite()
        || !bounds.width.is_finite()
        || !bounds.height.is_finite()
    {
        return Err("bounds must be finite numbers".to_string());
    }

    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Err("bounds width and height must be positive".to_string());
    }

    Ok(())
}

fn apply_bounds_to_webview<R: Runtime>(
    webview: &tauri::Webview<R>,
    bounds: &PluginViewBounds,
) -> Result<(), String> {
    webview
        .set_position(LogicalPosition::new(bounds.x, bounds.y))
        .map_err(|e| format!("failed to set webview position: {e}"))?;
    webview
        .set_size(LogicalSize::new(bounds.width, bounds.height))
        .map_err(|e| format!("failed to set webview size: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{plugin_id_from_webview_label, plugin_webview_label};

    #[test]
    fn plugin_webview_label_roundtrips_plugin_id() {
        let label = plugin_webview_label("video-subtitle-translator");
        assert_eq!(label, "plugin-video-subtitle-translator");
        assert_eq!(
            plugin_id_from_webview_label(&label).as_deref(),
            Some("video-subtitle-translator")
        );
        assert_eq!(plugin_id_from_webview_label("main"), None);
    }
}
