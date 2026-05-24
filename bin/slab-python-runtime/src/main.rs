use std::sync::Arc;

use slab_python_runtime::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio};
use slab_python_runtime::{PythonRuntime, PythonRuntimeConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).with_writer(std::io::stderr).init();

    let host = Arc::new(JsonRpcRuntimeHost::new());
    let runtime = Arc::new(PythonRuntime::with_config(PythonRuntimeConfig {
        host: host.clone(),
        ..PythonRuntimeConfig::default()
    }));
    runtime.initialize()?;
    serve_stdio(host, runtime).await
}
