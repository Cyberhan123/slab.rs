mod protocol;
mod registry;
mod types;
mod view;
mod ws_client;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, State, Webview, Window};
use tauri_plugin_dialog::{DialogExt, PickerMode};

use crate::setup::ApiEndpointConfig;

pub use types::{
    PluginCallRequest, PluginCallResponse, PluginInfo, PluginMountViewRequest,
    PluginMountViewResponse, PluginPickFileResponse, PluginThemeSnapshot, PluginUnmountViewRequest,
    PluginUpdateViewBoundsRequest,
};
pub use view::PluginViewManager;

pub use registry::PluginRegistryState;
use registry::resolve_plugins_root;
use ws_client::PluginRpcWsClient;

const HOST_THEME_EVENT_NAME: &str = "plugin://host/theme";

#[derive(Default)]
pub struct PluginThemeState {
    snapshot: Mutex<PluginThemeSnapshot>,
}

impl PluginThemeState {
    fn set_snapshot(&self, snapshot: PluginThemeSnapshot) -> Result<(), String> {
        let mut guard =
            self.snapshot.lock().map_err(|_| "failed to lock plugin theme state".to_string())?;
        *guard = snapshot;
        Ok(())
    }

    fn snapshot(&self) -> Result<PluginThemeSnapshot, String> {
        let guard =
            self.snapshot.lock().map_err(|_| "failed to lock plugin theme state".to_string())?;
        Ok(guard.clone())
    }
}

pub fn resolve_plugins_root_for_app<R: Runtime>(
    app: &tauri::App<R>,
) -> Result<std::path::PathBuf, String> {
    resolve_plugins_root(app)
}

pub fn register_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    protocol::register_protocol(builder)
}

pub fn init<R: Runtime>(
    app: &mut tauri::App<R>,
    api_endpoint: ApiEndpointConfig,
    plugins_root: PathBuf,
) -> Result<(), String> {
    log::info!("resolved plugins root to {}", plugins_root.display());
    let registry = PluginRegistryState::new(plugins_root)?;
    let ws_client = PluginRpcWsClient::new(api_endpoint.clone());
    ws_client::spawn_plugin_event_listener(app.handle().clone(), api_endpoint);
    app.manage(registry);
    app.manage(ws_client);
    app.manage(PluginViewManager::default());
    app.manage(PluginThemeState::default());
    Ok(())
}

#[tauri::command]
pub fn plugin_list(registry: State<'_, PluginRegistryState>) -> Result<Vec<PluginInfo>, String> {
    registry.refresh()?;
    registry.list()
}

#[tauri::command]
pub async fn plugin_mount_view(
    app_handle: AppHandle,
    window: Window,
    registry: State<'_, PluginRegistryState>,
    view_manager: State<'_, PluginViewManager>,
    request: PluginMountViewRequest,
) -> Result<PluginMountViewResponse, String> {
    view::mount_plugin_view(app_handle, window, registry, view_manager, request)
}

#[tauri::command]
pub fn plugin_update_view_bounds(
    app_handle: AppHandle,
    view_manager: State<'_, PluginViewManager>,
    request: PluginUpdateViewBoundsRequest,
) -> Result<(), String> {
    view::update_plugin_view_bounds(app_handle, view_manager, request)
}

#[tauri::command]
pub fn plugin_unmount_view(
    app_handle: AppHandle,
    view_manager: State<'_, PluginViewManager>,
    request: PluginUnmountViewRequest,
) -> Result<(), String> {
    view::unmount_plugin_view(app_handle, view_manager, request)
}

#[tauri::command]
pub async fn plugin_call(
    webview: Webview,
    runtime_client: State<'_, PluginRpcWsClient>,
    request: PluginCallRequest,
) -> Result<PluginCallResponse, String> {
    let caller_plugin_id = caller_plugin_id(&webview);
    authorize_plugin_call_request(caller_plugin_id.as_deref(), &request)?;

    runtime_client.call(&request).await
}

#[tauri::command]
pub async fn plugin_pick_file(
    app_handle: AppHandle,
    webview: Webview,
    registry: State<'_, PluginRegistryState>,
) -> Result<PluginPickFileResponse, String> {
    let caller_plugin_id = caller_plugin_id(&webview)
        .ok_or_else(|| "plugin file picker requires a plugin webview caller".to_string())?;
    let plugin = registry.get_plugin(&caller_plugin_id)?;
    ensure_video_file_read_permission(&plugin.manifest.permissions.files.read)?;

    let selected = app_handle
        .dialog()
        .file()
        .set_title("Select a video file")
        .set_picker_mode(PickerMode::Video)
        .add_filter("Video", VIDEO_FILE_EXTENSIONS)
        .blocking_pick_file();

    let Some(selected) = selected else {
        return Ok(PluginPickFileResponse { path: None });
    };

    let path = selected
        .simplified()
        .into_path()
        .map_err(|e| format!("failed to resolve selected file path: {e}"))?;
    if !is_allowed_video_path(&path) {
        return Err(format!("unsupported video file extension: {}", path.display()));
    }

    Ok(PluginPickFileResponse { path: Some(path.to_string_lossy().into_owned()) })
}

#[tauri::command]
pub fn plugin_set_theme_snapshot(
    app_handle: AppHandle,
    webview: Webview,
    theme_state: State<'_, PluginThemeState>,
    snapshot: PluginThemeSnapshot,
) -> Result<(), String> {
    if webview.label() != "main" {
        return Err("only the main host webview can publish plugin theme snapshots".to_string());
    }

    theme_state.set_snapshot(snapshot.clone())?;
    app_handle
        .emit(HOST_THEME_EVENT_NAME, snapshot)
        .map_err(|e| format!("failed to emit plugin theme snapshot: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn plugin_theme_snapshot(
    theme_state: State<'_, PluginThemeState>,
) -> Result<PluginThemeSnapshot, String> {
    theme_state.snapshot()
}

const VIDEO_FILE_EXTENSIONS: &[&str] =
    &["mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "flv", "mpeg", "mpg", "3gp"];
const VIDEO_FILE_READ_PERMISSION: &str = "video";

fn is_allowed_video_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        VIDEO_FILE_EXTENSIONS.iter().any(|allowed| allowed.eq_ignore_ascii_case(extension))
    })
}

fn caller_plugin_id(webview: &Webview) -> Option<String> {
    view::plugin_id_from_webview_label(webview.label())
}

fn authorize_plugin_call_request(
    caller_plugin_id: Option<&str>,
    request: &PluginCallRequest,
) -> Result<(), String> {
    let caller_plugin_id = caller_plugin_id
        .ok_or_else(|| "plugin call requires a plugin webview caller".to_string())?;
    ensure_same_plugin_call(caller_plugin_id, &request.plugin_id)
}

fn ensure_same_plugin_call(
    caller_plugin_id: &str,
    requested_plugin_id: &str,
) -> Result<(), String> {
    if caller_plugin_id == requested_plugin_id {
        return Ok(());
    }

    Err(format!("plugin `{caller_plugin_id}` cannot call plugin `{requested_plugin_id}`"))
}

fn ensure_video_file_read_permission(read_permissions: &[String]) -> Result<(), String> {
    if read_permissions.iter().any(|permission| permission == VIDEO_FILE_READ_PERMISSION) {
        return Ok(());
    }

    Err(format!(
        "plugin file picker requires permissions.files.read to include `{VIDEO_FILE_READ_PERMISSION}`"
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        PluginThemeSnapshot, PluginThemeState, authorize_plugin_call_request,
        ensure_same_plugin_call, ensure_video_file_read_permission, is_allowed_video_path,
    };
    use std::path::Path;

    #[test]
    fn video_file_filter_accepts_known_video_extensions() {
        assert!(is_allowed_video_path(Path::new("C:/media/movie.mp4")));
        assert!(is_allowed_video_path(Path::new("C:/media/MOVIE.MKV")));
        assert!(!is_allowed_video_path(Path::new("C:/media/audio.wav")));
    }

    #[test]
    fn file_picker_requires_video_read_permission() {
        assert!(
            ensure_video_file_read_permission(&[super::VIDEO_FILE_READ_PERMISSION.to_string()])
                .is_ok()
        );
        assert!(ensure_video_file_read_permission(&["audio".to_string()]).is_err());
    }

    #[test]
    fn plugin_webview_cannot_cross_call_other_plugins() {
        assert!(ensure_same_plugin_call("a", "a").is_ok());
        assert!(ensure_same_plugin_call("a", "b").is_err());
    }

    #[test]
    fn plugin_call_authorization_uses_webview_caller_when_present() {
        let request = super::PluginCallRequest {
            plugin_id: "video-subtitle-translator".to_string(),
            function: "run".to_string(),
            input: String::new(),
        };

        // `None` models a non-plugin webview (e.g. the main host window, whose
        // label carries no plugin prefix) — it must NOT be able to invoke plugins.
        let missing_caller = authorize_plugin_call_request(None, &request)
            .expect_err("caller-less plugin call should be rejected");
        assert!(missing_caller.contains("requires a plugin webview caller"));
        assert!(authorize_plugin_call_request(Some("video-subtitle-translator"), &request).is_ok());
        assert!(authorize_plugin_call_request(Some("other-plugin"), &request).is_err());
    }

    #[test]
    fn theme_state_roundtrips_snapshot() {
        let state = PluginThemeState::default();
        let mut snapshot = PluginThemeSnapshot::default();
        snapshot.tokens.insert("background".to_string(), "oklch(20% 0 0)".to_string());

        state.set_snapshot(snapshot.clone()).unwrap();

        assert_eq!(state.snapshot().unwrap(), snapshot);
    }

    /// Boundary guard: the desktop host must not re-introduce the
    /// `plugin_api_request` HTTP forward. Plugins reach slab-server directly
    /// over HTTP via `@slab/plugin-sdk` → `@slab/api` → slab-server; Tauri only
    /// owns the desktop plugin host surface (views, theme, file pick, events).
    /// These checks scan sibling files (never this one) to avoid self-match.
    #[test]
    fn plugin_api_request_http_forward_stays_removed() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        for capability in ["capabilities/plugin-webview.json", "capabilities/default.json"] {
            let path = format!("{manifest_dir}/{capability}");
            let content =
                std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {path}"));
            assert!(
                !content.contains("allow-plugin-api-request"),
                "{capability} must not whitelist the removed plugin_api_request command"
            );
        }

        let runtime_path = format!("{manifest_dir}/src/plugins/runtime.rs");
        assert!(
            !Path::new(&runtime_path).exists(),
            "src/plugins/runtime.rs (the reqwest HTTP forward) must stay removed"
        );

        let permission_path =
            format!("{manifest_dir}/permissions/autogenerated/plugin_api_request.toml");
        assert!(
            !Path::new(&permission_path).exists(),
            "the plugin_api_request autogenerated permission must stay removed"
        );

        for source in ["build.rs", "src/lib.rs"] {
            let path = format!("{manifest_dir}/{source}");
            let content =
                std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {path}"));
            assert!(
                !content.contains("plugin_api_request"),
                "{source} must not register the removed plugin_api_request command"
            );
        }
    }
}
