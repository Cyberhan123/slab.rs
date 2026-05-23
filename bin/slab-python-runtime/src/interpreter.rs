//! Python interpreter initialisation and VFS bootstrap.
//!
//! Call [`init`] once per process to:
//!   1. Initialise CPython (via PyO3's `prepare_freethreaded_python`).
//!   2. Register the embedded stdlib VFS at `sys.meta_path[0]`.

use anyhow::Result;
use pyo3::prelude::*;

use crate::vfs::{EmbeddedStdlib, register};

/// Initialise CPython and install the embedded-stdlib VFS.
///
/// `stdlib` contains any `.py` files that should be resolvable without the
/// real filesystem. Pass an empty `EmbeddedStdlib` when no static modules are
/// needed (the VFS finder is still registered but will never match).
///
/// This function is idempotent: calling it more than once is harmless because
/// `pyo3::prepare_freethreaded_python` is also idempotent.
pub fn init(stdlib: EmbeddedStdlib) -> Result<()> {
    pyo3::prepare_freethreaded_python();

    Python::with_gil(|py| register(py, &stdlib).map_err(|e| anyhow::anyhow!("{e}")))?;

    Ok(())
}
