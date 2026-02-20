#![allow(clippy::uninlined_format_args)]

extern crate bindgen;

use std::env;
use std::path::PathBuf;
use slab_libfetch::fetch_header;

fn main() {
    // 1. 定义你刚才下载头文件的路径
    let include_path = PathBuf::from("target/diffusion");
    // 2. 在 build.rs 中调用 fetch_header 来确保头文件存在
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        fetch_header("leejet", "diffusion.cpp", Some("master-504-636d3cb"), include_path.as_path())
            .await
            .expect("Failed to fetch diffusion headers");
    });
    // 2. 告诉 Cargo：如果这些头文件变了，就重新运行 build.rs
    println!("cargo:rerun-if-changed={}", include_path.display());
    // 3. 配置 Bindgen
    let bindings = bindgen::Builder::default()
        // 指定主头文件（diffusion.h 在 include 目录下）
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_path.join("include").display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .dynamic_library_name("DiffusionLib")
        .generate();

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    // 4. 将生成的代码写入 OUT_DIR
    match bindings {
        Ok(b) => {
            let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
            b.write_to_file(out_path.join("bindings.rs"))
                .expect("Couldn't write bindings!");
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
