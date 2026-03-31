// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod plugins;
mod setup;

use setup::ApiEndpointConfig;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Get the API base URL for the current environment
#[tauri::command]
fn get_api_url(api_endpoint: tauri::State<'_, ApiEndpointConfig>) -> String {
    api_endpoint.api_origin.clone()
}

/// Check if the backend server is running
#[tauri::command]
async fn check_backend_status(
    api_endpoint: tauri::State<'_, ApiEndpointConfig>,
) -> Result<bool, String> {
    match reqwest::get(api_endpoint.health_url()).await {
        Ok(response) => Ok(response.status().is_success()),
        Err(e) => Err(format!("Failed to connect to backend: {}", e)),
    }
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

    // Register slab-app-core native IPC commands so the frontend can call the
    // business logic directly without an HTTP round-trip when running embedded.
    builder = slab_app_core::tauri_bridge::register(builder);

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    let app = builder
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_url,
            check_backend_status,
            get_system_info,
            plugins::plugin_list,
            plugins::plugin_mount_view,
            plugins::plugin_update_view_bounds,
            plugins::plugin_unmount_view,
            plugins::plugin_call,
            plugins::plugin_api_request
        ])
        .setup(move |app| {
            setup::setup_windows(app)?;
            setup::run_server_sidecar(app)?;
            plugins::init(app, api_endpoint.clone()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            setup::shutdown_server_sidecar(app_handle);
        }
        _ => {}
    });
}
