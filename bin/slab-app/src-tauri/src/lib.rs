// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod plugins;
mod setup;

use setup::ApiEndpointConfig;
use slab_app_core::tauri_bridge::{
    core_backend_status, core_cancel_task, core_complete_setup, core_create_model,
    core_create_session, core_delete_model, core_delete_session, core_download_backend_lib,
    core_download_ffmpeg, core_download_model, core_get_model, core_get_setting, core_get_task,
    core_get_task_result, core_gpu_status, core_health, core_import_model_config,
    core_list_available_models, core_list_backends, core_list_models, core_list_session_messages,
    core_list_sessions, core_list_settings, core_list_tasks, core_load_model,
    core_reload_backend_lib, core_setup_status, core_switch_model, core_unload_model,
    core_update_model, core_update_setting,
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Get the API base URL for the current environment
#[tauri::command]
fn get_api_url(api_endpoint: tauri::State<'_, ApiEndpointConfig>) -> String {
    api_endpoint.api_origin.clone()
}

/// Get system information
#[tauri::command]
async fn get_system_info() -> Result<String, String> {
    Ok(format!("OS: {}\nArch: {}", std::env::consts::OS, std::env::consts::ARCH))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let api_endpoint = ApiEndpointConfig::desktop();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_decorum::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("slab-app".to_string()),
                    }),
                ])
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(api_endpoint.clone());

    builder = plugins::register_protocol(builder);

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    let app = builder
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_url,
            get_system_info,
            plugins::plugin_list,
            plugins::plugin_mount_view,
            plugins::plugin_update_view_bounds,
            plugins::plugin_unmount_view,
            plugins::plugin_call,
            plugins::plugin_api_request,
            // slab-app-core native IPC commands — health
            core_health,
            // models
            core_list_models,
            core_create_model,
            core_import_model_config,
            core_get_model,
            core_update_model,
            core_delete_model,
            core_load_model,
            core_unload_model,
            core_switch_model,
            core_download_model,
            core_list_available_models,
            // sessions
            core_list_sessions,
            core_create_session,
            core_delete_session,
            core_list_session_messages,
            // tasks
            core_list_tasks,
            core_get_task,
            core_get_task_result,
            core_cancel_task,
            // setup
            core_setup_status,
            core_download_ffmpeg,
            core_complete_setup,
            // backends
            core_backend_status,
            core_list_backends,
            core_download_backend_lib,
            core_reload_backend_lib,
            // system
            core_gpu_status,
            // settings
            core_list_settings,
            core_get_setting,
            core_update_setting,
        ])
        .setup(move |app| {
            setup::setup_windows(app)?;
            setup::run_runtime_sidecar(app)?;
            plugins::init(app, api_endpoint.clone()).map_err(std::io::Error::other)?;

            // Initialise slab-app-core state so native IPC commands work.
            tauri::async_runtime::block_on(slab_app_core::tauri_bridge::init_state(
                app.handle(),
                &format!("http://{}", setup::RUNTIME_GRPC_BIND),
            ))
            .map_err(|e| std::io::Error::other(format!("core state init failed: {e}")))?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            setup::shutdown_runtime_sidecar(app_handle);
        }
        _ => {}
    });
}
