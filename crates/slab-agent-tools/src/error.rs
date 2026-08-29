//! Tool-layer error normalization.
//!
//! `std::io::Error`'s Display is the OS-localized message (e.g. 中文
//! "系统找不到指定的文件。 (os error 2)" on a Chinese Windows host). Feeding that
//! straight back to the model produces mixed-language tool errors that
//! English-trained models parse poorly, and gives no stable code to grep for.
//! [`io_tool_error`] maps the common io kinds to fixed English templates with
//! a bracketed code prefix; unknown kinds keep the raw message but gain the
//! code prefix and the numeric OS error code.

use std::path::Path;

use slab_agent::AgentError;

/// Map an io error from a filesystem action on `path` to a stable, coded,
/// English tool error.
pub(crate) fn io_tool_error(action: &str, path: &Path, error: &std::io::Error) -> AgentError {
    let path_display = path.display();
    let message = match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("[io.not_found] failed to {action}: '{path_display}' not found")
        }
        std::io::ErrorKind::PermissionDenied => {
            format!(
                "[io.permission_denied] failed to {action}: permission denied for '{path_display}'"
            )
        }
        std::io::ErrorKind::AlreadyExists => {
            format!("[io.already_exists] failed to {action}: '{path_display}' already exists")
        }
        std::io::ErrorKind::InvalidData => format!(
            "[io.invalid_data] failed to {action}: '{path_display}' is not valid UTF-8 (binary file?)"
        ),
        std::io::ErrorKind::IsADirectory => {
            format!("[io.is_a_directory] failed to {action}: '{path_display}' is a directory")
        }
        std::io::ErrorKind::NotADirectory => {
            format!("[io.not_a_directory] failed to {action}: '{path_display}' is not a directory")
        }
        std::io::ErrorKind::InvalidInput => {
            format!("[io.invalid_input] failed to {action} on '{path_display}: {error}")
        }
        _ => match error.raw_os_error() {
            Some(code) => format!(
                "[io.unclassified] failed to {action} on '{path_display}': {error} (os error {code})"
            ),
            None => format!("[io.unclassified] failed to {action} on '{path_display}': {error}"),
        },
    };
    AgentError::ToolExecution(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_tool_error_maps_kinds_to_codes_and_english() {
        let path = Path::new("some/file.txt");

        let not_found =
            io_tool_error("read file", path, &std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(not_found.to_string().contains("[io.not_found]"), "{not_found}");
        assert!(not_found.to_string().contains("'some/file.txt' not found"), "{not_found}");

        let denied = io_tool_error(
            "write file",
            path,
            &std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(denied.to_string().contains("[io.permission_denied]"), "{denied}");
        assert!(denied.to_string().contains("permission denied"), "{denied}");

        let invalid = io_tool_error(
            "read file",
            path,
            &std::io::Error::from(std::io::ErrorKind::InvalidData),
        );
        assert!(invalid.to_string().contains("[io.invalid_data]"), "{invalid}");
        assert!(invalid.to_string().contains("binary file"), "{invalid}");
    }

    #[test]
    fn io_tool_error_keeps_raw_message_and_code_for_unknown_kinds() {
        // os error 5 = ERROR_ACCESS_DENIED on Windows; kind is Unclassified
        // (or PermissionDenied on Unix where 5 maps to EIO — either way this
        // exercises the fallback arms).
        let path = Path::new("x");
        let raw = std::io::Error::from_raw_os_error(5);
        let rendered = io_tool_error("stat", path, &raw).to_string();
        assert!(
            rendered.contains("os error 5") || rendered.contains("[io.permission_denied]"),
            "{rendered}"
        );
    }
}
