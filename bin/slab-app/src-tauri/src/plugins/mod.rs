mod protocol;
mod registry;
mod runtime;
mod types;
mod view;

use tauri::{AppHandle, Manager, Runtime, State, Window};

use crate::setup::ApiEndpointConfig;

pub use types::{
    PluginApiRequest, PluginApiResponse, PluginCallRequest, PluginCallResponse, PluginInfo,
    PluginMountViewRequest, PluginMountViewResponse, PluginUnmountViewRequest,
    PluginUpdateViewBoundsRequest,
};
pub use view::PluginViewManager;

use registry::{PluginRegistryState, resolve_plugins_root};
use runtime::{PluginRuntimeManager, execute_plugin_api_request_async};

pub fn register_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    protocol::register_protocol(builder)
}

pub fn init<R: Runtime>(
    app: &mut tauri::App<R>,
    api_endpoint: ApiEndpointConfig,
) -> Result<(), String> {
    let plugins_root = resolve_plugins_root()?;
    let registry = PluginRegistryState::new(plugins_root)?;
    let runtime = PluginRuntimeManager::new(api_endpoint)?;
    app.manage(registry);
    app.manage(runtime);
    app.manage(PluginViewManager::default());
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
    registry: State<'_, PluginRegistryState>,
    runtime: State<'_, PluginRuntimeManager>,
    request: PluginCallRequest,
) -> Result<PluginCallResponse, String> {
    let plugin = registry.get_plugin(&request.plugin_id)?;
    runtime.call_plugin(&app_handle, &plugin, &request)
}

#[tauri::command]
pub async fn plugin_api_request(
    api_endpoint: State<'_, ApiEndpointConfig>,
    request: PluginApiRequest,
) -> Result<PluginApiResponse, String> {
    execute_plugin_api_request_async(api_endpoint.inner(), &request).await
}
