use std::ffi::{CStr, CString, c_char};
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

#[cfg(test)]
mod tests {
    use super::*;

    fn alloc_c_string(value: &str) -> *mut c_char {
        let bytes = CString::new(value).unwrap().into_bytes_with_nul();
        let ptr = unsafe { libc::malloc(bytes.len()).cast::<c_char>() };
        assert!(!ptr.is_null());
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), ptr, bytes.len()) };
        ptr
    }

    #[test]
    fn copy_and_free_c_string_returns_owned_string() {
        let text = copy_and_free_c_string(alloc_c_string("slab"));
        assert_eq!(text.as_deref(), Some("slab"));
        assert_eq!(copy_and_free_c_string(ptr::null_mut()), None);
    }

    #[test]
    fn image_helpers_preserve_shape_and_data_pointers() {
        let image = Image { width: 2, height: 3, channel: 4, data: vec![1, 2, 3, 4] };
        let view = image_view(&image);

        assert_eq!(view.width, 2);
        assert_eq!(view.height, 3);
        assert_eq!(view.channel, 4);
        assert_eq!(view.data, image.data.as_ptr().cast_mut());

        let empty = empty_image();
        assert_eq!(empty.width, 0);
        assert!(empty.data.is_null());

        let images = vec![image.clone()];
        let mut views = Vec::new();
        sync_image_views(&images, &mut views);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].width, image.width);
        assert_eq!(views[0].data, images[0].data.as_ptr().cast_mut());
    }

    #[test]
    fn sync_lora_and_embedding_views_keep_strings_alive() {
        let loras = vec![
            Lora { is_high_noise: true, multiplier: 0.5, path: "hi.safetensors" },
            Lora { is_high_noise: false, multiplier: 1.25, path: "lo.safetensors" },
        ];
        let mut lora_paths = Vec::new();
        let mut lora_views = Vec::new();
        sync_lora_views(&loras, &mut lora_paths, &mut lora_views);

        assert_eq!(lora_paths.len(), 2);
        assert_eq!(lora_views.len(), 2);
        assert!(lora_views[0].is_high_noise);
        assert_eq!(lora_views[1].multiplier, 1.25);
        assert_eq!(
            unsafe { CStr::from_ptr(lora_views[0].path) }.to_str().unwrap(),
            "hi.safetensors"
        );

        let embeddings = vec![
            Embedding { name: "style", path: "style.pt" },
            Embedding { name: "face", path: "face.pt" },
        ];
        let mut embedding_names = Vec::new();
        let mut embedding_paths = Vec::new();
        let mut embedding_views = Vec::new();
        sync_embedding_views(
            &embeddings,
            &mut embedding_names,
            &mut embedding_paths,
            &mut embedding_views,
        );

        assert_eq!(embedding_names.len(), 2);
        assert_eq!(embedding_paths.len(), 2);
        assert_eq!(embedding_views.len(), 2);
        assert_eq!(unsafe { CStr::from_ptr(embedding_views[1].name) }.to_str().unwrap(), "face");
        assert_eq!(unsafe { CStr::from_ptr(embedding_views[1].path) }.to_str().unwrap(), "face.pt");
    }
}
