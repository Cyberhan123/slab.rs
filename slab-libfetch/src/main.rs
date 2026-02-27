//! slab-libfetch - Library fetcher for Slab backend dependencies
//!
//! This tool automatically downloads the appropriate GGML backend libraries
//! (Whisper, Llama, Stable Diffusion) for the current platform.

use std::env;
use std::path::PathBuf;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs::File;
use std::io::{Cursor, Write};
use tar::Archive;
use tokio::runtime::Runtime;
use zip::ZipArchive;

fn main() -> Result<()> {
    println!("==========================================");
    println!("  Slab Library Fetcher");
    println!("==========================================");
    println!();

    // Detect platform
    let platform = detect_platform();
    println!("Detected platform: {}", platform.name);
    println!();

    // Create libraries directory
    let lib_dir = get_lib_dir();
    println!("Library directory: {}", lib_dir.display());
    println!();

    std::fs::create_dir_all(&lib_dir).context("Failed to create library directory")?;

    // Create HTTP client
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    // Fetch libraries
    let rt = Runtime::new()?;
    rt.block_on(async {
        fetch_whisper(&client, &platform, &lib_dir).await;
        fetch_llama(&client, &platform, &lib_dir).await;
        fetch_diffusion(&client, &platform, &lib_dir).await;
    });

    println!();
    println!("==========================================");
    println!("  All libraries downloaded!");
    println!("==========================================");
    println!();
    println!("Set the following environment variables:");
    println!("  export SLAB_LLAMA_LIB_DIR={}/llama", lib_dir.display());
    println!("  export SLAB_WHISPER_LIB_DIR={}/whisper", lib_dir.display());
    println!("  export SLAB_DIFFUSION_LIB_DIR={}/diffusion", lib_dir.display());

    Ok(())
}

#[derive(Debug)]
struct Platform {
    name: String,
    os: String,
    arch: String,
    lib_extension: String,
}

fn detect_platform() -> Platform {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| {
        if cfg!(target_os = "linux") { "linux".to_string() }
        else if cfg!(target_os = "macos") { "macos".to_string() }
        else if cfg!(target_os = "windows") { "windows".to_string() }
        else { "unknown".to_string() }
    });

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| {
        if cfg!(target_arch = "x86_64") { "x86_64".to_string() }
        else if cfg!(target_arch = "aarch64") { "aarch64".to_string() }
        else { "unknown".to_string() }
    });

    let (name, lib_extension) = match (os.as_str(), arch.as_str()) {
        ("linux", "x86_64") => ("Linux (x86_64)".to_string(), "so".to_string()),
        ("linux", "aarch64") => ("Linux (ARM64)".to_string(), "so".to_string()),
        ("macos", "x86_64") => ("macOS (Intel)".to_string(), "dylib".to_string()),
        ("macos", "aarch64") => ("macOS (Apple Silicon)".to_string(), "dylib".to_string()),
        ("windows", "x86_64") => ("Windows (x86_64)".to_string(), "dll".to_string()),
        ("windows", "aarch64") => ("Windows (ARM64)".to_string(), "dll".to_string()),
        _ => (format!("Unknown ({})", os), "unknown".to_string()),
    };

    Platform { name, os, arch, lib_extension }
}

fn get_lib_dir() -> PathBuf {
    if let Ok(dir) = env::var("SLAB_LIB_DIR") {
        PathBuf::from(dir)
    } else {
        let mut path = env::current_dir().expect("Failed to get current directory");
        path.push("libraries");
        path
    }
}

async fn download_file(client: &Client, url: &str) -> Result<Vec<u8>> {
    println!("  Downloading: {}", url);

    let response = client.get(url).send().await
        .context("Failed to initiate download")?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    let bytes = response.bytes().await
        .context("Failed to download file")?;

    println!("  ✅ Downloaded {} bytes", bytes.len());
    Ok(bytes.to_vec())
}

fn extract_tar_gz(bytes: &[u8], dest_dir: &PathBuf) -> Result<()> {
    println!("  Extracting tar.gz archive...");

    let cursor = Cursor::new(bytes);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    archive.unpack(dest_dir).context("Failed to extract tar.gz archive")?;
    println!("  ✅ Extraction complete");
    Ok(())
}

fn extract_zip(bytes: &[u8], dest_dir: &PathBuf, filter: Option<&str>) -> Result<()> {
    println!("  Extracting zip archive...");

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("Failed to read zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to get file")?;
        let file_name = file.name().to_string();

        // Apply filter if specified
        if let Some(pattern) = filter {
            if !file_name.contains(pattern) {
                continue;
            }
        }

        let dest_path = dest_dir.join(
            PathBuf::from(file_name)
                .file_name()
                .context("Invalid filename")?
        );

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut dest_file = File::create(&dest_path)?;
            std::io::copy(&mut file, &mut dest_file)?;
        }
    }

    println!("  ✅ Extraction complete");
    Ok(())
}

async fn fetch_whisper(client: &Client, platform: &Platform, lib_dir: &PathBuf) {
    println!("📦 Fetching Whisper library...");

    let whisper_dir = lib_dir.join("whisper");
    std::fs::create_dir_all(&whisper_dir).expect("Failed to create whisper directory");

    // Whisper v1.8.3 - Build from source for Linux, download for others
    let version = "v1.8.3";
    let repo_url = "https://github.com/ggml-org/whisper.cpp";

    match platform.os.as_str() {
        "linux" => {
            println!("  ⚠️  Whisper must be built from source on Linux.");
            println!("  Please run:");
            println!("    git clone --depth 1 --branch {} {}", version, repo_url);
            println!("    cd whisper.cpp && cmake -B build && cmake --build build -j");
            println!("    cp build/src/libwhisper.so* {}", whisper_dir.display());
        }
        "macos" => {
            let filename = "libwhisper.dylib";
            let url = format!("{}/releases/download/{}/{}", repo_url, version, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = whisper_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Whisper library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        "windows" => {
            let filename = "whisper.dll";
            let url = format!("{}/releases/download/{}/{}", repo_url, version, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = whisper_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Whisper library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        _ => {
            println!("  ⚠️  Unsupported platform: {}", platform.name);
        }
    }
    println!();
}

async fn fetch_llama(client: &Client, platform: &Platform, lib_dir: &PathBuf) {
    println!("📦 Fetching Llama library...");

    let llama_dir = lib_dir.join("llama");
    std::fs::create_dir_all(&llama_dir).expect("Failed to create llama directory");

    let version = "b8170";
    let base_url = "https://github.com/ggml-org/llama.cpp/releases/download";

    match platform.os.as_str() {
        "linux" if platform.arch == "x86_64" => {
            let url = format!("{}/llama-{}-bin-ubuntu-x64.tar.gz", base_url, version);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let temp_dir = std::env::temp_dir().join("llama-extract");
                    std::fs::create_dir_all(&temp_dir).unwrap();

                    if let Err(e) = extract_tar_gz(&bytes, &temp_dir) {
                        println!("  ❌ Failed to extract: {}", e);
                        return;
                    }

                    // Copy library files
                    let extracted_dir = temp_dir.join(format!("llama-{}-bin-ubuntu-x64", version));
                    if let Ok(entries) = std::fs::read_dir(&extracted_dir) {
                        for entry in entries.flatten() {
                            let file_name = entry.file_name();
                            if file_name.to_string_lossy().starts_with("libllama.so") {
                                let dest = llama_dir.join(&file_name);
                                let _ = std::fs::copy(entry.path(), &dest);
                            }
                        }
                        println!("  ✅ Llama library downloaded");
                    }

                    let _ = std::fs::remove_dir_all(temp_dir);
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        "macos" => {
            let filename = "libllama.dylib";
            let url = format!("{}/{}/{}", base_url, version, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = llama_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Llama library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        "windows" => {
            let filename = "llama.dll";
            let url = format!("{}/{}/{}", base_url, version, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = llama_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Llama library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        _ => {
            println!("  ⚠️  Unsupported platform: {}", platform.name);
        }
    }
    println!();
}

async fn fetch_diffusion(client: &Client, platform: &Platform, lib_dir: &PathBuf) {
    println!("📦 Fetching Stable Diffusion library...");

    let diffusion_dir = lib_dir.join("diffusion");
    std::fs::create_dir_all(&diffusion_dir).expect("Failed to create diffusion directory");

    let commit = "master-507-b314d80";
    let base_url = "https://github.com/leejet/stable-diffusion.cpp/releases/download";

    match platform.os.as_str() {
        "linux" if platform.arch == "x86_64" => {
            let url = format!("{}/{}", base_url, "sd-master-b314d80-bin-Linux-Ubuntu-24.04-x86_64.zip");

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let temp_dir = std::env::temp_dir().join("sd-extract");
                    std::fs::create_dir_all(&temp_dir).unwrap();

                    if let Err(e) = extract_zip(&bytes, &temp_dir, Some("libstable-diffusion")) {
                        println!("  ❌ Failed to extract: {}", e);
                        return;
                    }

                    // Move the library to final location
                    let src_lib = temp_dir.join("libstable-diffusion.so");
                    if src_lib.exists() {
                        let dest_lib = diffusion_dir.join("libstable-diffusion.so");
                        let _ = std::fs::rename(&src_lib, &dest_lib);
                        println!("  ✅ Stable Diffusion library downloaded");
                    }

                    let _ = std::fs::remove_dir_all(temp_dir);
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        "macos" => {
            let filename = "libstable-diffusion.dylib";
            let url = format!("{}/{}/{}", base_url, commit, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = diffusion_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Stable Diffusion library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        "windows" => {
            let filename = "stable-diffusion.dll";
            let url = format!("{}/{}/{}", base_url, commit, filename);

            match download_file(client, &url).await {
                Ok(bytes) => {
                    let dest = diffusion_dir.join(filename);
                    let mut file = File::create(&dest).unwrap();
                    file.write_all(&bytes).unwrap();
                    println!("  ✅ Stable Diffusion library downloaded");
                }
                Err(e) => {
                    println!("  ❌ Failed to download: {}", e);
                }
            }
        }
        _ => {
            println!("  ⚠️  Unsupported platform: {}", platform.name);
        }
    }
    println!();
}
