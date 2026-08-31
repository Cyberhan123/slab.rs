use crate::WhisperError;

/// Convert an array of 16 bit mono audio samples to a vector of 32 bit floats.
///
/// # Arguments
/// * `samples` - The array of 16 bit mono audio samples.
/// * `output` - The vector of 32 bit floats to write the converted samples to.
///
/// # Panics
/// * if `samples.len != output.len()`
///
/// # Examples
/// ```
/// # use slab_whisper::convert_integer_to_float_audio;
/// let samples = [0i16; 1024];
/// let mut output = vec![0.0f32; samples.len()];
/// convert_integer_to_float_audio(&samples, &mut output).expect("input and output lengths should be equal");
/// ```
pub fn convert_integer_to_float_audio(
    samples: &[i16],
    output: &mut [f32],
) -> Result<(), WhisperError> {
    if samples.len() != output.len() {
        return Err(WhisperError::InputOutputLengthMismatch {
            input_len: samples.len(),
            output_len: output.len(),
        });
    }

    for (input, output) in samples.iter().zip(output.iter_mut()) {
        *output = *input as f32 / 32768.0;
    }

    Ok(())
}

/// Convert 32-bit floating point stereo PCM audio to 32-bit floating point mono PCM audio.
///
/// # Arguments
/// * `input` - The array of 32-bit floating point stereo PCM audio samples.
/// * `output` - An output place to write all the mono samples.
///
/// # Errors
/// * if `samples.len()` is odd ([`WhisperError::HalfSampleMissing`])
/// * if `input.len() / 2 < samples.len()` ([`WhisperError::InputOutputLengthMismatch`])
///
/// # Returns
/// A vector of 32-bit floating point mono PCM audio samples.
///
/// # Examples
/// ```
/// # use slab_whisper::convert_stereo_to_mono_audio;
/// let samples = [0.0f32; 1024];
/// let mut mono_samples = [0.0f32; 512];
/// convert_stereo_to_mono_audio(&samples, &mut mono_samples).expect("should be no half samples missing");
/// ```
pub fn convert_stereo_to_mono_audio(input: &[f32], output: &mut [f32]) -> Result<(), WhisperError> {
    let (input, []) = input.as_chunks::<2>() else {
        // we only hit this branch if the second binding was not empty
        // or in other words, if input.len() % 2 != 0
        return Err(WhisperError::HalfSampleMissing(input.len()));
    };
    if output.len() != input.len() {
        return Err(WhisperError::InputOutputLengthMismatch {
            input_len: input.len(),
            output_len: output.len(),
        });
    }

    for ([left, right], output) in input.iter().zip(output) {
        *output = (left + right) / 2.0;
    }

    Ok(())
}
