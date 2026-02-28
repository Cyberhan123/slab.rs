use std::env;

use tauri::path::BaseDirectory;
use tauri::Manager;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

pub fn run_server_sidecar(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle();

    let mut sidecar_command = app_handle.shell().sidecar("slab-server")?;

    let lib_path = app.path().resolve("resources/lib", BaseDirectory::Resource)?;
    sidecar_command = sidecar_command
        .env("SLAB_BIND", "127.0.0.1:3000")
        .env("SLAB_LIB_DIR", lib_path.to_str().unwrap());

    let (mut rx, mut _child) = sidecar_command.spawn().expect("Failed to spawn sidecar");

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    println!("📤 [Sidecar STDOUT]: {}", msg);
                }
                CommandEvent::Stderr(line) => {
                    let msg = String::from_utf8_lossy(&line);
                    eprintln!("📥 [Sidecar STDERR]: {}", msg);
                }
                CommandEvent::Error(err) => {
                    eprintln!("❌ [Sidecar ERROR]: {}", err);
                }
                CommandEvent::Terminated(payload) => {
                    println!(
                        "⚠️ [Sidecar TERMINATED] by signal {:?} code {:?}",
                        payload.signal, payload.code
                    );
                }
                (event) => {
                    println!("⚠️ [Sidecar Unknown] by event {:?}", event);
                }
            }
        }
    });
    println!("🚀 Slab Sidecar started successfully ");

    Ok(())
}
