//! Linux sandbox implementation using bubblewrap (bwrap) with Landlock fallback.
//!
//! Uses `PR_SET_NO_NEW_PRIVS` in-process before exec, and constructs a bwrap
//! invocation that provides filesystem isolation.

pub mod bwrap;
pub mod driver;

pub use driver::LinuxSandboxDriver;
