#![allow(clippy::uninlined_format_args)]

use slab_build_utils::{
    configure_bindgen_builder, ensure_vendor_layout, generate_or_copy_bindings,
};
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=src/bindings.rs");
    println!("cargo:rerun-if-changed=../../vendor/whisper/include/whisper.h");

    let layout = ensure_vendor_layout("whisper", &["ggml"])
        .expect("Failed to prepare whisper vendor layout");
    let ggml_include_path = &layout
        .artifact("ggml")
        .expect("ggml dependency should be present in vendor layout")
        .include_dir;
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let fallback_source = PathBuf::from("src").join("bindings.rs");

    let builder = configure_bindgen_builder(
        "wrapper.h",
        [&layout.primary.include_dir, ggml_include_path],
        "WhisperLib",
    );

    generate_or_copy_bindings(builder, &out_dir, &fallback_source)
        .expect("failed to prepare whisper bindings");
}
