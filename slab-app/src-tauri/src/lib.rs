// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
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
    let health_url = format!("{}health", api_url.trim_end_matches('/'));

    match reqwest::get(&health_url).await {
        Ok(response) => Ok(response.status().is_success()),
        Err(e) => Err(format!("Failed to connect to backend: {}", e)),
    }
}

/// Get system information
#[tauri::command]
async fn get_system_info() -> Result<String, String> {
    Ok(format!(
        "OS: {}\nArch: {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_url,
            check_backend_status,
            get_system_info
        ])
        .setup(|app| {
            setup::run_server_sidecar(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
