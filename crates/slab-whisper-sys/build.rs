#![allow(clippy::uninlined_format_args)]

extern crate bindgen;

use slab_build_utils::ensure_vendor_layout;
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

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", layout.primary.include_dir.display()))
        .clang_arg(format!("-I{}", ggml_include_path.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .dynamic_library_name("WhisperLib")
        .generate();

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    match bindings {
        Ok(b) => {
            b.write_to_file(out_path.join("bindings.rs")).expect("Couldn't write bindings!");
        }
        Err(e) => {
            println!("cargo:warning=Unable to generate bindings: {}", e);
            println!("cargo:warning=Using bundled bindings.rs, which may be out of date");
            let bundled = PathBuf::from("src").join("bindings.rs");
            if !bundled.exists() {
                panic!(
                    "Unable to generate bindings and bundled fallback is missing at {}",
                    bundled.display()
                );
            }
        }
    }
}
