// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod commands;
mod plugins;
mod setup;

use setup::ApiEndpointConfig;
use commands::{core_health, core_list_models, core_list_sessions, core_list_tasks};

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
            // slab-app-core native IPC commands
            core_health,
            core_list_models,
            core_list_sessions,
            core_list_tasks,
        ])
        .setup(move |app| {
            setup::setup_windows(app)?;
            setup::run_runtime_sidecar(app)?;
            plugins::init(app, api_endpoint.clone()).map_err(std::io::Error::other)?;

            // Initialise slab-app-core state so native IPC commands work.
            tauri::async_runtime::block_on(commands::init_core_state(
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
