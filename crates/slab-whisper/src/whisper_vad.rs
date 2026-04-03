use crate::WhisperError;
use serde::{Deserialize, Serialize};
use slab_whisper_sys::{
    whisper_vad_context, whisper_vad_context_params, whisper_vad_params, whisper_vad_segments,
};
use std::ffi::CString;
use std::os::raw::c_int;
use std::path::Path;

/// Stable Rust-native VAD parameters shared across the runtime chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WhisperVadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_speech_duration_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_silence_duration_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speech_duration_s: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_pad_ms: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples_overlap: Option<f32>,
}

impl WhisperVadParams {
    pub(crate) fn from_native(params: whisper_vad_params) -> Self {
        Self {
            threshold: Some(params.threshold),
            min_speech_duration_ms: Some(params.min_speech_duration_ms),
            min_silence_duration_ms: Some(params.min_silence_duration_ms),
            max_speech_duration_s: Some(params.max_speech_duration_s),
            speech_pad_ms: Some(params.speech_pad_ms),
            samples_overlap: Some(params.samples_overlap),
        }
    }

    pub(crate) fn apply_to(&self, params: &mut whisper_vad_params) {
        if let Some(threshold) = self.threshold {
            params.threshold = threshold;
        }
        if let Some(min_speech_duration_ms) = self.min_speech_duration_ms {
            params.min_speech_duration_ms = min_speech_duration_ms;
        }
        if let Some(min_silence_duration_ms) = self.min_silence_duration_ms {
            params.min_silence_duration_ms = min_silence_duration_ms;
        }
        if let Some(max_speech_duration_s) = self.max_speech_duration_s {
            params.max_speech_duration_s = max_speech_duration_s;
        }
        if let Some(speech_pad_ms) = self.speech_pad_ms {
            params.speech_pad_ms = speech_pad_ms;
        }
        if let Some(samples_overlap) = self.samples_overlap {
            params.samples_overlap = samples_overlap;
        }
    }
}

/// Stable Rust-native standalone VAD context parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WhisperVadContextParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<c_int>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_gpu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_device: Option<c_int>,
}

impl WhisperVadContextParams {
    pub(crate) fn from_native(params: whisper_vad_context_params) -> Self {
        Self {
            n_threads: Some(params.n_threads),
            use_gpu: Some(params.use_gpu),
            gpu_device: Some(params.gpu_device),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InnerWhisperVadContextParams {
    params: whisper_vad_context_params,
}

impl InnerWhisperVadContextParams {
    pub(crate) fn from_canonical(
        lib: &slab_whisper_sys::WhisperLib,
        value: &WhisperVadContextParams,
    ) -> Self {
        let mut params = unsafe { lib.whisper_vad_default_context_params() };

        if let Some(n_threads) = value.n_threads {
            params.n_threads = n_threads;
        }
        if let Some(use_gpu) = value.use_gpu {
            params.use_gpu = use_gpu;
        }
        if let Some(gpu_device) = value.gpu_device {
            params.gpu_device = gpu_device;
        }

        Self { params }
    }

    pub(crate) fn into_inner(self) -> whisper_vad_context_params {
        self.params
    }
}

/// A handle to use `whisper.cpp`'s built in VAD standalone.
///
/// You probably want to use [`Self::segments_from_samples`].
pub struct WhisperVadContext {
    instance: Whisper,
    ptr: *mut whisper_vad_context,
}
unsafe impl Send for WhisperVadContext {}
unsafe impl Sync for WhisperVadContext {}

use crate::Whisper;

impl Whisper {
    pub fn new_vad_context<P: AsRef<Path>>(
        &self,
        model_path: P,
        params: WhisperVadContextParams,
    ) -> Result<WhisperVadContext, WhisperError> {
        let model_path = CString::new(model_path.as_ref().to_string_lossy().as_ref())?;
        let params = InnerWhisperVadContextParams::from_canonical(self.lib.as_ref(), &params);
        let ptr = unsafe {
            self.lib
                .whisper_vad_init_from_file_with_params(model_path.as_ptr(), params.into_inner())
        };

        if ptr.is_null() {
            Err(WhisperError::NullPointer)
        } else {
            Ok(WhisperVadContext { instance: self.clone(), ptr })
        }
    }

    pub fn default_vad_params(&self) -> WhisperVadParams {
        WhisperVadParams::from_native(unsafe { self.lib.whisper_vad_default_params() })
    }

    pub fn default_vad_context_params(&self) -> WhisperVadContextParams {
        WhisperVadContextParams::from_native(unsafe {
            self.lib.whisper_vad_default_context_params()
        })
    }
}

impl WhisperVadContext {
    /// Detect speech in `samples`. Call [`Self::segments_from_probabilities`] to finish the pipeline.
    ///
    /// # Errors
    /// This function will exclusively return `WhisperError::GenericError(-1)` on error.
    /// If you've registered logging hooks, they will have much more detailed information.
    pub fn detect_speech(&mut self, samples: &[f32]) -> Result<(), WhisperError> {
        let (samples, len) = (samples.as_ptr(), samples.len() as c_int);

        let success =
            unsafe { self.instance.lib.whisper_vad_detect_speech(self.ptr, samples, len) };

        if !success { Err(WhisperError::GenericError(-1)) } else { Ok(()) }
    }

    /// Get an array of probabilities. Undocumented use.
    pub fn probabilities(&self) -> &[f32] {
        let prob_ptr = unsafe { self.instance.lib.whisper_vad_probs(self.ptr) };
        let prob_count = unsafe { self.instance.lib.whisper_vad_n_probs(self.ptr) }
            .try_into()
            .expect("n_probs is too large to fit into usize");
        unsafe { core::slice::from_raw_parts(prob_ptr, prob_count) }
    }

    /// Finish running the VAD pipeline and return segment details.
    ///
    /// # Errors
    /// The only possible error is [`WhisperError::NullPointer`].
    pub fn segments_from_probabilities(
        &mut self,
        params: WhisperVadParams,
    ) -> Result<WhisperVadSegments, WhisperError> {
        let mut native = unsafe { self.instance.lib.whisper_vad_default_params() };
        params.apply_to(&mut native);
        let ptr = unsafe { self.instance.lib.whisper_vad_segments_from_probs(self.ptr, native) };

        if ptr.is_null() {
            Err(WhisperError::NullPointer)
        } else {
            Ok(self.instance.new_vad_segments(ptr))
        }
    }

    /// Run the entire VAD pipeline.
    /// This calls both [`Self::detect_speech`] and [`Self::segments_from_probabilities`] behind the scenes.
    ///
    /// # Errors
    /// The only possible error is [`WhisperError::NullPointer`].
    pub fn segments_from_samples(
        &mut self,
        params: WhisperVadParams,
        samples: &[f32],
    ) -> Result<WhisperVadSegments, WhisperError> {
        let (sample_ptr, sample_len) = (samples.as_ptr(), samples.len() as c_int);
        let mut native = unsafe { self.instance.lib.whisper_vad_default_params() };
        params.apply_to(&mut native);
        let ptr = unsafe {
            self.instance
                .lib
                .whisper_vad_segments_from_samples(self.ptr, native, sample_ptr, sample_len)
        };

        if ptr.is_null() {
            Err(WhisperError::NullPointer)
        } else {
            Ok(self.instance.new_vad_segments(ptr))
        }
    }
}

impl Drop for WhisperVadContext {
    fn drop(&mut self) {
        unsafe { self.instance.lib.whisper_vad_free(self.ptr) }
    }
}

/// You can obtain this struct from a [`WhisperVadContext`].
pub struct WhisperVadSegments {
    ptr: *mut whisper_vad_segments,
    segment_count: c_int,
    iter_idx: c_int,
    instance: Whisper,
}

impl Whisper {
    fn new_vad_segments(&self, ptr: *mut whisper_vad_segments) -> WhisperVadSegments {
        let segment_count = unsafe { self.lib.whisper_vad_segments_n_segments(ptr) };
        WhisperVadSegments { ptr, segment_count, iter_idx: 0, instance: self.clone() }
    }
}

impl WhisperVadSegments {
    pub fn num_segments(&self) -> c_int {
        self.segment_count
    }

    pub fn index_in_bounds(&self, idx: c_int) -> bool {
        idx >= 0 && idx < self.segment_count
    }

    /// Return the start timestamp of this segment in centiseconds (10s of milliseconds).
    pub fn get_segment_start_timestamp(&self, idx: c_int) -> Option<f32> {
        if self.index_in_bounds(idx) {
            Some(unsafe { self.instance.lib.whisper_vad_segments_get_segment_t0(self.ptr, idx) })
        } else {
            None
        }
    }

    /// Return the end timestamp of this segment in centiseconds (10s of milliseconds).
    pub fn get_segment_end_timestamp(&self, idx: c_int) -> Option<f32> {
        if self.index_in_bounds(idx) {
            Some(unsafe { self.instance.lib.whisper_vad_segments_get_segment_t1(self.ptr, idx) })
        } else {
            None
        }
    }

    pub fn get_segment(&self, idx: c_int) -> Option<WhisperVadSegment> {
        let start = self.get_segment_start_timestamp(idx)?;
        let end = self.get_segment_end_timestamp(idx)?;

        Some(WhisperVadSegment { start, end })
    }
}

impl Iterator for WhisperVadSegments {
    type Item = WhisperVadSegment;

    fn next(&mut self) -> Option<Self::Item> {
        let segment = self.get_segment(self.iter_idx)?;
        self.iter_idx += 1;
        Some(segment)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhisperVadSegment {
    /// Start timestamp of this segment in centiseconds.
    pub start: f32,
    /// End timestamp of this segment in centiseconds.
    pub end: f32,
}

impl Drop for WhisperVadSegments {
    fn drop(&mut self) {
        unsafe { self.instance.lib.whisper_vad_free_segments(self.ptr) }
    }
}
