use slab_diffusion_sys::sd_embedding_t;
use std::ffi::CString;

#[derive(Debug, Clone, Default)]
pub struct Embedding {
    pub name: &'static str,
    pub path: &'static str,
    // pub(crate) fp: *const sd_embedding_t,
}

impl From<Embedding> for sd_embedding_t {
    fn from(embedding: Embedding) -> Self {
        let name_c_str = CString::new(embedding.name).unwrap();
        let path_c_str = CString::new(embedding.path).unwrap();
        sd_embedding_t { name: name_c_str.into_raw(), path: path_c_str.into_raw() }
    }
}