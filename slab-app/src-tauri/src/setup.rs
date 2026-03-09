use tauri::path::BaseDirectory;
use tauri::Manager;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

pub fn run_server_sidecar(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();
    let lib_path = app
        .path()
        .resolve("resources/lib", BaseDirectory::Resource)?;
    let lib_path_str = lib_path.to_str().ok_or("invalid lib path")?;

    let sidecar_command = app_handle.shell().sidecar("slab-server")?.args([
        "--gateway-bind",
        "127.0.0.1:3000",
        "--whisper-bind",
        "127.0.0.1:3001",
        "--llama-bind",
        "127.0.0.1:3002",
        "--lib-dir",
        lib_path_str,
    ]);

    let (mut rx, mut _child) = sidecar_command.spawn().expect("failed to spawn sidecar");

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    println!("[Sidecar STDOUT] {}", msg);
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    eprintln!("[Sidecar STDERR] {}", msg);
                }
                CommandEvent::Error(err) => {
                    eprintln!("[Sidecar ERROR] {}", err);
                }
                CommandEvent::Terminated(payload) => {
                    println!(
                        "[Sidecar TERMINATED] signal {:?} code {:?}",
                        payload.signal, payload.code
                    );
                }
                other => {
                    println!("[Sidecar Event] {:?}", other);
                }
            }
        }
    });

    println!("Slab sidecar started");
    Ok(())
}
