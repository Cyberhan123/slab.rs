use std::ffi::{c_char, CStr, CString};
use std::ptr;

use libc::free;
use slab_diffusion_sys::{sd_embedding_t, sd_image_t, sd_lora_t};

use super::{Embedding, Image, Lora};

pub(crate) fn c_string_ptr(value: &CString) -> *const c_char {
    value.as_c_str().as_ptr()
}

pub(crate) fn copy_and_free_c_string(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    unsafe { free(ptr.cast()) };
    Some(text)
}

pub(crate) fn new_c_string(value: &str) -> CString {
    CString::new(value).expect("string contains an interior NUL byte")
}

pub(crate) fn image_view(image: &Image) -> sd_image_t {
    sd_image_t {
        width: image.width,
        height: image.height,
        channel: image.channel,
        data: if image.data.is_empty() { ptr::null_mut() } else { image.data.as_ptr().cast_mut() },
    }
}

pub(crate) fn empty_image() -> sd_image_t {
    sd_image_t { width: 0, height: 0, channel: 0, data: ptr::null_mut() }
}

pub(crate) fn sync_image_views(images: &[Image], views: &mut Vec<sd_image_t>) {
    views.clear();
    views.extend(images.iter().map(image_view));
}

pub(crate) fn sync_lora_views(
    loras: &[Lora],
    paths: &mut Vec<CString>,
    views: &mut Vec<sd_lora_t>,
) {
    paths.clear();
    views.clear();

    for lora in loras {
        let path = new_c_string(lora.path);
        views.push(sd_lora_t {
            is_high_noise: lora.is_high_noise,
            multiplier: lora.multiplier,
            path: c_string_ptr(&path),
        });
        paths.push(path);
    }
}

pub(crate) fn sync_embedding_views(
    embeddings: &[Embedding],
    names: &mut Vec<CString>,
    paths: &mut Vec<CString>,
    views: &mut Vec<sd_embedding_t>,
) {
    names.clear();
    paths.clear();
    views.clear();

    for embedding in embeddings {
        let name = new_c_string(embedding.name);
        let path = new_c_string(embedding.path);

        views.push(sd_embedding_t { name: c_string_ptr(&name), path: c_string_ptr(&path) });
        names.push(name);
        paths.push(path);
    }
}
