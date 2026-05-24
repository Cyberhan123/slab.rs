pub mod api;
mod bundle;
mod domain;
mod host_bridge;
mod interpreter;
mod permissions;
mod security;
mod stdlib;
mod vfs;
mod worker;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, bail};
use dashmap::DashMap;
use slab_types::{PluginRuntimeCallRequest, PluginRuntimeCallResponse};

use bundle::load_python_bundle;
pub use domain::RuntimeHost;
pub use permissions::PythonPluginPermissions;
pub use vfs::EmbeddedStdlib;
use worker::PythonWorkerHandle;

/// Configuration for the Python runtime's host environment.
#[derive(Clone)]
pub struct PythonRuntimeConfig {
    /// Callback transport used by the Python `slab` bridge.
    pub host: Arc<dyn RuntimeHost>,
    /// Python source modules to register in the embedded stdlib VFS.
    pub embedded_stdlib: EmbeddedStdlib,
}

impl Default for PythonRuntimeConfig {
    fn default() -> Self {
        Self {
            host: Arc::new(domain::DenyRuntimeHost),
            embedded_stdlib: stdlib::default_embedded_stdlib(),
        }
    }
}

/// The top-level Python plugin runtime managing per-plugin workers.
pub struct PythonRuntime {
    workers: DashMap<String, Arc<PythonWorkerHandle>>,
    config: PythonRuntimeConfig,
}

impl PythonRuntime {
    pub fn new() -> Self {
        Self { workers: DashMap::new(), config: PythonRuntimeConfig::default() }
    }

    pub fn with_config(config: PythonRuntimeConfig) -> Self {
        Self { workers: DashMap::new(), config }
    }

    pub fn initialize(&self) -> Result<()> {
        interpreter::init(self.config.embedded_stdlib.clone())
    }

    pub async fn call(
        &self,
        request: PluginRuntimeCallRequest,
    ) -> Result<PluginRuntimeCallResponse> {
        let resolved_entry =
            resolve_python_entry(&request.root_dir, &request.entry, request.bundle.as_deref())?;
        let worker = match self.workers.entry(request.plugin_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(worker_entry) => worker_entry.get().clone(),
            dashmap::mapref::entry::Entry::Vacant(worker_entry) => {
                let handle =
                    Arc::new(PythonWorkerHandle::new(resolved_entry.clone(), self.config.clone())?);
                worker_entry.insert(handle.clone());
                handle
            }
        };
        worker.call(request, resolved_entry).await
    }

    pub fn unload(&self, plugin_id: &str) {
        self.workers.remove(plugin_id);
    }
}

impl Default for PythonRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub(crate) enum ResolvedPythonEntry {
    File { path: PathBuf },
    Bundle { entry_source: String, modules: EmbeddedStdlib, path: PathBuf },
}

impl ResolvedPythonEntry {
    pub fn cache_key(&self) -> String {
        match self {
            Self::File { path } | Self::Bundle { path, .. } => path.to_string_lossy().into_owned(),
        }
    }
}

fn resolve_python_entry(
    root_dir: &str,
    entry: &str,
    bundle: Option<&str>,
) -> Result<ResolvedPythonEntry> {
    let entry_path = Path::new(entry);
    if entry_path.is_absolute() {
        bail!("runtime.python.entry must be relative to the plugin root");
    }
    if entry_path.extension().and_then(|extension| extension.to_str()) != Some("py") {
        bail!("runtime.python.entry must use .py");
    }

    let root = PathBuf::from(root_dir).canonicalize().map_err(|error| {
        anyhow::anyhow!("failed to canonicalize plugin root `{root_dir}`: {error}")
    })?;

    if let Some(bundle) = bundle {
        let bundle_path = Path::new(bundle);
        if bundle_path.is_absolute() {
            bail!("runtime.python.bundle must be relative to the plugin root");
        }
        if bundle_path.extension().and_then(|extension| extension.to_str()) != Some("slabpy") {
            bail!("runtime.python.bundle must use .slabpy");
        }
        let bundle_path = root.join(bundle_path).canonicalize().map_err(|error| {
            anyhow::anyhow!("failed to canonicalize Python bundle `{bundle}`: {error}")
        })?;
        if !bundle_path.starts_with(&root) {
            bail!("runtime.python.bundle must stay inside the plugin root");
        }
        if !bundle_path.is_file() {
            bail!("Python bundle does not exist at {}", bundle_path.display());
        }
        let bundle = load_python_bundle(&bundle_path)?;
        return Ok(ResolvedPythonEntry::Bundle {
            entry_source: bundle.entry_source,
            modules: bundle.modules,
            path: bundle_path,
        });
    }

    let module_path = root.join(entry_path).canonicalize().map_err(|error| {
        anyhow::anyhow!("failed to canonicalize Python entry `{entry}`: {error}")
    })?;
    if !module_path.starts_with(&root) {
        bail!("runtime.python.entry must stay inside the plugin root");
    }
    if !module_path.is_file() {
        bail!("Python module entry does not exist at {}", module_path.display());
    }
    Ok(ResolvedPythonEntry::File { path: module_path })
}
