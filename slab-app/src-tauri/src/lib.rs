// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod plugins;
mod setup;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Get the API base URL for the current environment
#[tauri::command]
fn get_api_url() -> String {
    // In development, default to localhost:3000
    // In production, this could be configured via config files
    std::env::var("SLAB_API_URL").unwrap_or_else(|_| "http://localhost:3000/".to_string())
}

/// Check if the backend server is running
#[tauri::command]
async fn check_backend_status() -> Result<bool, String> {
    let api_url = get_api_url();
    let health_url = format!("{}/health", api_url.trim_end_matches('/'));

    match reqwest::get(&health_url).await {
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
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());

    builder = plugins::register_protocol(builder);

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
        .setup(|app| {
            setup::run_server_sidecar(app)?;
            plugins::init(app).map_err(std::io::Error::other)?;
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
