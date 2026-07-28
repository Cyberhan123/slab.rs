use std::ptr::NonNull;
use std::sync::Arc;

use crate::Mtmd;
use crate::SharedMtmdLib;
use crate::context::MtmdContext;
use crate::error::{MtmdError, Result};

/// Owned `mtmd_bitmap` — a decoded image (RGB) or audio (PCM f32) payload ready
/// for [`MtmdContext::tokenize`].
///
/// [`MtmdContext::tokenize`]: crate::MtmdContext::tokenize
pub struct MtmdBitmap {
    ptr: NonNull<slab_mtmd_sys::mtmd_bitmap>,
    lib: Arc<SharedMtmdLib>,
}

// SAFETY: the bitmap pointer is treated as an opaque, immutable handle once
// constructed and is not tied to any thread-local state.
unsafe impl Send for MtmdBitmap {}
unsafe impl Sync for MtmdBitmap {}

impl MtmdBitmap {
    /// Wrap a raw owned bitmap pointer.
    fn from_raw(ptr: *mut slab_mtmd_sys::mtmd_bitmap, lib: Arc<SharedMtmdLib>) -> Result<Self> {
        let ptr = NonNull::new(ptr).ok_or(MtmdError::BitmapCreateFailed)?;
        Ok(Self { ptr, lib })
    }

    /// Create a bitmap from raw, interleaved RGB pixels (`nx * ny * 3` bytes).
    pub fn from_rgb(mtmd: &Mtmd, nx: u32, ny: u32, rgb: &[u8]) -> Result<Self> {
        let expected = (nx as usize)
            .checked_mul(ny as usize)
            .and_then(|n| n.checked_mul(3))
            .ok_or(MtmdError::BitmapCreateFailed)?;
        if rgb.len() < expected {
            return Err(MtmdError::BitmapCreateFailed);
        }
        // SAFETY: `mtmd_bitmap_init` copies `nx*ny*3` bytes from `rgb`; the
        // pointer/length are valid for the duration of the call.
        let ptr = unsafe { mtmd.lib.mtmd_bitmap_init(nx, ny, rgb.as_ptr()) };
        Self::from_raw(ptr, mtmd.lib_arc())
    }

    /// Decode an encoded image buffer (PNG / JPEG / …) using the loaded vision
    /// projector. `placeholder` inserts a zero-embedding stand-in (no decode).
    pub fn from_buf(ctx: &MtmdContext, buf: &[u8], placeholder: bool) -> Result<Self> {
        // SAFETY: `mtmd_helper_bitmap_init_from_buf` reads `len` bytes from
        // `buf`; the returned wrapper owns a freshly-allocated bitmap.
        let wrapper = unsafe {
            ctx.lib_arc().mtmd_helper_bitmap_init_from_buf(
                ctx.as_ptr(),
                buf.as_ptr(),
                buf.len(),
                placeholder,
            )
        };
        // The video ctx is null for still images; only the bitmap field is used.
        Self::from_raw(wrapper.bitmap, ctx.lib_arc())
    }

    pub(crate) fn as_const_ptr(&self) -> *const slab_mtmd_sys::mtmd_bitmap {
        self.ptr.as_ptr()
    }

    pub fn nx(&self) -> u32 {
        // SAFETY: pointer valid until Drop.
        unsafe { self.lib.mtmd_bitmap_get_nx(self.ptr.as_ptr()) }
    }

    pub fn ny(&self) -> u32 {
        unsafe { self.lib.mtmd_bitmap_get_ny(self.ptr.as_ptr()) }
    }

    pub fn n_bytes(&self) -> usize {
        unsafe { self.lib.mtmd_bitmap_get_n_bytes(self.ptr.as_ptr()) }
    }

    pub fn is_audio(&self) -> bool {
        unsafe { self.lib.mtmd_bitmap_is_audio(self.ptr.as_ptr()) }
    }

    /// Attach a string id (used for lazy/multi-image correlation).
    pub fn set_id(&self, id: &str) -> Result<()> {
        let c = std::ffi::CString::new(id)?;
        // SAFETY: `mtmd_bitmap_set_id` copies the string.
        unsafe { self.lib.mtmd_bitmap_set_id(self.ptr.as_ptr(), c.as_ptr()) };
        Ok(())
    }
}

impl Drop for MtmdBitmap {
    fn drop(&mut self) {
        // SAFETY: the bitmap owns its allocation and is dropped once.
        unsafe { self.lib.mtmd_bitmap_free(self.ptr.as_ptr()) };
    }
}
