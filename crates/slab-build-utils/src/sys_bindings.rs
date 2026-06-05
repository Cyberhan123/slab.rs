use crate::{
    configure_bindgen_builder, ensure_vendor_layout, generate_or_copy_bindings,
    sync_vendor_runtime_artifact_to_dir, workspace_root,
};
use anyhow::{Context, Result, anyhow};
use std::env;
use std::path::PathBuf;

pub fn generate_vendor_sys_bindings(
    primary_artifact: &str,
    include_deps: &[&str],
    dynamic_library_name: &str,
    extra_rerun_paths: &[&str],
) -> Result<()> {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=src/bindings.rs");
    for path in extra_rerun_paths {
        println!("cargo:rerun-if-changed={path}");
    }

    let layout = ensure_vendor_layout(primary_artifact, include_deps)
        .with_context(|| format!("failed to prepare {primary_artifact} vendor layout"))?;
    let out_dir = PathBuf::from(env::var("OUT_DIR").context("missing OUT_DIR")?);
    let fallback_source = PathBuf::from("src").join("bindings.rs");

    let mut include_dirs = Vec::with_capacity(include_deps.len() + 1);
    include_dirs.push(layout.primary.include_dir.clone());
    for dep in include_deps {
        let artifact = layout
            .artifact(dep)
            .ok_or_else(|| anyhow!("{dep} dependency should be present in vendor layout"))?;
        include_dirs.push(artifact.include_dir.clone());
    }

    let builder = configure_bindgen_builder("wrapper.h", &include_dirs, dynamic_library_name);
    generate_or_copy_bindings(builder, &out_dir, &fallback_source)
        .with_context(|| format!("failed to prepare {primary_artifact} bindings"))?;

    let target = env::var("TARGET").context("missing TARGET")?;
    let runtime_output_dir = workspace_root()?
        .join("bin")
        .join("slab-app")
        .join("src-tauri")
        .join("resources")
        .join("libs");
    sync_vendor_runtime_artifact_to_dir(&target, &layout.primary, &runtime_output_dir)
        .with_context(|| format!("failed to copy {primary_artifact} runtime libraries"))
}
