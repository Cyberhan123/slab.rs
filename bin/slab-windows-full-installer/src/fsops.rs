use std::fs;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::payload::SelectedPayloadManifest;

const PAYLOAD_MANIFEST_FILE_NAME: &str = "payload-manifest.json";

pub fn collect_files_recursive(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_recursive_inner(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(root)
        .with_context(|| format!("failed to read directory {}", root.display()))?;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry under {}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

pub fn normalize_relative_path(path: &Path) -> Result<String> {
    validate_relative_path(path)?;
    let value = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_string_lossy().replace('\\', "/")),
            _ => Err(anyhow!("path '{}' is not a normal relative path", path.display())),
        })
        .collect::<Result<Vec<_>>>()?
        .join("/");

    if value.is_empty() {
        bail!("path '{}' resolved to an empty relative path", path.display());
    }

    Ok(value)
}

pub fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        bail!("path '{}' must be relative", path.display());
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => {
                bail!("path '{}' cannot contain parent segments", path.display());
            }
            Component::Prefix(_) | Component::RootDir => {
                bail!("path '{}' must be relative", path.display());
            }
        }
    }

    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse JSON from {}", path.display()))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent_dir(path)?;
    let bytes = serde_json::to_vec_pretty(value).context("failed to serialize JSON")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    hash_reader(&mut reader)
}

pub fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 64];
    loop {
        let read = reader.read(&mut buffer).context("failed to read stream while hashing")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(bytes_to_hex(&hasher.finalize()))
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!(),
    }
}

pub fn remove_dir_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))?;
    }
    Ok(())
}

pub fn apply_selected_payload(source_root: &Path, dest_root: &Path) -> Result<()> {
    let manifest_path = source_root.join(PAYLOAD_MANIFEST_FILE_NAME);
    let manifest: SelectedPayloadManifest = read_json(&manifest_path)?;

    let dest_parent = dest_root.parent().ok_or_else(|| {
        anyhow!("destination '{}' does not have a parent directory", dest_root.display())
    })?;
    fs::create_dir_all(dest_parent)
        .with_context(|| format!("failed to create directory {}", dest_parent.display()))?;

    let dest_name = dest_root
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("destination '{}' is invalid", dest_root.display()))?;
    let staging_root = dest_parent.join(format!("{dest_name}.staging-{}", Uuid::new_v4()));

    let result = (|| -> Result<()> {
        fs::create_dir_all(&staging_root).with_context(|| {
            format!("failed to create staging directory {}", staging_root.display())
        })?;

        for file in &manifest.files {
            let source_relative = Path::new(&file.source_relative_path);
            let dest_relative = Path::new(&file.dest_relative_path);
            validate_relative_path(source_relative)?;
            validate_relative_path(dest_relative)?;

            let source_path = source_root.join(source_relative);
            let copied_path = staging_root.join(dest_relative);
            ensure_parent_dir(&copied_path)?;

            let source_hash = sha256_file(&source_path)?;
            if source_hash != file.sha256 {
                bail!(
                    "source payload hash mismatch for '{}': expected {}, got {}",
                    source_path.display(),
                    file.sha256,
                    source_hash
                );
            }

            fs::copy(&source_path, &copied_path).with_context(|| {
                format!(
                    "failed to copy payload file {} -> {}",
                    source_path.display(),
                    copied_path.display()
                )
            })?;

            let copied_hash = sha256_file(&copied_path)?;
            if copied_hash != file.sha256 {
                bail!(
                    "copied payload hash mismatch for '{}': expected {}, got {}",
                    copied_path.display(),
                    file.sha256,
                    copied_hash
                );
            }
        }

        remove_dir_if_exists(dest_root)?;
        fs::rename(&staging_root, dest_root).with_context(|| {
            format!(
                "failed to move staged payload {} -> {}",
                staging_root.display(),
                dest_root.display()
            )
        })?;
        Ok(())
    })();

    if result.is_err() {
        let _ = remove_dir_if_exists(&staging_root);
    }

    result
}
