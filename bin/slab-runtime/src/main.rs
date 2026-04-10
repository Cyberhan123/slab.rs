// Prevents an extra console window when the runtime is launched as a Tauri
// sidecar from the packaged desktop app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;

use slab_runtime::infra::config::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    slab_runtime::api::server::run(Cli::parse()).await
}
