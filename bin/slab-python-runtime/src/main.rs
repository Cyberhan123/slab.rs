use std::sync::Arc;

use slab_python_runtime::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio};
use slab_python_runtime::{PythonRuntime, PythonRuntimeConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    slab_utils::tracing::init_stderr_tracing("info");

    let host = Arc::new(JsonRpcRuntimeHost::new());
    let runtime = Arc::new(PythonRuntime::with_config(PythonRuntimeConfig {
        host: host.clone(),
        ..PythonRuntimeConfig::default()
    }));
    runtime.initialize()?;
    serve_stdio(host, runtime).await
}
