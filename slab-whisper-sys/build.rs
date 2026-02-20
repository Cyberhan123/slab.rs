#![allow(clippy::uninlined_format_args)]

extern crate bindgen;

use slab_libfetch::fetch_header;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // 1. 定义你刚才下载头文件的路径
    let include_path = PathBuf::from("target/whisper");
    // 2. 在 build.rs 中调用 fetch_header 来确保头文件存在
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        fetch_header(
            "ggml-org",
            "whisper.cpp",
            Some("v1.8.3"),
            include_path.as_path(),
        )
        .await
        .expect("Failed to fetch whisper headers");
    });
    // 2. 告诉 Cargo：如果这些头文件变了，就重新运行 build.rs
    println!("cargo:rerun-if-changed={}", include_path.display());
    // 3. 配置 Bindgen
    let bindings = bindgen::Builder::default()
        // 指定主头文件（whisper.h 在 include 目录下）
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_path.join("include").display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .dynamic_library_name("WhisperLib")
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
    let profile = env::var("PROFILE").unwrap();
    if profile == "debug" {
        // 传入文件夹路径和需要匹配的后缀
        copy_assets_to_out_dir("../testdata/whisper");
    }
}

fn copy_assets_to_out_dir(rel_src_dir: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    // 1. 确定源目录和目标目录
    let src_dir = Path::new(&manifest_dir).join(rel_src_dir);
    let dest_dir = PathBuf::from(out_dir).join("../../../");
    let deps_dir = dest_dir.join("deps");

    // 2. 定义动态库后缀
    let extension = if cfg!(windows) {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };

    // 3. 遍历并拷贝
    if src_dir.exists() && src_dir.is_dir() {
        // 读取文件夹内容
        for entry in fs::read_dir(src_dir).expect("无法读取源目录") {
            let entry = entry.expect("读取目录项失败");
            let path = entry.path();

            // 仅处理文件且后缀匹配
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some(extension) {
                let file_name = path.file_name().unwrap();

                // 拷贝到 target/debug
                let dest_file = dest_dir.join(file_name);
                fs::copy(&path, &dest_file).ok();

                // 拷贝到 target/debug/deps (为了单元测试)
                let deps_file = deps_dir.join(file_name);
                fs::copy(&path, &deps_file).ok();

                // 告诉 Cargo：如果这个 DLL 变了，重新运行 build.rs
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }

        // 将源目录加入搜索路径
        println!("cargo:rustc-link-search=native={}", rel_src_dir);
    } else {
        println!("cargo:warning=源目录不存在: {}", src_dir.display());
    }
}
