#![allow(clippy::uninlined_format_args)]

use slab_build_utils::{
    configure_bindgen_builder, ensure_vendor_layout, generate_or_copy_bindings,
};
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=src/bindings.rs");

    let layout = ensure_vendor_layout("ggml", &[]).expect("Failed to prepare ggml vendor layout");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let fallback_source = PathBuf::from("src").join("bindings.rs");

    let builder =
        configure_bindgen_builder("wrapper.h", [&layout.primary.include_dir], "GGmlBaseLib");

    generate_or_copy_bindings(builder, &out_dir, &fallback_source)
        .expect("failed to prepare ggml bindings");
}
