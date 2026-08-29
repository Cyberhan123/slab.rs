//! Elevated Windows helper entry point. The helper-side logic lives in the
//! `slab-windows-sandbox` library (so it is unit-testable); this binary is a thin clap dispatch.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "slab-sandbox-helper",
    about = "Elevated Windows helper that applies slab's OS-enforced sandbox isolation."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// One-shot: process a signed payload file (S2a Provision path).
    Payload {
        /// Path to the signed payload JSON file.
        payload: PathBuf,
    },
    /// (S2b) Start the long-lived elevated daemon on a named pipe.
    Serve {
        /// Named pipe path, e.g. `\\.\pipe\slab-sandbox-helper-<hash>`.
        pipe: String,
        /// Path to the DPAPI-sealed HMAC key. Defaults to `<app_home>/sandbox-helper.key`. The
        /// orchestrator passes its own key path so both sides share one key (HMAC must match).
        #[arg(long)]
        key: Option<PathBuf>,
        /// Path to the setup marker JSON. Defaults to `<app_home>/sandbox-marker.json`.
        #[arg(long)]
        marker: Option<PathBuf>,
        /// PID of the owner process (slab-server). The daemon exits as soon as this process dies.
        #[arg(long)]
        owner_pid: Option<u32>,
    },
    /// Print version.
    Version,
}

fn main() {
    let cli = Cli::parse();
    #[cfg(target_os = "windows")]
    {
        run(cli);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cli;
        eprintln!("slab-sandbox-helper is Windows-only; this build is a no-op.");
    }
}

#[cfg(target_os = "windows")]
fn run(cli: Cli) {
    match cli.command {
        Some(Command::Payload { payload }) => {
            let key_path = slab_utils::app_home::app_home_dir().join("sandbox-helper.key");
            let code = slab_windows_sandbox::run_payload(&payload, &key_path);
            std::process::exit(code);
        }
        Some(Command::Serve { pipe, key, marker, owner_pid }) => {
            // The long-lived elevated daemon. Runs until its owner process (`--owner-pid`) exits —
            // or it is killed. S2b1 handled Ping/Pong only; S2b2 drives Provision/Spawn/Kill for
            // the real Low-IL restricted-token child.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("slab-sandbox-helper: failed to start tokio runtime: {e}");
                    std::process::exit(1);
                }
            };
            let app_home = slab_utils::app_home::app_home_dir();
            let key_path = key.unwrap_or_else(|| app_home.join("sandbox-helper.key"));
            let marker_path = marker.unwrap_or_else(|| app_home.join("sandbox-marker.json"));
            let code = rt.block_on(async {
                match slab_windows_sandbox::run_daemon(pipe, key_path, marker_path, owner_pid).await
                {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("slab-sandbox-helper daemon exited: {e}");
                        1
                    }
                }
            });
            std::process::exit(code);
        }
        Some(Command::Version) | None => {
            println!("slab-sandbox-helper {}", env!("CARGO_PKG_VERSION"));
        }
    }
}
