use anyhow::Context;
use serde_json::Value;
use slab_mcp_server::{handle_message, parse_error_response};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await.context("failed to read stdin")?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(message) => handle_message(message),
            Err(error) => Some(parse_error_response(error.to_string())),
        };

        if let Some(response) = response {
            stdout
                .write_all(response.to_string().as_bytes())
                .await
                .context("failed to write stdout")?;
            stdout.write_all(b"\n").await.context("failed to write stdout newline")?;
            stdout.flush().await.context("failed to flush stdout")?;
        }
    }

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("slab_mcp_server=info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).with_writer(std::io::stderr).init();
}
