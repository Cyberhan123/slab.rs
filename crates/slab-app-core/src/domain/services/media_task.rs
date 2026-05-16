use std::path::{Path, PathBuf};

use crate::error::AppCoreError;

pub(super) fn parse_json_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()))
}

pub(super) async fn save_rgb_png(
    path: &Path,
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<(), AppCoreError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    let bytes = data.to_vec();
    tokio::task::spawn_blocking(move || {
        image::save_buffer_with_format(
            &path,
            &bytes,
            width,
            height,
            image::ColorType::Rgb8,
            image::ImageFormat::Png,
        )
    })
    .await
    .map_err(|error| AppCoreError::Internal(format!("reference image task panicked: {error}")))?
    .map_err(|error| {
        AppCoreError::Internal(format!("failed to save PNG '{}': {error}", error_path.display()))
    })
}

pub(super) async fn read_managed_file(
    path: &str,
    output_root: &Path,
) -> Result<Vec<u8>, AppCoreError> {
    let candidate = PathBuf::from(path);
    if !candidate.starts_with(output_root) {
        return Err(AppCoreError::BadRequest("artifact path escapes output root".to_owned()));
    }
    tokio::fs::read(&candidate).await.map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            AppCoreError::NotFound(format!("artifact '{}' not found", candidate.display()))
        }
        _ => AppCoreError::Internal(format!(
            "failed to read artifact '{}': {error}",
            candidate.display()
        )),
    })
}

pub(super) async fn cleanup_dir(path: &Path) {
    tokio::fs::remove_dir_all(path).await.ok();
}
