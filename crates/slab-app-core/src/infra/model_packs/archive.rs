use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use base64::Engine as _;
use slab_model_pack::{MANIFEST_FILE_NAME, PACK_EXTENSION};
use slab_utils::fs::atomic_write_bytes;
use slab_utils::hash::sha256_hex_bytes;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::AppCoreError;

use super::ensure_model_pack_dir;

pub(super) fn read_pack_bytes(path: &Path) -> Result<Vec<u8>, AppCoreError> {
    fs::read(path).map_err(|error| {
        AppCoreError::Internal(format!("failed to read model pack '{}': {error}", path.display()))
    })
}

pub(super) fn model_pack_file_name(id: &str) -> String {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes());
    format!("{encoded}.{PACK_EXTENSION}")
}

pub(super) fn write_bytes_file(path: &Path, payload: &[u8]) -> Result<(), AppCoreError> {
    let parent = path.parent().ok_or_else(|| {
        AppCoreError::Internal(format!(
            "model pack path '{}' has no parent directory",
            path.display()
        ))
    })?;
    ensure_model_pack_dir(parent)?;

    atomic_write_bytes(path, payload).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to write model pack file '{}': {error}",
            path.display()
        ))
    })
}

pub(super) fn build_pack_bytes(entries: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, AppCoreError> {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(&mut cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (path, payload) in entries {
        writer.start_file(&path, options).map_err(|error| {
            AppCoreError::Internal(format!("failed to create pack entry '{path}': {error}"))
        })?;
        writer.write_all(&payload).map_err(|error| {
            AppCoreError::Internal(format!("failed to write pack entry '{path}': {error}"))
        })?;
    }

    writer.finish().map_err(|error| {
        AppCoreError::Internal(format!("failed to finalize model pack bytes: {error}"))
    })?;
    Ok(cursor.into_inner())
}

pub(super) fn collect_pack_entries(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::Internal(format!("failed to open model pack archive: {error}"))
    })?;
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to access model pack archive entry {index}: {error}"
            ))
        })?;
        if entry.is_dir() {
            continue;
        }

        let path = entry.name().trim().to_owned();
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to read model pack archive entry '{path}': {error}"
            ))
        })?;
        entries.push((path, payload));
    }

    Ok(entries)
}

pub(super) fn read_pack_entry_bytes(
    bytes: &[u8],
    entry_name: &str,
) -> Result<Option<Vec<u8>>, AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::Internal(format!("failed to open model pack archive: {error}"))
    })?;

    match archive.by_name(entry_name) {
        Ok(mut entry) => {
            let mut payload = Vec::new();
            entry.read_to_end(&mut payload).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to read model pack archive entry '{entry_name}': {error}"
                ))
            })?;
            Ok(Some(payload))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(error) => Err(AppCoreError::Internal(format!(
            "failed to access model pack archive entry '{entry_name}': {error}"
        ))),
    }
}

pub(super) fn manifest_sha256_from_pack_bytes(bytes: &[u8]) -> Result<String, AppCoreError> {
    let manifest_bytes = read_pack_entry_bytes(bytes, MANIFEST_FILE_NAME)?.ok_or_else(|| {
        AppCoreError::BadRequest("missing required manifest.json in .slab archive".into())
    })?;
    Ok(sha256_hex_bytes(&manifest_bytes))
}
