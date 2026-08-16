use std::{borrow::Cow, path::Path};

use deno_core::{ModuleSpecifier, v8};

use crate::Error;

/// Converts a string representing a relative or absolute path into a
/// `ModuleSpecifier`. A relative path is considered relative to the passed
/// `current_dir`.
///
/// This is a patch for the str only `deno_core` provided version
fn resolve_path(
    path_str: impl AsRef<Path>,
    current_dir: &Path,
) -> Result<ModuleSpecifier, deno_core::ModuleResolutionError> {
    let path = current_dir.join(path_str);
    let path = deno_core::normalize_path(Cow::Owned(path));
    deno_core::url::Url::from_file_path(&path).map_err(|()| {
        deno_core::ModuleResolutionError::InvalidUrl(
            deno_core::url::ParseError::RelativeUrlWithoutBase,
        )
    })
}

pub trait ToModuleSpecifier {
    fn to_module_specifier(&self, base: &Path) -> Result<ModuleSpecifier, Error>;
}

impl<T: AsRef<Path>> ToModuleSpecifier for T {
    fn to_module_specifier(&self, base: &Path) -> Result<ModuleSpecifier, Error> {
        Ok(resolve_path(self, base)?)
    }
}

pub trait ToDefinedValue<T> {
    fn if_defined(&self) -> Option<T>;
}

impl<'a> ToDefinedValue<v8::Local<'a, v8::Value>> for Option<v8::Local<'a, v8::Value>> {
    fn if_defined(&self) -> Option<v8::Local<'a, v8::Value>> {
        self.filter(|v| !v.is_undefined())
    }
}
