use regex::Regex;
use std::sync::OnceLock;

static SECRET_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

pub fn redact_secrets(input: &str) -> String {
    let mut output = input.to_owned();
    for pattern in SECRET_PATTERNS.get_or_init(secret_patterns) {
        output = pattern.replace_all(&output, "$1[REDACTED_SECRET]").into_owned();
    }
    output
}

fn secret_patterns() -> Vec<Regex> {
    [
        r#"(?i)\b((?:api[_-]?key|token|secret|password|passwd|authorization)\s*[:=]\s*["']?)([A-Za-z0-9_\-./+=]{12,})(["']?)"#,
        r#"(?i)\b(bearer\s+)([A-Za-z0-9_\-./+=]{12,})"#,
        r#"\b(sk-)([A-Za-z0-9_\-]{16,})"#,
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("valid secret regex"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_shapes() {
        let redacted =
            redact_secrets("api_key = abcdefghijklmnop\nAuthorization: Bearer tokenvalue123456789");

        assert!(redacted.contains("api_key = [REDACTED_SECRET]"));
        assert!(redacted.contains("Bearer [REDACTED_SECRET]"));
        assert!(!redacted.contains("abcdefghijklmnop"));
    }
}
