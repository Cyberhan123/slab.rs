// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod diagnostics;
mod paths;
mod plugins;
mod setup;
mod workspace;

use setup::ApiEndpointConfig;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _otel_provider = init_telemetry();
    let api_endpoint = ApiEndpointConfig::desktop();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::new().build())
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
            plugins::plugin_pick_file,
            plugins::plugin_set_theme_snapshot,
            plugins::plugin_theme_snapshot,
            diagnostics::export_diagnostics,
        ])
        .setup(move |app| {
            let workspace_bootstrap = workspace::init(app).map_err(|error| {
                log::error!("failed to initialize workspace state: {error}");
                std::io::Error::other(error)
            })?;
            let plugins_root = plugins::resolve_plugins_root_for_app(app).map_err(|error| {
                log::error!("failed to resolve plugins root before starting sidecar: {error}");
                std::io::Error::other(error)
            })?;
            setup::run_server_sidecar(app, workspace_bootstrap.sidecar_config)?;
            plugins::init(app, api_endpoint.clone(), plugins_root).map_err(|error| {
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

fn init_telemetry() -> Option<slab_otel::OtelProvider> {
    slab_otel::provider::install_log_bridge();
    let settings = load_telemetry_settings();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match slab_otel::OtelProvider::from(&settings) {
        Ok(Some(provider)) => {
            if tracing_subscriber::registry()
                .with(env_filter)
                .with(provider.logger_layer())
                .with(provider.tracing_layer())
                .try_init()
                .is_ok()
            {
                Some(provider)
            } else {
                None
            }
        }
        Ok(None) => {
            let _ = tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
                .try_init();
            None
        }
        Err(error) => {
            eprintln!("WARN: failed to initialize slab-app telemetry: {error}");
            let _ = tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_thread_ids(true))
                .try_init();
            None
        }
    }
}

fn load_telemetry_settings() -> slab_otel::config::OtelSettings {
    let path = slab_utils::app_home::settings_path();
    let mut settings = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<slab_config::SettingsDocument>(&raw).ok())
        .map(|document| document.telemetry)
        .unwrap_or_default();
    if settings.service_name == "slab" {
        settings.service_name = "slab-app".to_owned();
    }
    if settings.service_version.is_none() {
        settings.service_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    }
    settings
}
