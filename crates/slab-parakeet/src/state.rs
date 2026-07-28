use crate::context::ParakeetInnerContext;
use crate::full_params::InnerFullParams;
use crate::{FullParams, Parakeet, ParakeetError};
use std::borrow::Cow;
use std::ffi::{CStr, c_int};
use std::fmt;
use std::sync::Arc;

/// Rustified pointer to a parakeet state.
#[derive(Debug)]
pub struct ParakeetState {
    pub(crate) ctx: Arc<ParakeetInnerContext>,
    pub(crate) ptr: *mut slab_parakeet_sys::parakeet_state,
}

// SAFETY: The state pointer is only accessed through `&self`/`&mut self` methods.
// The parakeet library does not use thread-local state for state operations.
unsafe impl Send for ParakeetState {}
// SAFETY: Same as Send - all mutable access is exclusive through Rust's borrowing rules.
unsafe impl Sync for ParakeetState {}

impl Drop for ParakeetState {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                self.ctx.instance.lib.parakeet_free_state(self.ptr);
            }
        }
    }
}

impl ParakeetState {
    /// # Safety
    /// * `ptr` must be non-null
    /// * `ptr` must be a valid pointer to a `parakeet_state`.
    pub(crate) unsafe fn new(
        ctx: Arc<ParakeetInnerContext>,
        ptr: *mut slab_parakeet_sys::parakeet_state,
    ) -> Self {
        Self { ctx, ptr }
    }

    /// Run the entire model: PCM -> log mel spectrogram -> encoder -> decoder -> text
    /// using greedy sampling.
    ///
    /// # Arguments
    /// * `params`: [`crate::FullParams`] struct.
    /// * `data`: raw PCM audio data, 32 bit floating point at a sample rate of 16 kHz,
    ///   1 channel.
    ///
    /// # C++ equivalent
    /// `int parakeet_full_with_state(ctx, state, params, samples, n_samples)`
    pub fn full(&mut self, params: FullParams, data: &[f32]) -> Result<(), ParakeetError> {
        if data.is_empty() {
            // can randomly trigger segmentation faults if we don't check this
            return Err(ParakeetError::NoSamples);
        }

        let params = InnerFullParams::from_canonical(self.ctx.instance.lib.as_ref(), &params)?;
        let ret = unsafe {
            self.ctx.instance.lib.parakeet_full_with_state(
                self.ctx.ctx,
                self.ptr,
                params.fp,
                data.as_ptr(),
                data.len() as c_int,
            )
        };
        if ret == -1 {
            Err(ParakeetError::UnableToCalculateSpectrogram)
        } else if ret == 7 {
            Err(ParakeetError::FailedToEncode)
        } else if ret == 8 {
            Err(ParakeetError::FailedToDecode)
        } else if ret == 0 {
            Ok(())
        } else {
            Err(ParakeetError::GenericError(ret))
        }
    }

    /// Number of generated text segments.
    ///
    /// # C++ equivalent
    /// `int parakeet_full_n_segments_from_state(struct parakeet_state * state)`
    pub fn full_n_segments(&self) -> c_int {
        unsafe { self.ctx.instance.lib.parakeet_full_n_segments_from_state(self.ptr) }
    }

    fn segment_in_bounds(&self, segment: c_int) -> bool {
        segment >= 0 && segment < self.full_n_segments()
    }

    /// Get a [`ParakeetSegment`] object for the specified segment index.
    ///
    /// # Returns
    /// `Some(ParakeetSegment)` if `segment` is in bounds, otherwise [`None`].
    pub fn get_segment(&self, segment: c_int) -> Option<ParakeetSegment<'_>> {
        self.segment_in_bounds(segment)
            // SAFETY: we've just asserted that this segment is in bounds
            .then(|| ParakeetSegment::new_unchecked(self, segment))
    }

    /// Get an iterator over all segments.
    pub fn as_iter(&self) -> ParakeetSegmentIterator<'_> {
        ParakeetSegmentIterator::new(self)
    }
}

/// A segment returned by parakeet after running the transcription pipeline.
pub struct ParakeetSegment<'a> {
    state: &'a ParakeetState,
    segment_idx: c_int,
    instance: Parakeet,
}

impl<'a> ParakeetSegment<'a> {
    /// # Safety
    /// You must ensure `segment_idx` is in bounds for the linked [`ParakeetState`].
    pub(super) fn new_unchecked(state: &'a ParakeetState, segment_idx: c_int) -> Self {
        debug_assert!(
            state.segment_in_bounds(segment_idx),
            "tried to create a ParakeetSegment out of bounds for linked state"
        );
        ParakeetSegment { state, segment_idx, instance: state.ctx.instance.clone() }
    }

    /// Get the index of this segment.
    pub fn segment_index(&self) -> c_int {
        self.segment_idx
    }

    /// Get the start time of the specified segment (centiseconds).
    ///
    /// # C++ equivalent
    /// `int64_t parakeet_full_get_segment_t0_from_state(state, i_segment)`
    pub fn start_timestamp(&self) -> i64 {
        unsafe {
            self.instance
                .lib
                .parakeet_full_get_segment_t0_from_state(self.state.ptr, self.segment_idx)
        }
    }

    /// Get the end time of the specified segment (centiseconds).
    ///
    /// # C++ equivalent
    /// `int64_t parakeet_full_get_segment_t1_from_state(state, i_segment)`
    pub fn end_timestamp(&self) -> i64 {
        unsafe {
            self.instance
                .lib
                .parakeet_full_get_segment_t1_from_state(self.state.ptr, self.segment_idx)
        }
    }

    fn to_raw_cstr(&self) -> Result<&'a CStr, ParakeetError> {
        let ret = unsafe {
            self.instance
                .lib
                .parakeet_full_get_segment_text_from_state(self.state.ptr, self.segment_idx)
        };
        if ret.is_null() {
            return Err(ParakeetError::NullPointer);
        }
        Ok(unsafe { CStr::from_ptr(ret) })
    }

    /// Get the raw bytes of this segment.
    pub fn to_bytes(&self) -> Result<&'a [u8], ParakeetError> {
        Ok(self.to_raw_cstr()?.to_bytes())
    }

    /// Get the text of this segment (UTF-8 validated).
    pub fn to_str(&self) -> Result<&'a str, ParakeetError> {
        Ok(self.to_raw_cstr()?.to_str()?)
    }

    /// Get the text of this segment, replacing invalid UTF-8 with the replacement char.
    pub fn to_str_lossy(&self) -> Result<Cow<'a, str>, ParakeetError> {
        Ok(self.to_raw_cstr()?.to_string_lossy())
    }
}

impl fmt::Display for ParakeetSegment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str_lossy().expect("got null pointer during string write"))
    }
}

impl fmt::Debug for ParakeetSegment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParakeetSegment")
            .field("segment", &self.segment_idx)
            .field("start_ts", &self.start_timestamp())
            .field("end_ts", &self.end_timestamp())
            .field("text", &self.to_str_lossy())
            .finish_non_exhaustive()
    }
}

/// Iterator over the segments of a [`ParakeetState`].
pub struct ParakeetSegmentIterator<'a> {
    state: &'a ParakeetState,
    index: c_int,
    len: c_int,
}

impl<'a> ParakeetSegmentIterator<'a> {
    pub(crate) fn new(state: &'a ParakeetState) -> Self {
        Self { state, index: 0, len: state.full_n_segments() }
    }
}

impl<'a> Iterator for ParakeetSegmentIterator<'a> {
    type Item = ParakeetSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let idx = self.index;
        self.index += 1;
        // SAFETY: idx is checked against the snapshot length taken at construction.
        Some(ParakeetSegment::new_unchecked(self.state, idx))
    }
}
