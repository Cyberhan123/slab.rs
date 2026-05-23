//! Virtual filesystem for embedding Python source modules statically.
//!
//! `EmbeddedStdlib` holds a collection of fully-qualified Python module names
//! mapped to their source bytes (bundled into the binary via `include_bytes!`
//! or inserted at runtime).
//!
//! At interpreter startup, [`register`] injects an `_EmbeddedFinder` class
//! into `sys.meta_path` (at position 0) so the embedded modules take
//! precedence over the real filesystem. This is the mechanism that enables
//! static embedding: no `.py` files need to exist on disk at runtime.
//!
//! # Python-side design
//!
//! The finder and loader are implemented as pure Python classes and evaluated
//! once per interpreter via `Python::run_bound`. The Rust side only provides
//! the module-name → bytes mapping through a `PyDict`.

use std::collections::HashMap;
use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

/// A static map of fully-qualified Python module names to their source bytes.
///
/// Populate this before calling [`register`] with every `.py` file that
/// should be loadable without touching the real filesystem.
///
/// # Example
///
/// ```rust,ignore
/// let mut stdlib = EmbeddedStdlib::default();
/// stdlib.add("mypackage.utils", include_bytes!("../python/mypackage/utils.py"));
/// ```
#[derive(Default)]
pub struct EmbeddedStdlib {
    modules: HashMap<&'static str, &'static [u8]>,
}

impl EmbeddedStdlib {
    /// Register a module by its fully-qualified name and its source bytes.
    pub fn add(&mut self, name: &'static str, source: &'static [u8]) -> &mut Self {
        self.modules.insert(name, source);
        self
    }
}

/// Register the embedded stdlib as the first entry in `sys.meta_path`.
///
/// Must be called while holding the GIL, before any user code imports modules.
pub fn register(py: Python<'_>, stdlib: &EmbeddedStdlib) -> PyResult<()> {
    // Build a Python dict: str -> bytes
    let py_modules = PyDict::new(py);
    for (name, src) in &stdlib.modules {
        py_modules.set_item(*name, PyBytes::new(py, src))?;
    }

    // The finder and loader are written in Python for correctness and
    // simplicity. The Rust side only supplies the data.
    let setup = r#"
import sys
import importlib.abc
import importlib.machinery

class _EmbeddedLoader(importlib.abc.Loader):
    """Loads a Python module from statically embedded source bytes."""

    def __init__(self, fullname, source_bytes):
        self._fullname = fullname
        self._source = source_bytes

    def create_module(self, spec):
        return None  # use default module creation

    def exec_module(self, module):
        origin = '<embedded:{}>'.format(self._fullname)
        code = compile(self._source.decode('utf-8'), origin, 'exec')
        exec(code, module.__dict__)

    def get_source(self, fullname):
        return self._source.decode('utf-8')

    def is_package(self, fullname):
        return False


class _EmbeddedFinder(importlib.abc.MetaPathFinder):
    """A sys.meta_path finder that resolves modules from embedded bytes."""

    def __init__(self, modules):
        # modules: dict[str, bytes]
        self._modules = modules

    def find_spec(self, fullname, path, target=None):
        src = self._modules.get(fullname)
        if src is None:
            return None
        loader = _EmbeddedLoader(fullname, src)
        return importlib.machinery.ModuleSpec(
            fullname,
            loader,
            origin='<embedded:{}>'.format(fullname),
        )

    def invalidate_caches(self):
        pass


# Insert before all other finders so embedded modules take priority.
sys.meta_path.insert(0, _EmbeddedFinder(_slab_embedded_modules))
del _EmbeddedFinder, _EmbeddedLoader
"#;

    let globals = PyDict::new(py);
    globals.set_item("_slab_embedded_modules", py_modules)?;
    let code = CString::new(setup).expect("setup code contains no null bytes");
    py.run(&code, Some(&globals), None)?;
    Ok(())
}
