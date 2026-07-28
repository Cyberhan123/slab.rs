use thiserror::Error;

/// A specialized [`Result`] for [`MtmdError`].
pub type Result<T> = std::result::Result<T, MtmdError>;

/// Errors returned by the slab-mtmd safe wrapper.
#[derive(Debug, Error)]
pub enum MtmdError {
    #[error("mtmd context creation failed (mmproj load returned null)")]
    ContextCreateFailed,
    #[error("mtmd bitmap creation failed")]
    BitmapCreateFailed,
    #[error("mtmd tokenize failed (rc={0})")]
    TokenizeError(i32),
    #[error("mtmd encode failed (rc={0})")]
    EncodeError(i32),
    #[error("mtmd eval failed (rc={0})")]
    EvalError(i32),
    #[error("path contains invalid UTF-8 or a NUL byte")]
    InvalidPath,
    #[error("string contains a NUL byte")]
    NulByte(#[from] std::ffi::NulError),
    #[error("native library load error")]
    Load(#[from] libloading::Error),
}
