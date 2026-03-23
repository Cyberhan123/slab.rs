use tauri::Manager;
use tauri_plugin_decorum::WebviewWindowExt;

pub fn setup_windows(app: &mut tauri::App) -> tauri::Result<()> {
     let main_window = app.get_webview_window("main").unwrap();
     main_window.create_overlay_titlebar().unwrap();
    // Some macOS-specific helpers
    #[cfg(target_os = "macos")]
    {
        // Set a custom inset to the traffic lights
        main_window.set_traffic_lights_inset(12.0, 16.0)?;

        // Make window transparent without privateApi
        main_window.make_transparent()?;

        // Set window level
        // NSWindowLevel: https://developer.apple.com/documentation/appkit/nswindowlevel
        main_window.set_window_level(25)?;
    }

    Ok(())
}
