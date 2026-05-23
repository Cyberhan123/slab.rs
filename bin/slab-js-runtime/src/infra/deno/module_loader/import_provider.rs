use deno_core::{error::ModuleLoaderError, ModuleSource, ModuleSpecifier, RequestedModuleType};

/// A trait that can be implemented to modify the behavior of the module loader
/// Allows for custom schemes, caching, and more granular permissions
#[allow(unused_variables)]
pub trait ImportProvider {
    /// Resolve an import statement's specifier to a URL to later be imported
    /// This can be used to modify the URL, or to disallow certain imports
    ///
    /// The default behavior is to return None, which will fall back to the standard resolution behavior
    ///
    /// # Arguments
    /// - `specifier`: The module specifier to resolve, as an absolute URL
    /// - `referrer`: The URL of the module that is importing the specifier
    /// - `kind`: The kind of resolution being performed (e.g. main module, import, dynamic import)
    ///
    /// # Returns
    /// - Some(Ok(ModuleSpecifier)): The resolved module specifier that will be imported
    /// - Some(Err(Error)): An error that will be returned to the caller, denying the import
    /// - None: Fall back to the default resolution behavior
    fn resolve(
        &mut self,
        specifier: &ModuleSpecifier,
        referrer: &str,
        kind: deno_core::ResolutionKind,
    ) -> Option<Result<ModuleSpecifier, ModuleLoaderError>> {
        None
    }

    /// Retrieve a JavaScript/TypeScript module from a given URL and return it as a string.
    ///
    /// This is the new version without the `requested_module_type` parameter.
    /// For backward compatibility, this method calls `import_with_type` by default.
    ///
    /// # Arguments
    /// - `specifier`: The module specifier to import, as an absolute URL
    /// - `referrer`: The URL of the module that is importing the specifier
    /// - `is_dyn_import`: Whether the import is a dynamic import or not
    ///
    /// # Returns
    /// - Some(Ok(String)): The module source code as a string
    /// - Some(Err(Error)): An error that will be returned to the caller
    /// - None: Fall back to the default import behavior
    fn import(
        &mut self,
        specifier: &ModuleSpecifier,
        referrer: Option<&ModuleSpecifier>,
        is_dyn_import: bool,
    ) -> Option<Result<String, ModuleLoaderError>> {
        // Call the old method for backward compatibility
        #[allow(deprecated)]
        self.import_with_type(
            specifier,
            referrer,
            is_dyn_import,
            RequestedModuleType::None,
        )
    }

    /// Retrieve a JavaScript/TypeScript module from a given URL (deprecated version)
    ///
    /// **DEPRECATED**: Use `import()` instead. The `requested_module_type` parameter is no longer used by the runtime.
    ///
    /// This method is kept for backward compatibility. Existing implementations should migrate to the new `import()` method.
    ///
    /// # Migration Guide
    /// ```rust,ignore
    /// // Old implementation:
    /// fn import_with_type(
    ///     &mut self,
    ///     specifier: &ModuleSpecifier,
    ///     referrer: Option<&ModuleSpecifier>,
    ///     is_dyn_import: bool,
    ///     requested_module_type: RequestedModuleType,
    /// ) -> Option<Result<String, ModuleLoaderError>> {
    ///     // your implementation
    /// }
    ///
    /// // New implementation (just rename and remove the last parameter):
    /// fn import(
    ///     &mut self,
    ///     specifier: &ModuleSpecifier,
    ///     referrer: Option<&ModuleSpecifier>,
    ///     is_dyn_import: bool,
    /// ) -> Option<Result<String, ModuleLoaderError>> {
    ///     // same implementation, don't use requested_module_type
    /// }
    /// ```
    #[deprecated(
        since = "0.8.0",
        note = "Use `import()` instead. The `requested_module_type` parameter is ignored by the runtime."
    )]
    fn import_with_type(
        &mut self,
        specifier: &ModuleSpecifier,
        referrer: Option<&ModuleSpecifier>,
        is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> Option<Result<String, ModuleLoaderError>> {
        // Call the new method (ignore requested_module_type)
        self.import(specifier, referrer, is_dyn_import)
    }

    /// Apply an optional transform to the source code after it has been imported
    /// This can be used to modify the source code before it is executed
    /// Or to cache the source code for later use
    ///
    /// The default behavior is to return the source code unmodified
    ///
    /// # Arguments
    /// - `specifier`: The module specifier that was imported
    /// - `source`: The source code of the module
    ///
    /// # Returns
    /// - Ok(ModuleSource): The modified source code
    /// - Err(Error): An error that will be returned to the caller
    ///
    /// # Errors
    /// - Any error that occurs during post-processing
    fn post_process(
        &mut self,
        specifier: &ModuleSpecifier,
        source: ModuleSource,
    ) -> Result<ModuleSource, ModuleLoaderError> {
        Ok(source)
    }
}
