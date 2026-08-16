//! Standalone functions that have no associated type.

use crate::Parakeet;
use std::ffi::CStr;

impl Parakeet {
    /// Callback to control logging output: default behaviour is to print to stderr.
    ///
    /// # Safety
    /// The callback must be safe to call from C (i.e. no panicking, no unwinding, etc).
    ///
    /// # C++ equivalent
    /// `void parakeet_log_set(ggml_log_callback log_callback, void * user_data);`
    pub unsafe fn set_log_callback(
        &self,
        log_callback: crate::ParakeetLogCallback,
        user_data: *mut std::ffi::c_void,
    ) {
        unsafe {
            self.lib.parakeet_log_set(log_callback, user_data);
        }
    }

    /// Get the current parakeet version.
    pub fn get_parakeet_version(&self) -> &'static str {
        let ptr = unsafe { self.lib.parakeet_version() };
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr) }.to_str().expect("parakeet version should be valid UTF-8")
    }

    /// Print system information.
    ///
    /// # C++ equivalent
    /// `const char * parakeet_print_system_info(void)`
    pub fn print_system_info(&self) -> &'static str {
        let c_buf = unsafe { self.lib.parakeet_print_system_info() };
        let c_str = unsafe { CStr::from_ptr(c_buf) };
        c_str.to_str().unwrap()
    }
}
