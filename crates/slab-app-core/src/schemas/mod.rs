//! Shared request / response DTO types for both HTTP API and Tauri IPC.
//!
//! These schemas are used by both `bin/slab-server` (HTTP API) and
//! `bin/slab-app` (Tauri IPC native commands), ensuring a consistent API
//! contract across both entry points.

pub mod agent;
pub mod audio;
pub mod backend;
pub mod chat;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod setup;
pub mod system;
pub mod tasks;
pub mod validation;
pub mod video;

use base64::Engine as _;
use image::GenericImageView;

/// Maximum number of raw bytes accepted in a decoded init-image payload (20 MiB).
pub(crate) const MAX_INIT_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Maximum width or height (in pixels) accepted for an init-image (2 048 px).
pub(crate) const MAX_INIT_IMAGE_DIM: u32 = 2048;

/// Decode a base64 data URI (or raw base64 string) into raw RGB pixels.
///
/// Returns `(raw_rgb_bytes, width, height)`.
///
/// Enforces two guards before touching the image decoder:
/// 1. The decoded byte length must not exceed [`MAX_INIT_IMAGE_BYTES`].
/// 2. Neither image dimension may exceed [`MAX_INIT_IMAGE_DIM`] pixels.
pub(crate) fn decode_base64_init_image(
    data_uri: &str,
) -> Result<(Vec<u8>, u32, u32), crate::error::AppCoreError> {
    let b64 = if let Some(pos) = data_uri.find("base64,") {
        &data_uri[pos + "base64,".len()..]
    } else {
        data_uri
    };

    // Base64 expands ~4/3; add a small constant for padding/whitespace.
    let b64_cap = MAX_INIT_IMAGE_BYTES * 4 / 3 + 8;
    if b64.len() > b64_cap {
        return Err(crate::error::AppCoreError::BadRequest(format!(
            "init_image payload is too large (encoded length {} exceeds limit)",
            b64.len()
        )));
    }

    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|error| {
        crate::error::AppCoreError::BadRequest(format!("init_image base64 decode failed: {error}"))
    })?;

    if bytes.len() > MAX_INIT_IMAGE_BYTES {
        return Err(crate::error::AppCoreError::BadRequest(format!(
            "init_image decoded size ({} bytes) exceeds the maximum of {} bytes",
            bytes.len(),
            MAX_INIT_IMAGE_BYTES
        )));
    }

    let image = image::load_from_memory(&bytes).map_err(|error| {
        crate::error::AppCoreError::BadRequest(format!("init_image decode failed: {error}"))
    })?;

    let (width, height) = image.dimensions();
    if width > MAX_INIT_IMAGE_DIM || height > MAX_INIT_IMAGE_DIM {
        return Err(crate::error::AppCoreError::BadRequest(format!(
            "init_image dimensions ({width}x{height}) exceed the maximum of \
             {MAX_INIT_IMAGE_DIM}x{MAX_INIT_IMAGE_DIM}",
        )));
    }

    let rgb = image.to_rgb8();
    Ok((rgb.into_raw(), width, height))
}
