use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use serde::Deserialize;

use crate::vfs::EmbeddedStdlib;

/// Loaded Slab Python bundle source.
pub struct LoadedPythonBundle {
    pub entry_source: String,
    pub modules: EmbeddedStdlib,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlabPythonBundle {
    entry_module: String,
    format: String,
    #[serde(default)]
    modules: Vec<SlabPythonBundleModule>,
    #[serde(default)]
    native_extensions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlabPythonBundleModule {
    #[serde(default)]
    is_package: bool,
    name: String,
    source_base64: String,
}

pub fn load_python_bundle(path: &Path) -> Result<LoadedPythonBundle> {
    let raw = std::fs::read(path)
        .with_context(|| format!("failed to read Python bundle {}", path.display()))?;
    let bundle: SlabPythonBundle = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse Python bundle {}", path.display()))?;
    if bundle.format != "slab.python.bundle.v1" {
        bail!("unsupported Python bundle format `{}`", bundle.format);
    }
    if !bundle.native_extensions.is_empty() {
        bail!(
            "Python bundle contains unsupported native extensions: {}",
            bundle.native_extensions.join(", ")
        );
    }

    let mut modules = EmbeddedStdlib::default();
    let mut entry_source = None;
    for module in bundle.modules {
        let source = base64::engine::general_purpose::STANDARD
            .decode(module.source_base64.as_bytes())
            .with_context(|| format!("failed to decode bundled module `{}`", module.name))?;
        if module.name == bundle.entry_module {
            entry_source = Some(
                String::from_utf8(source.clone())
                    .with_context(|| format!("bundled module `{}` is not utf-8", module.name))?,
            );
        }
        if module.is_package {
            modules.add_owned_package(module.name, source);
        } else {
            modules.add_owned_module(module.name, source);
        }
    }

    let Some(entry_source) = entry_source else {
        bail!("Python bundle is missing entry module `{}`", bundle.entry_module);
    };

    Ok(LoadedPythonBundle { entry_source, modules })
}
