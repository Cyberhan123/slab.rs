// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod plugins;
mod setup;

use setup::ApiEndpointConfig;

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
            plugins::plugin_list,
            plugins::plugin_mount_view,
            plugins::plugin_update_view_bounds,
            plugins::plugin_unmount_view,
            plugins::plugin_call,
            plugins::plugin_api_request,
            plugins::plugin_pick_file,
            plugins::plugin_set_theme_snapshot,
            plugins::plugin_theme_snapshot,
        ])
        .setup(move |app| {
            setup::setup_windows(app)?;
            let plugins_root = plugins::resolve_plugins_root_for_app(app).map_err(|error| {
                log::error!("failed to resolve plugins root before starting sidecar: {error}");
                std::io::Error::other(error)
            })?;
            setup::run_server_sidecar(app, &plugins_root)?;
            plugins::init(app, api_endpoint.clone()).map_err(|error| {
                log::error!("failed to initialize plugins: {error}");
                std::io::Error::other(error)
            })?;

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
