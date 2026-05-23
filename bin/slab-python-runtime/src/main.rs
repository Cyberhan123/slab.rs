use clap::Parser;
use slab_python_runtime::{PythonRuntime, PythonRuntimeConfig};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "slab-python-runtime", about = "Slab Python plugin runtime")]
struct Cli {
    /// Base URL for the slab HTTP API.
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    api_base_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let cli = Cli::parse();

    let config =
        PythonRuntimeConfig { api_base_url: cli.api_base_url, ..PythonRuntimeConfig::default() };
    let _runtime = PythonRuntime::with_config(config);

    tracing::info!("slab-python-runtime ready");

    // Block until signalled.
    tokio::signal::ctrl_c().await?;
    Ok(())
}
