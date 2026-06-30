use std::path::Path;

use slab_agent::ToolApprovalRequest;

const SENSITIVE_MARKERS: &[&str] = &[".env", ".pem", ".slab/slab.db", ".ssh"];

pub(crate) fn approval_for_path(
    tool_name: &str,
    field: &str,
    value: Option<&str>,
) -> Option<ToolApprovalRequest> {
    let value = value?;
    if !is_sensitive_path_like(value) {
        return None;
    }
    Some(ToolApprovalRequest {
        command: format!("{tool_name} requires approval for sensitive {field}: {value}"),
    })
}

pub(crate) fn approval_for_values(
    tool_name: &str,
    fields: &[(&str, Option<&str>)],
) -> Option<ToolApprovalRequest> {
    fields.iter().find_map(|(field, value)| approval_for_path(tool_name, field, *value))
}

fn is_sensitive_path_like(value: &str) -> bool {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    if normalized.trim().is_empty() {
        return false;
    }
    if normalized == "~/.ssh" || normalized.starts_with("~/.ssh/") {
        return true;
    }
    if normalized.contains(".slab/slab.db") {
        return true;
    }
    if normalized.split('/').any(|component| component == ".ssh") {
        return true;
    }
    Path::new(&normalized).components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        SENSITIVE_MARKERS.iter().any(|marker| text.contains(marker))
            || contains_sensitive_word(&text)
    })
}

fn contains_sensitive_word(value: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|part| matches!(part, "token" | "credential" | "credentials"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sensitive_path_like_values() {
        for value in [
            ".env",
            "config/.env.local",
            "id_rsa.pem",
            "service_credentials.json",
            "api-token.txt",
            ".slab/slab.db",
            "~/.ssh/config",
            "C:\\Users\\me\\.ssh\\id_rsa",
        ] {
            assert!(is_sensitive_path_like(value), "{value}");
        }
    }

    #[test]
    fn ignores_non_sensitive_path_like_values() {
        for value in ["src/main.rs", "docs/tokenization.md", "credentialed-notes.md"] {
            assert!(!is_sensitive_path_like(value), "{value}");
        }
    }
}
