//! The elevated daemon's accept loop. One named-pipe instance per concurrent client; each
//! connection runs in its own task. S2b1 handles only `Ping` (liveness); S2b2 extends
//! `handle_connection` to drive `Spawn`/`Output`/`Exited` for the real Low-IL child.

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

use crate::error::WindowsSandboxError;
use crate::pipe::{PipeFrame, read_frame, write_frame};

/// Run the daemon forever (until the process is killed). Creates the first pipe instance to
/// reserve the name, then loops: wait for a client, hand the connection to a task, create the
/// next instance.
pub async fn run_daemon(pipe_name: String) -> Result<(), WindowsSandboxError> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
        .map_err(|e| WindowsSandboxError::WindowsApi(format!("create pipe: {e}")))?;

    tracing::info!(%pipe_name, "slab-sandbox-helper daemon listening");
    loop {
        // Block until a client connects to this instance.
        if let Err(e) = server.connect().await {
            tracing::warn!(error = %e, "daemon: pipe connect failed");
            // Recreate the instance and continue (a transient client error shouldn't kill the daemon).
            server = ServerOptions::new()
                .create(&pipe_name)
                .map_err(|e2| WindowsSandboxError::WindowsApi(format!("recreate pipe: {e2}")))?;
            continue;
        }

        // Hand the connected instance off and create a fresh one for the next client.
        let next = match ServerOptions::new().create(&pipe_name) {
            Ok(next) => next,
            Err(e) => {
                tracing::error!(error = %e, "daemon: could not create next pipe instance; stopping");
                return Err(WindowsSandboxError::WindowsApi(format!("create next pipe: {e}")));
            }
        };
        let prev = std::mem::replace(&mut server, next);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(prev).await {
                tracing::warn!(error = %e, "daemon: connection handler failed");
            }
        });
    }
}

/// Handle one client connection. S2b1: a single Ping→Pong exchange. S2b2 will loop, dispatching
/// Spawn/Kill and relaying Output/Exited frames.
async fn handle_connection(server: NamedPipeServer) -> Result<(), WindowsSandboxError> {
    let (mut reader, mut writer) = tokio::io::split(server);
    let frame = read_frame(&mut reader).await?;
    match frame {
        PipeFrame::Ping { nonce } => {
            write_frame(&mut writer, &PipeFrame::Pong { nonce }).await?;
        }
        other => {
            tracing::warn!(?other, "daemon: unexpected frame in S2b1 (Spawn/Kill arrive in S2b2)");
        }
    }
    Ok(())
}
