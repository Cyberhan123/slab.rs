#![allow(clippy::uninlined_format_args)]

extern crate bindgen;

use cargo_metadata::MetadataCommand;
use slab_libfetch::fetch_header;
use std::env;
use std::path::PathBuf;
fn get_workspace_root() -> std::path::PathBuf {
    let metadata = MetadataCommand::new()
        .no_deps() // 不加载依赖，速度极快
        .exec()
        .expect("Could not fetch cargo metadata");

    metadata.workspace_root.into_std_path_buf()
}
fn main() {
    let mainfest_dir = get_workspace_root();
    let ggml_include_path = mainfest_dir.join("slab-ggml-sys/target/ggml/include");
    let include_path = PathBuf::from("target/llama");

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        fetch_header("seasonjs", "llama.cpp-build", Some("v8464"), include_path.as_path())
            .await
            .expect("Failed to fetch llama headers");
    });

    println!("cargo:rerun-if-changed={}", include_path.display());
    println!("cargo:rerun-if-changed={}", ggml_include_path.display());

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_path.join("include").display()))
        .clang_arg(format!("-I{}", ggml_include_path.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .dynamic_library_name("LlamaLib")
        .generate();

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    match bindings {
        Ok(b) => {
            let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
            b.write_to_file(out_path.join("bindings.rs")).expect("Couldn't write bindings!");
        }
        Err(e) => {
            println!("cargo:warning=Unable to generate bindings: {}", e);
            println!("cargo:warning=Using bundled bindings.rs, which may be out of date");
            // copy src/bindings.rs to OUT_DIR
            std::fs::copy("src/bindings.rs", out.join("bindings.rs"))
                .expect("Unable to copy bindings.rs");
        }
    }
}
