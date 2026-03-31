// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::SubtitleFormat;
use std::error::Error as StdError;

pub use crate::formats::idx::errors as idx_errors;
pub use crate::formats::srt::errors as srt_errors;
pub use crate::formats::ssa::errors as ssa_errors;

/// A result type that can be used wide for error handling.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Eq, PartialEq, Debug, thiserror::Error)]
/// Error kind for a crate-wide error.
pub enum ErrorKind {
    /// Parsing error
    #[error("parsing the subtitle data failed")]
    ParsingError,

    /// The file format is not supported by this library.
    #[error(
        "unknown file format, only SubRip (.srt), SubStationAlpha (.ssa/.ass) and VobSub (.idx and .sub) are supported at the moment"
    )]
    UnknownFileFormat,

    /// The file format is not supported by this library.
    #[error("error while decoding subtitle from bytes to string (wrong charset encoding?)")]
    DecodingError,

    /// The file format is not supported by this library.
    #[error(
        "could not determine character encoding from byte array (manually supply character encoding?)"
    )]
    EncodingDetectionError,

    /// The attempted operation does not work on binary subtitle formats.
    #[error("operation does not work on binary subtitle formats (only text formats)")]
    TextFormatOnly,

    /// The attempted operation does not work on this format (not supported in this version of this library).
    #[error(
        "updating subtitles is not implemented or supported by this subtitle library for this format: {}",
        format.get_name()
    )]
    UpdatingEntriesNotSupported {
        /// The format for which updating the subtitle entries is not supported.
        format: SubtitleFormat,
    },
}

/// The crate-wide error type.
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct Error {
    kind: ErrorKind,
    #[source]
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    /// Returns the actual error kind for this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub(crate) fn with_source<E>(kind: ErrorKind, source: E) -> Error
    where
        E: StdError + Send + Sync + 'static,
    {
        Error { kind, source: Some(Box::new(source)) }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error { kind, source: None }
    }
}

/// Creates the `Error`-context type for an ErrorKind and associated conversion methods.
macro_rules! define_error {
    ($error:ident, $kind:ident) => {
        /// The error structure which containes the error kind enum variant and an optional source.
        #[derive(Debug, thiserror::Error)]
        #[error("{kind}")]
        pub struct $error {
            kind: $kind,
            #[source]
            source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
        }

        impl $error {
            /// Get inner error enum variant.
            pub fn kind(&self) -> &$kind {
                &self.kind
            }

            #[allow(dead_code)]
            pub(crate) fn with_source<E>(kind: $kind, source: E) -> $error
            where
                E: std::error::Error + Send + Sync + 'static,
            {
                $error { kind, source: Some(Box::new(source)) }
            }
        }

        impl From<$kind> for $error {
            fn from(kind: $kind) -> $error {
                $error { kind, source: None }
            }
        }
    };
}
