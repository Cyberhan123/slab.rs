#[cfg(target_os = "macos")]
pub fn setup_windows(app: &mut tauri::App) -> tauri::Result<()> {
    use tauri::{TitleBarStyle, WebviewUrl, WebviewWindowBuilder};
    let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Slab")
        .inner_size(1280.0, 1040.0);
    let win_builder = win_builder.title_bar_style(TitleBarStyle::Transparent);
    let window = win_builder.build()?;
    {
        use cocoa::appkit::{NSColor, NSWindow};
        use cocoa::base::{id, nil};

        let ns_window = window.ns_window().unwrap() as id;
        unsafe {
            let bg_color = NSColor::colorWithRed_green_blue_alpha_(
                nil,
                50.0 / 255.0,
                158.0 / 255.0,
                163.5 / 255.0,
                1.0,
            );
            ns_window.setBackgroundColor_(bg_color);
        }
    }

    Ok(())
}
#[cfg(target_os = "windows")]
pub fn setup_windows(_app: &mut tauri::App) -> tauri::Result<()> {
    Ok(())
}
