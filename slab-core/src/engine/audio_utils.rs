//! Shared audio-loading utilities for capability adapter implementations.
//!
//! These helpers provide minimal PCM extraction from common audio formats
//! without requiring an external audio library.  For production workloads
//! with MP3, FLAC, or other compressed formats, apply an FFmpeg pre-process
//! step at the call site.

use crate::base::error::CoreError;

/// Load a WAV file from `path` and return 16 kHz–compatible mono f32 PCM.
///
/// Only uncompressed (PCM) WAV files are supported.  Stereo tracks are
/// averaged to mono.  The returned samples are normalised to `[-1.0, 1.0]`.
///
/// # Errors
///
/// - [`CoreError::EngineIo`] – the file could not be read.
/// - [`CoreError::UnsupportedOperation`] – the file is not a PCM WAV.
pub fn load_pcm_from_wav(path: &str) -> Result<Vec<f32>, CoreError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CoreError::EngineIo(format!("failed to read audio file '{path}': {e}")))?;

    parse_wav_pcm(&bytes).ok_or_else(|| CoreError::UnsupportedOperation {
        backend: "whisper".into(),
        op: format!(
            "audio decoding for '{path}': only uncompressed WAV is supported – \
             pipe through an FFmpeg pre-process stage for other formats"
        ),
    })
}

/// Parse an uncompressed WAV byte buffer and return f32 PCM samples.
///
/// Returns `None` if the data is not a valid PCM WAV.
pub fn parse_wav_pcm(data: &[u8]) -> Option<Vec<f32>> {
    if data.len() < 44 {
        return None;
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }

    let mut pos = 12usize;
    let mut bits_per_sample = 0u16;
    let mut num_channels = 0u16;
    let mut data_start = 0usize;
    let mut data_len = 0usize;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;

        if chunk_id == b"fmt " && chunk_size >= 16 {
            let audio_format = u16::from_le_bytes(data[pos..pos + 2].try_into().ok()?);
            if audio_format != 1 {
                return None; // Not PCM
            }
            num_channels = u16::from_le_bytes(data[pos + 2..pos + 4].try_into().ok()?);
            bits_per_sample = u16::from_le_bytes(data[pos + 14..pos + 16].try_into().ok()?);
        } else if chunk_id == b"data" {
            data_start = pos;
            data_len = chunk_size;
        }

        pos += chunk_size;
    }

    if data_start == 0 || num_channels == 0 || bits_per_sample == 0 {
        return None;
    }

    let raw = &data[data_start..data_start.saturating_add(data_len).min(data.len())];
    let samples: Vec<f32> = match bits_per_sample {
        16 => raw
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect(),
        32 => raw
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
        _ => return None,
    };

    if num_channels == 1 {
        Some(samples)
    } else {
        let ch = num_channels as usize;
        Some(
            samples
                .chunks_exact(ch)
                .map(|frame| frame.iter().sum::<f32>() / ch as f32)
                .collect(),
        )
    }
}
