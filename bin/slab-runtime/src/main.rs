// Prevents an extra console window when the runtime is launched as a Tauri
// sidecar from the packaged desktop app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slab_runtime::bootstrap::{self, Cli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::run(Cli::parse()).await
}
