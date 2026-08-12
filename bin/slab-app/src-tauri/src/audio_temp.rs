//! Host-only Tauri commands that bridge the assistant mic recorder to the
//! path-based `/v1/audio/transcriptions` endpoint.
//!
//! The recorder produces an in-memory `Blob`; the transcription endpoint only
//! accepts an absolute filesystem `path`. These commands stage the recorded
//! bytes in the OS temp dir and return the path, then clean up afterwards. They
//! are host-only (never carry product/API traffic) per the AGENTS.md boundary.

use std::path::PathBuf;

use uuid::Uuid;

/// Write recorded audio `bytes` to a uniquely-named temp file with the given
/// extension (e.g. `"webm"`) and return its absolute path. The caller passes
/// the path to `/v1/audio/transcriptions`, then removes it with
/// [`remove_temp_audio`].
#[tauri::command]
pub(crate) fn write_temp_audio(bytes: Vec<u8>, extension: String) -> Result<String, String> {
    let safe_ext = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    let safe_ext = if safe_ext.is_empty() { "webm".to_owned() } else { safe_ext };
    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!("slab-mic-{}.{}", Uuid::new_v4(), safe_ext));
    std::fs::write(&path, &bytes)
        .map_err(|error| format!("failed to write temp audio: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Remove a temp file previously created by [`write_temp_audio`]. Missing files
/// are silently ignored (best-effort cleanup).
#[tauri::command]
pub(crate) fn remove_temp_audio(path: String) -> Result<(), String> {
    let candidate = PathBuf::from(&path);
    // Only delete files that live inside the OS temp dir to avoid ever erasing
    // arbitrary user files via this command.
    let temp_dir = std::env::temp_dir();
    if !candidate.starts_with(&temp_dir) {
        return Err("refusing to remove a file outside the temp directory".to_owned());
    }
    match std::fs::remove_file(&candidate) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to remove temp audio: {error}")),
    }
}
