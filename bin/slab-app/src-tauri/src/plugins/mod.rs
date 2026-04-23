mod protocol;
mod registry;
mod runtime;
mod types;
mod view;

use std::path::Path;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime, State, Webview, Window};
use tauri_plugin_dialog::{DialogExt, PickerMode};

use crate::setup::ApiEndpointConfig;

pub use types::{
    PluginApiRequest, PluginApiResponse, PluginCallRequest, PluginCallResponse, PluginInfo,
    PluginMountViewRequest, PluginMountViewResponse, PluginPickFileResponse, PluginThemeSnapshot,
    PluginUnmountViewRequest, PluginUpdateViewBoundsRequest,
};
pub use view::PluginViewManager;

use registry::{PluginRegistryState, resolve_plugins_root};
use runtime::{PluginRuntimeManager, execute_plugin_api_request_async};

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
) -> Result<(), String> {
    let plugins_root = resolve_plugins_root(app)?;
    log::info!("resolved plugins root to {}", plugins_root.display());
    let registry = PluginRegistryState::new(plugins_root)?;
    let runtime = PluginRuntimeManager::new(api_endpoint)?;
    app.manage(registry);
    app.manage(runtime);
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
pub fn plugin_mount_view(
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
pub fn plugin_call(
    app_handle: AppHandle,
    webview: Webview,
    registry: State<'_, PluginRegistryState>,
    runtime: State<'_, PluginRuntimeManager>,
    request: PluginCallRequest,
) -> Result<PluginCallResponse, String> {
    if let Some(caller_plugin_id) = caller_plugin_id(&webview) {
        ensure_same_plugin_call(&caller_plugin_id, &request.plugin_id)?;
    }

    let plugin = registry.get_plugin(&request.plugin_id)?;
    runtime.call_plugin(&app_handle, &plugin, &request)
}

#[tauri::command]
pub async fn plugin_api_request(
    webview: Webview,
    api_endpoint: State<'_, ApiEndpointConfig>,
    registry: State<'_, PluginRegistryState>,
    request: PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    if let Some(caller_plugin_id) = caller_plugin_id(&webview) {
        let plugin = registry.get_plugin(&caller_plugin_id)?;
        authorize_slab_api_request(&plugin.manifest.permissions.slab_api, &request)?;
    }

    execute_plugin_api_request_async(api_endpoint.inner(), &request).await
}

#[tauri::command]
pub async fn plugin_pick_file(
    app_handle: AppHandle,
    webview: Webview,
    registry: State<'_, PluginRegistryState>,
) -> Result<PluginPickFileResponse, String> {
    if let Some(caller_plugin_id) = caller_plugin_id(&webview) {
        let plugin = registry.get_plugin(&caller_plugin_id)?;
        ensure_video_file_read_permission(&plugin.manifest.permissions.files.read)?;
    }

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

fn is_allowed_video_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        VIDEO_FILE_EXTENSIONS.iter().any(|allowed| allowed.eq_ignore_ascii_case(extension))
    })
}

fn caller_plugin_id(webview: &Webview) -> Option<String> {
    view::plugin_id_from_webview_label(webview.label())
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
    if read_permissions.iter().any(|permission| permission == "video") {
        return Ok(());
    }

    Err("plugin file picker requires permissions.files.read to include `video`".to_string())
}

pub(super) fn authorize_slab_api_request(
    slab_api_permissions: &[String],
    request: &PluginApiRequest,
) -> Result<(), String> {
    let Some(required_permission) = required_slab_api_permission(&request.method, &request.path)
    else {
        return Err(format!(
            "plugin API request {} {} is not part of the allowed plugin API surface",
            request.method, request.path
        ));
    };

    if slab_api_permissions.iter().any(|permission| permission == required_permission) {
        return Ok(());
    }

    Err(format!(
        "plugin API request {} {} requires permissions.slabApi `{required_permission}`",
        request.method, request.path
    ))
}

fn required_slab_api_permission(method: &str, path: &str) -> Option<&'static str> {
    let method = method.to_ascii_uppercase();
    let path = path.split('?').next().unwrap_or(path);

    match method.as_str() {
        "GET" if path_matches(path, "/v1/models") => Some("models:read"),
        "POST" if path == "/v1/ffmpeg/convert" => Some("ffmpeg:convert"),
        "POST" if path == "/v1/audio/transcriptions" => Some("audio:transcribe"),
        "POST" if path == "/v1/subtitles/render" => Some("subtitle:render"),
        "POST" if path == "/v1/chat/completions" => Some("chat:complete"),
        "GET" if path_matches(path, "/v1/tasks") => Some("tasks:read"),
        "POST" if path.starts_with("/v1/tasks/") && path.ends_with("/cancel") => {
            Some("tasks:cancel")
        }
        _ => None,
    }
}

fn path_matches(path: &str, base: &str) -> bool {
    path == base || path.starts_with(&format!("{base}/"))
}

#[cfg(test)]
mod tests {
    use super::{
        PluginThemeSnapshot, PluginThemeState, authorize_slab_api_request, ensure_same_plugin_call,
        ensure_video_file_read_permission, is_allowed_video_path,
    };
    use std::path::Path;

    #[test]
    fn video_file_filter_accepts_known_video_extensions() {
        assert!(is_allowed_video_path(Path::new("C:/media/movie.mp4")));
        assert!(is_allowed_video_path(Path::new("C:/media/MOVIE.MKV")));
        assert!(!is_allowed_video_path(Path::new("C:/media/audio.wav")));
    }

    #[test]
    fn slab_api_permissions_allow_declared_plugin_surface() {
        let permissions = vec![
            "models:read".to_string(),
            "ffmpeg:convert".to_string(),
            "audio:transcribe".to_string(),
            "subtitle:render".to_string(),
            "chat:complete".to_string(),
            "tasks:read".to_string(),
            "tasks:cancel".to_string(),
        ];

        for (method, path) in [
            ("GET", "/v1/models?capability=audio_transcription"),
            ("POST", "/v1/ffmpeg/convert"),
            ("POST", "/v1/audio/transcriptions"),
            ("POST", "/v1/subtitles/render"),
            ("POST", "/v1/chat/completions"),
            ("GET", "/v1/tasks/task-1/result"),
            ("POST", "/v1/tasks/task-1/cancel"),
        ] {
            let request = super::PluginApiRequest {
                method: method.to_string(),
                path: path.to_string(),
                headers: Default::default(),
                body: None,
                timeout_ms: None,
            };
            assert!(authorize_slab_api_request(&permissions, &request).is_ok());
        }
    }

    #[test]
    fn slab_api_permissions_reject_missing_or_unknown_surface() {
        let request = super::PluginApiRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            headers: Default::default(),
            body: None,
            timeout_ms: None,
        };
        assert!(authorize_slab_api_request(&["models:read".to_string()], &request).is_err());

        let unknown = super::PluginApiRequest {
            method: "DELETE".to_string(),
            path: "/v1/models/model-1".to_string(),
            headers: Default::default(),
            body: None,
            timeout_ms: None,
        };
        assert!(authorize_slab_api_request(&["models:read".to_string()], &unknown).is_err());
    }

    #[test]
    fn file_picker_requires_video_read_permission() {
        assert!(ensure_video_file_read_permission(&["video".to_string()]).is_ok());
        assert!(ensure_video_file_read_permission(&["audio".to_string()]).is_err());
    }

    #[test]
    fn plugin_webview_cannot_cross_call_other_plugins() {
        assert!(ensure_same_plugin_call("a", "a").is_ok());
        assert!(ensure_same_plugin_call("a", "b").is_err());
    }

    #[test]
    fn theme_state_roundtrips_snapshot() {
        let state = PluginThemeState::default();
        let mut snapshot = PluginThemeSnapshot::default();
        snapshot.tokens.insert("background".to_string(), "oklch(20% 0 0)".to_string());

        state.set_snapshot(snapshot.clone()).unwrap();

        assert_eq!(state.snapshot().unwrap(), snapshot);
    }
}
