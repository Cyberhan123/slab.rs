use std::path::PathBuf;
#[cfg(feature = "lsp_runtime")]
use std::time::Duration;

use anyhow::bail;
#[cfg(feature = "lsp_runtime")]
use anyhow::{Context, anyhow};

#[cfg(feature = "lsp_runtime")]
use crate::{Module, Runtime, RuntimeOptions, deno_core};

pub async fn run(entry: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    run_inner(entry, args).await
}

#[cfg(feature = "lsp_runtime")]
async fn run_inner(entry: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    let entry = entry
        .canonicalize()
        .with_context(|| format!("failed to resolve LSP entry {}", entry.display()))?;
    if !entry.is_file() {
        bail!("LSP entry does not exist at {}", entry.display());
    }
    let entry_dir =
        entry.parent().ok_or_else(|| anyhow!("failed to resolve LSP entry parent directory"))?;
    let entry_specifier = deno_core::ModuleSpecifier::from_file_path(&entry)
        .map_err(|()| anyhow!("failed to convert LSP entry to file URL: {}", entry.display()))?;
    let entry_json = serde_json::to_string(entry_specifier.as_str())?;
    let args_json = serde_json::to_string(&args)?;
    let bootstrap = format!(
        r#"
import {{ Buffer as __SlabBuffer }} from "node:buffer";
globalThis.Buffer ??= __SlabBuffer;
globalThis.__SLAB_LSP_ARGS__ = {args_json};
await import({entry_json});
"#
    );

    let module = Module::new(entry_dir.join("__slab_lsp_bootstrap__.mjs"), bootstrap);
    let mut runtime = Runtime::with_tokio_runtime_handle(
        RuntimeOptions { timeout: Duration::MAX, ..Default::default() },
        tokio::runtime::Handle::current(),
    )?;
    runtime.set_current_dir(entry_dir)?;
    runtime.load_module_async(&module).await?;
    runtime.await_event_loop(deno_core::PollEventLoopOptions::default(), None).await?;
    Ok(())
}

#[cfg(not(feature = "lsp_runtime"))]
async fn run_inner(_entry: PathBuf, _args: Vec<String>) -> anyhow::Result<()> {
    bail!("slab-js-runtime was built without lsp_runtime support")
}
