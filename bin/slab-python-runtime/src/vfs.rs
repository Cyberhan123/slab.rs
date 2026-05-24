//! Virtual filesystem for statically embedded Python source modules.
//!
//! `EmbeddedStdlib` maps fully-qualified Python module names to source bytes
//! bundled with `include_bytes!`. At interpreter startup, [`register`] injects
//! a `sys.meta_path` finder so embedded modules and packages can load before
//! the real filesystem.

use std::collections::HashMap;
use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

#[derive(Clone, Copy)]
struct EmbeddedModule {
    source: &'static [u8],
    is_package: bool,
}

/// A static map of fully-qualified Python module names to source bytes.
#[derive(Clone, Default)]
pub struct EmbeddedStdlib {
    modules: HashMap<&'static str, EmbeddedModule>,
}

impl EmbeddedStdlib {
    /// Register a non-package module by its fully-qualified name.
    pub fn add(&mut self, name: &'static str, source: &'static [u8]) -> &mut Self {
        self.add_module(name, source)
    }

    /// Register a non-package module by its fully-qualified name.
    pub fn add_module(&mut self, name: &'static str, source: &'static [u8]) -> &mut Self {
        self.modules.insert(name, EmbeddedModule { source, is_package: false });
        self
    }

    /// Register a package `__init__.py` by its fully-qualified package name.
    pub fn add_package(&mut self, name: &'static str, source: &'static [u8]) -> &mut Self {
        self.modules.insert(name, EmbeddedModule { source, is_package: true });
        self
    }
}

/// Register the embedded stdlib as the first entry in `sys.meta_path`.
pub fn register(py: Python<'_>, stdlib: &EmbeddedStdlib) -> PyResult<()> {
    let py_modules = PyDict::new(py);
    for (name, module) in &stdlib.modules {
        py_modules.set_item(*name, (PyBytes::new(py, module.source), module.is_package))?;
    }

    let setup = r#"import sys
import importlib.abc
import importlib.machinery

class _EmbeddedLoader(importlib.abc.Loader):
    """Loads a Python module or package from statically embedded source bytes."""

    def __init__(self, fullname, source_bytes, is_package):
        self._fullname = fullname
        self._source = source_bytes
        self._is_package = is_package

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        origin = '<embedded:{}>'.format(self._fullname)
        module.__file__ = origin
        module.__loader__ = self
        if self._is_package:
            module.__package__ = self._fullname
            module.__path__ = []
        else:
            module.__package__ = self._fullname.rpartition('.')[0]
        code = compile(self._source.decode('utf-8'), origin, 'exec')
        exec(code, module.__dict__)

    def get_source(self, fullname):
        return self._source.decode('utf-8')

    def is_package(self, fullname):
        return self._is_package


class _EmbeddedFinder(importlib.abc.MetaPathFinder):
    """A sys.meta_path finder that resolves modules from embedded bytes."""

    def __init__(self, modules):
        self._modules = modules

    def find_spec(self, fullname, path, target=None):
        entry = self._modules.get(fullname)
        if entry is None:
            return None
        src, is_package = entry
        loader = _EmbeddedLoader(fullname, src, is_package)
        spec = importlib.machinery.ModuleSpec(
            fullname,
            loader,
            origin='<embedded:{}>'.format(fullname),
            is_package=is_package,
        )
        if is_package:
            spec.submodule_search_locations = []
        return spec

    def invalidate_caches(self):
        pass


_slab_finder = next(
    (finder for finder in sys.meta_path if getattr(finder, '_slab_embedded_finder', False)),
    None,
)
if _slab_finder is None:
    _slab_finder = _EmbeddedFinder(_slab_embedded_modules)
    _slab_finder._slab_embedded_finder = True
    sys.meta_path.insert(0, _slab_finder)
else:
    _slab_finder._modules = _slab_embedded_modules
del _slab_finder
"#;

    let globals = PyDict::new(py);
    globals.set_item("_slab_embedded_modules", py_modules)?;
    let code = CString::new(setup).expect("setup code contains no null bytes");
    py.run(&code, Some(&globals), None)?;
    Ok(())
}
