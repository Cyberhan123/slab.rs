// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod api;
mod plugins;
mod setup;

use std::sync::Arc;

use setup::ApiEndpointConfig;
use slab_app_core::context::AppState;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Get the API base URL for the current environment
#[tauri::command]
fn get_api_url(api_endpoint: tauri::State<'_, ApiEndpointConfig>) -> String {
    api_endpoint.api_origin.clone()
}

/// Check if the embedded backend/core state is available.
#[tauri::command]
async fn check_backend_status(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    api::health::health(state).await
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
            check_backend_status,
            get_system_info,
            plugins::plugin_list,
            plugins::plugin_mount_view,
            plugins::plugin_update_view_bounds,
            plugins::plugin_unmount_view,
            plugins::plugin_call,
            plugins::plugin_api_request,
            // native IPC commands - health
            api::health::health,
            // audio
            api::v1::audio::handler::transcribe,
            // chat
            api::v1::chat::handler::list_chat_models,
            api::v1::chat::handler::chat_completions,
            api::v1::chat::handler::chat_completions_stream,
            api::v1::chat::handler::completions,
            // models
            api::v1::models::handler::list_models,
            api::v1::models::handler::create_model,
            api::v1::models::handler::import_model_pack,
            api::v1::models::handler::get_model,
            api::v1::models::handler::get_model_enhancement,
            api::v1::models::handler::update_model,
            api::v1::models::handler::update_model_enhancement,
            api::v1::models::handler::delete_model,
            api::v1::models::handler::load_model,
            api::v1::models::handler::unload_model,
            api::v1::models::handler::switch_model,
            api::v1::models::handler::download_model,
            api::v1::models::handler::list_available_models,
            // sessions
            api::v1::session::handler::list_sessions,
            api::v1::session::handler::create_session,
            api::v1::session::handler::delete_session,
            api::v1::session::handler::list_session_messages,
            // tasks
            api::v1::tasks::handler::list_tasks,
            api::v1::tasks::handler::get_task,
            api::v1::tasks::handler::get_task_result,
            api::v1::tasks::handler::cancel_task,
            api::v1::tasks::handler::restart_task,
            // setup
            api::v1::setup::handler::setup_status,
            api::v1::setup::handler::download_ffmpeg,
            api::v1::setup::handler::complete_setup,
            // backends
            api::v1::backend::handler::backend_status,
            api::v1::backend::handler::list_backends,
            api::v1::backend::handler::download_backend_lib,
            // system
            api::v1::system::handler::gpu_status,
            // settings
            api::v1::settings::handler::list_settings,
            api::v1::settings::handler::get_setting,
            api::v1::settings::handler::update_setting,
            // images
            api::v1::images::handler::generate_images,
            // video
            api::v1::video::handler::generate_video,
            // ffmpeg
            api::v1::ffmpeg::handler::convert,
            // agents
            api::v1::agent::handler::spawn_agent,
            api::v1::agent::handler::agent_input,
            api::v1::agent::handler::agent_status,
            api::v1::agent::handler::agent_shutdown,
        ])
        .setup(move |app| {
            setup::setup_windows(app)?;
            let runtime_supervisor = setup::run_runtime_sidecar(app)?;
            plugins::init(app, api_endpoint.clone()).map_err(std::io::Error::other)?;

            tauri::async_runtime::block_on(api::init_state(
                app.handle(),
                runtime_supervisor.launch_spec(),
                runtime_supervisor.status_registry(),
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
