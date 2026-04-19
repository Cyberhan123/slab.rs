use std::io::ErrorKind;

use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};

pub(super) async fn wait_for_stdin() {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                info!("stdin closed; shutting down runtime");
                break;
            }
            Ok(_) => {
                let cmd = line.trim();
                if cmd.eq_ignore_ascii_case("shutdown")
                    || cmd.eq_ignore_ascii_case("exit")
                    || cmd.eq_ignore_ascii_case("quit")
                {
                    info!(command = %cmd, "received shutdown command from stdin");
                    break;
                }
            }
            Err(error) => {
                if error.kind() != ErrorKind::Interrupted {
                    warn!(
                        error = %error,
                        "failed reading stdin for shutdown command; shutting down runtime"
                    );
                    break;
                }
            }
        }
    }
}

pub(super) async fn shutdown_signal(listen_stdin: bool) {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            warn!(error = %error, "failed to install CTRL+C signal handler");
        }
        "ctrl_c"
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => warn!(error = %error, "failed to install SIGTERM handler"),
        }
        "sigterm"
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<&'static str>();

    let stdin_signal = async {
        if listen_stdin {
            wait_for_stdin().await;
            "stdin"
        } else {
            std::future::pending::<&'static str>().await
        }
    };

    let source = tokio::select! {
        source = ctrl_c => source,
        source = terminate => source,
        source = stdin_signal => source,
    };
    info!(source, "shutdown signal received; shutting down runtime");
}
