use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use octocrab;
use reqwest;
use std::fs;
use std::io::Cursor;
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::Archive;

const VERSION_FILE: &str = ".version";

/// 检查版本文件，如果当前版本已存在则返回 true
fn check_version_file(target_path: &Path, expected_version: &str) -> Result<bool> {
    let version_file_path = target_path.join(VERSION_FILE);

    if version_file_path.exists() {
        let current_version = fs::read_to_string(&version_file_path)
            .context("无法读取版本文件")?;

        if current_version.trim() == expected_version {
            return Ok(true);
        }
    }

    Ok(false)
}

/// 写入版本文件
fn write_version_file(target_path: &Path, version: &str) -> Result<()> {
    let version_file_path = target_path.join(VERSION_FILE);

    let mut file = fs::File::create(&version_file_path)
        .context("无法创建版本文件")?;

    file.write_all(version.as_bytes())
        .context("无法写入版本文件")?;

    Ok(())
}


/// 提取所有头文件（.h, .hpp, .hxx），跳过根目录
fn extract_header_files(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();

    // 跳过根目录（通常是 {repo}-{tag} 这样的目录）
    if comps.len() <= 1 {
        return None;
    }

    // 只提取头文件
    if let Some(file_name) = path.file_name() {
        let name = file_name.to_string_lossy();
        if name.ends_with(".h") || name.ends_with(".hpp") || name.ends_with(".hxx") {
            // 返回跳过根目录后的相对路径
            return Some(comps.iter().skip(1).collect());
        }
    }

    None
}

fn extract_include_part(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();
    if let Some(index) = comps.iter().position(|c| c.as_os_str() == "include") {
        return Some(comps.iter().skip(index).collect());
    }
    None
}

pub async fn fetch_header(
    owner: &str,
    repo: &str,
    tag: Option<&str>,
    target_include_path: &Path,
) -> Result<()> {
    // Ignore error if a provider has already been installed
    let _ = rustls::crypto::ring::default_provider().install_default();

    let instance = octocrab::instance();
    let repo_handler = instance.repos(owner, repo);
    let release_handler = repo_handler.releases();

    let version = match tag {
        Some(t) => {
           t.to_string()
        }
        None => {
            println!("🔍 未指定版本，正在获取 {}/{} 的最新版本...", owner, repo);
            let release= release_handler
                .get_latest()
                .await
                .context("无法获取最新 Release")?;
            release.tag_name
        }
    };

    // 检查是否已经下载过该版本
    if check_version_file(target_include_path, &version)? {
        println!("✅ 版本 {} 的头文件已存在，跳过下载。", version);
        return Ok(());
    }

    let tarball_url = format!(
        "https://github.com/{}/{}/archive/refs/tags/{}.tar.gz",
        owner, repo, version
    );

    println!("🚀 正在从 {} 下载...", tarball_url);

    // 清理旧的头文件目录
    if target_include_path.exists() {
        fs::remove_dir_all(target_include_path)
            .context("无法清理旧的头文件目录")?;
    }
    fs::create_dir_all(target_include_path)
        .context("无法创建目标目录")?;

    // 执行下载与流式解压
    let response = reqwest::get(tarball_url).await?.error_for_status()?;
    let bytes = response.bytes().await?;

    // 第一次尝试：提取 include 目录
    let tar_gz = GzDecoder::new(Cursor::new(bytes.clone()));
    let mut archive = Archive::new(tar_gz);

    let mut has_include_dir = false;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let full_path = entry.path()?.to_path_buf();

        // 优先提取 include 目录下的文件
        if let Some(rel_path) = extract_include_part(&full_path) {
            has_include_dir = true;
            let dest = target_include_path.join(rel_path);
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            entry.unpack(dest)?;
        }
    }

    // 如果没有 include 目录，则提取所有头文件
    if !has_include_dir {
        println!("⚠️  未找到 include 目录，提取所有头文件...");
        let tar_gz = GzDecoder::new(Cursor::new(bytes));
        let mut archive = Archive::new(tar_gz);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let full_path = entry.path()?.to_path_buf();

            // 跳过根目录，提取所有 .h 和 .hpp 文件
            if let Some(rel_path) = extract_header_files(&full_path) {
                let dest = target_include_path.join(rel_path);
                if let Some(p) = dest.parent() {
                    fs::create_dir_all(p)?;
                }
                entry.unpack(dest)?;
            }
        }
    }

    // 写入版本文件
    write_version_file(target_include_path, &version)?;

    println!("✨ 版本 {} 的头文件已准备就绪。", version);
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path};

    #[tokio::test]
    async fn test_extract_include_ggml() {
        fetch_header(
            "ggml-org",
            "ggml",
            Some("v0.9.7"),
            Path::new("target/ggml"),
        )
        .await
        .unwrap();
    }

     #[tokio::test]
    async fn test_extract_include_whisper() {
        fetch_header(
            "ggml-org",
            "whisper.cpp",
            Some("v1.8.3"),
            Path::new("target/whisper"),
        )
        .await
        .unwrap();
    }
}
