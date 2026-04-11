use tauri::Manager;
use tauri_plugin_decorum::WebviewWindowExt;

pub fn setup_windows(app: &mut tauri::App) -> tauri::Result<()> {
    let Some(main_window) = app.get_webview_window("main") else {
        log::warn!("Skipping window setup because the main webview window is unavailable");
        return Ok(());
    };

    if let Err(error) = main_window.create_overlay_titlebar() {
        log::warn!("Failed to create the overlay titlebar for the main window: {error}");
    }

    // Some macOS-specific helpers
    #[cfg(target_os = "macos")]
    {
        // Set a custom inset to the traffic lights
        if let Err(error) = main_window.set_traffic_lights_inset(12.0, 16.0) {
            log::warn!("Failed to set the traffic lights inset for the main window: {error}");
        }

        // Make window transparent without privateApi
        if let Err(error) = main_window.make_transparent() {
            log::warn!("Failed to make the main window transparent: {error}");
        }

        // Set window level
        // NSWindowLevel: https://developer.apple.com/documentation/appkit/nswindowlevel
        if let Err(error) = main_window.set_window_level(25) {
            log::warn!("Failed to set the main window level: {error}");
        }
    }

    Ok(())
}
