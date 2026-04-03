use libloading::Library;
use std::ffi::{OsStr, c_char};

use crate::{ggml_backend_dev_t, ggml_backend_dev_type, ggml_backend_reg_t, ggml_backend_t};

pub struct GGmlLoaderLib {
    _lib: Library,
    ggml_backend_dev_by_name:
        Result<unsafe extern "C" fn(*const c_char) -> ggml_backend_dev_t, ::libloading::Error>,
    ggml_backend_dev_by_type: Result<
        unsafe extern "C" fn(ggml_backend_dev_type) -> ggml_backend_dev_t,
        ::libloading::Error,
    >,
    ggml_backend_dev_count: Result<unsafe extern "C" fn() -> usize, ::libloading::Error>,
    ggml_backend_dev_get:
        Result<unsafe extern "C" fn(usize) -> ggml_backend_dev_t, ::libloading::Error>,
    ggml_backend_device_register:
        Result<unsafe extern "C" fn(ggml_backend_dev_t), ::libloading::Error>,
    ggml_backend_init_best: Result<unsafe extern "C" fn() -> ggml_backend_t, ::libloading::Error>,
    ggml_backend_init_by_name: Result<
        unsafe extern "C" fn(*const c_char, *const c_char) -> ggml_backend_t,
        ::libloading::Error,
    >,
    ggml_backend_init_by_type: Result<
        unsafe extern "C" fn(ggml_backend_dev_type, *const c_char) -> ggml_backend_t,
        ::libloading::Error,
    >,
    ggml_backend_load:
        Result<unsafe extern "C" fn(*const c_char) -> ggml_backend_reg_t, ::libloading::Error>,
    ggml_backend_load_all: Result<unsafe extern "C" fn(), ::libloading::Error>,
    ggml_backend_load_all_from_path:
        Result<unsafe extern "C" fn(*const c_char), ::libloading::Error>,
    ggml_backend_reg_by_name:
        Result<unsafe extern "C" fn(*const c_char) -> ggml_backend_reg_t, ::libloading::Error>,
    ggml_backend_reg_count: Result<unsafe extern "C" fn() -> usize, ::libloading::Error>,
    ggml_backend_reg_get:
        Result<unsafe extern "C" fn(usize) -> ggml_backend_reg_t, ::libloading::Error>,
    ggml_backend_register: Result<unsafe extern "C" fn(ggml_backend_reg_t), ::libloading::Error>,
    ggml_backend_unload: Result<unsafe extern "C" fn(ggml_backend_reg_t), ::libloading::Error>,
}

impl GGmlLoaderLib {
    pub unsafe fn new<P: AsRef<OsStr>>(path: P) -> Result<Self, libloading::Error> {
        let library = ::libloading::Library::new(path)?;
        Self::from_library(library)
    }

    pub unsafe fn from_library<L>(library: L) -> Result<Self, ::libloading::Error>
    where
        L: Into<::libloading::Library>,
    {
        let __library = library.into();

        Ok(Self {
            ggml_backend_dev_by_name: unsafe {
                __library.get(b"ggml_backend_dev_by_name\0").map(|sym| *sym)
            },
            ggml_backend_dev_by_type: unsafe {
                __library.get(b"ggml_backend_dev_by_type\0").map(|sym| *sym)
            },
            ggml_backend_dev_count: unsafe {
                __library.get(b"ggml_backend_dev_count\0").map(|sym| *sym)
            },
            ggml_backend_dev_get: unsafe {
                __library.get(b"ggml_backend_dev_get\0").map(|sym| *sym)
            },
            ggml_backend_device_register: unsafe {
                __library.get(b"ggml_backend_device_register\0").map(|sym| *sym)
            },
            ggml_backend_init_best: unsafe {
                __library.get(b"ggml_backend_init_best\0").map(|sym| *sym)
            },
            ggml_backend_init_by_name: unsafe {
                __library.get(b"ggml_backend_init_by_name\0").map(|sym| *sym)
            },
            ggml_backend_init_by_type: unsafe {
                __library.get(b"ggml_backend_init_by_type\0").map(|sym| *sym)
            },
            ggml_backend_load: unsafe { __library.get(b"ggml_backend_load\0").map(|sym| *sym) },
            ggml_backend_load_all: unsafe {
                __library.get(b"ggml_backend_load_all\0").map(|sym| *sym)
            },
            ggml_backend_load_all_from_path: unsafe {
                __library.get(b"ggml_backend_load_all_from_path\0").map(|sym| *sym)
            },
            ggml_backend_reg_by_name: unsafe {
                __library.get(b"ggml_backend_reg_by_name\0").map(|sym| *sym)
            },
            ggml_backend_reg_count: unsafe {
                __library.get(b"ggml_backend_reg_count\0").map(|sym| *sym)
            },
            ggml_backend_reg_get: unsafe {
                __library.get(b"ggml_backend_reg_get\0").map(|sym| *sym)
            },
            ggml_backend_register: unsafe {
                __library.get(b"ggml_backend_register\0").map(|sym| *sym)
            },
            ggml_backend_unload: unsafe { __library.get(b"ggml_backend_unload\0").map(|sym| *sym) },
            _lib: __library,
        })
    }

    pub unsafe fn ggml_backend_dev_by_name(&self, name: *const c_char) -> ggml_backend_dev_t {
        (self.ggml_backend_dev_by_name.as_ref().expect("Expected function, got error."))(name)
    }

    pub unsafe fn ggml_backend_dev_by_type(
        &self,
        dev_type: ggml_backend_dev_type,
    ) -> ggml_backend_dev_t {
        (self.ggml_backend_dev_by_type.as_ref().expect("Expected function, got error."))(dev_type)
    }

    pub unsafe fn ggml_backend_dev_count(&self) -> usize {
        (self.ggml_backend_dev_count.as_ref().expect("Expected function, got error."))()
    }

    pub unsafe fn ggml_backend_dev_get(&self, index: usize) -> ggml_backend_dev_t {
        (self.ggml_backend_dev_get.as_ref().expect("Expected function, got error."))(index)
    }

    pub unsafe fn ggml_backend_device_register(&self, dev: ggml_backend_dev_t) {
        (self.ggml_backend_device_register.as_ref().expect("Expected function, got error."))(dev)
    }

    pub unsafe fn ggml_backend_init_best(&self) -> ggml_backend_t {
        (self.ggml_backend_init_best.as_ref().expect("Expected function, got error."))()
    }

    pub unsafe fn ggml_backend_init_by_name(
        &self,
        name: *const c_char,
        dev_name: *const c_char,
    ) -> ggml_backend_t {
        (self.ggml_backend_init_by_name.as_ref().expect("Expected function, got error."))(
            name, dev_name,
        )
    }

    pub unsafe fn ggml_backend_init_by_type(
        &self,
        backend_type: ggml_backend_dev_type,
        dev_name: *const c_char,
    ) -> ggml_backend_t {
        (self.ggml_backend_init_by_type.as_ref().expect("Expected function, got error."))(
            backend_type,
            dev_name,
        )
    }

    pub unsafe fn ggml_backend_load(&self, path: *const c_char) -> ggml_backend_reg_t {
        (self.ggml_backend_load.as_ref().expect("Expected function, got error."))(path)
    }

    pub unsafe fn ggml_backend_load_all(&self) {
        (self.ggml_backend_load_all.as_ref().expect("Expected function, got error."))()
    }

    pub unsafe fn ggml_backend_load_all_from_path(&self, path: *const c_char) {
        (self.ggml_backend_load_all_from_path.as_ref().expect("Expected function, got error."))(
            path,
        )
    }

    pub unsafe fn ggml_backend_reg_by_name(&self, name: *const c_char) -> ggml_backend_reg_t {
        (self.ggml_backend_reg_by_name.as_ref().expect("Expected function, got error."))(name)
    }

    pub unsafe fn ggml_backend_reg_count(&self) -> usize {
        (self.ggml_backend_reg_count.as_ref().expect("Expected function, got error."))()
    }

    pub unsafe fn ggml_backend_reg_get(&self, index: usize) -> ggml_backend_reg_t {
        (self.ggml_backend_reg_get.as_ref().expect("Expected function, got error."))(index)
    }

    pub unsafe fn ggml_backend_register(&self, reg: ggml_backend_reg_t) {
        (self.ggml_backend_register.as_ref().expect("Expected function, got error."))(reg)
    }

    pub unsafe fn ggml_backend_unload(&self, reg: ggml_backend_reg_t) {
        (self.ggml_backend_unload.as_ref().expect("Expected function, got error."))(reg)
    }
}
