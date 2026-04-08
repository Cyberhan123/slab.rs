//! Shared audio-loading utilities for capability adapter implementations.
//!
//! These helpers provide minimal PCM extraction from uncompressed WAV files
//! without requiring an external audio library.  For production workloads
//! with MP3, FLAC, or other compressed formats, apply an FFmpeg pre-process
//! step at the call site.

use crate::base::error::CoreError;

/// Load a WAV file from `path` and return mono f32 PCM samples at the file's
/// native sample rate.
///
/// The file must be an uncompressed (PCM) WAV.  Stereo and multi-channel
/// tracks are averaged to mono.  16-bit samples are normalised to
/// `[-1.0, 1.0]`; 32-bit float samples are returned as-is.
///
/// **Note**: this function does **not** resample.  If the Whisper backend
/// requires 16 kHz input and the file's sample rate differs, the caller must
/// resample the returned buffer or pre-process the file with FFmpeg before
/// calling this function.
///
/// # Errors
///
/// - [`CoreError::EngineIo`] – the file could not be read.
/// - [`CoreError::UnsupportedOperation`] – the file is not a valid PCM WAV.
pub fn load_pcm_from_wav(path: &str) -> Result<Vec<f32>, CoreError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CoreError::EngineIo(format!("failed to read audio file '{path}': {e}")))?;

    parse_wav_pcm(&bytes).ok_or_else(|| CoreError::UnsupportedOperation {
        backend: "whisper".into(),
        op: format!(
            "audio decoding for '{path}': only uncompressed PCM WAV is supported – \
             pipe through an FFmpeg pre-process stage for other formats"
        ),
    })
}

/// Parse an uncompressed WAV byte buffer and return f32 PCM samples.
///
/// Returns `None` if the data is not a valid uncompressed PCM WAV or if
/// parsing encounters a truncated/malformed chunk.
pub fn parse_wav_pcm(data: &[u8]) -> Option<Vec<f32>> {
    // Minimum valid WAV: 12-byte RIFF header + 8-byte chunk header.
    if data.len() < 20 {
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
        // Bounds-check the chunk size field itself.
        let chunk_size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        pos += 8;

        // Bounds-check the chunk body before slicing into it.
        if pos.checked_add(chunk_size)? > data.len() {
            // Truncated chunk – stop rather than panic.
            break;
        }

        if chunk_id == b"fmt " && chunk_size >= 16 {
            // fmt chunk layout (PCM):
            //   0-1  audio_format (1 = PCM)
            //   2-3  num_channels
            //   4-7  sample_rate
            //   8-11 byte_rate
            //   12-13 block_align
            //   14-15 bits_per_sample
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

        // WAV chunks are word-aligned; skip the padding byte when chunk_size is odd.
        let padded = chunk_size + (chunk_size & 1);
        pos = pos.checked_add(padded)?;
    }

    if data_start == 0 || num_channels == 0 || bits_per_sample == 0 {
        return None;
    }

    let raw = &data[data_start..data_start.saturating_add(data_len).min(data.len())];
    let samples: Vec<f32> = match bits_per_sample {
        16 => {
            raw.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect()
        }
        32 => raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect(),
        _ => return None,
    };

    if num_channels == 1 {
        Some(samples)
    } else {
        let ch = num_channels as usize;
        Some(samples.chunks_exact(ch).map(|frame| frame.iter().sum::<f32>() / ch as f32).collect())
    }
}
