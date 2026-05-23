use std::sync::Arc;

use crate::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio};
use crate::infra::deno::DenoPluginExecutor;

pub fn run() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let host = Arc::new(JsonRpcRuntimeHost::new());
        let executor = Arc::new(DenoPluginExecutor::new(host.clone()));
        serve_stdio(host, executor).await
    })
}
