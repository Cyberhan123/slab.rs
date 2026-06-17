use regex::Regex;
use std::sync::OnceLock;

static SECRET_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

pub fn redact_secrets(input: &str) -> String {
    let mut output = input.to_owned();
    for pattern in SECRET_PATTERNS.get_or_init(secret_patterns) {
        output = pattern
            .replace_all(&output, |captures: &regex::Captures<'_>| {
                let prefix = captures.get(1).map_or("", |capture| capture.as_str());
                let suffix = captures.get(3).map_or("", |capture| capture.as_str());
                format!("{prefix}[REDACTED_SECRET]{suffix}")
            })
            .into_owned();
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
        let cases = [
            ("api_key = abcdefghijklmnop", "api_key = [REDACTED_SECRET]"),
            ("api-key=abcdefghijklmnop", "api-key=[REDACTED_SECRET]"),
            ("token='abcdefghijklmnop'", "token='[REDACTED_SECRET]'"),
            ("secret: abcdefghijklmnop", "secret: [REDACTED_SECRET]"),
            ("password=\"abcdefghijklmnop\"", "password=\"[REDACTED_SECRET]\""),
            ("passwd=abcdefghijklmnop", "passwd=[REDACTED_SECRET]"),
            (
                "Authorization: Bearer tokenvalue123456789",
                "Authorization: Bearer [REDACTED_SECRET]",
            ),
            ("bearer tokenvalue123456789", "bearer [REDACTED_SECRET]"),
            ("sk-abcdefghijklmnop", "sk-[REDACTED_SECRET]"),
        ];

        for (input, expected) in cases {
            assert_eq!(redact_secrets(input), expected);
        }
    }

    #[test]
    fn preserves_non_secret_text_and_short_values() {
        let input = "token=abcdefghijk\nfile=sketch-plan.md\npassword=short";

        assert_eq!(redact_secrets(input), input);
    }

    #[test]
    fn redacts_multiple_secrets_in_one_input() {
        let redacted = redact_secrets(
            "api_key=abcdefghijklmnop Authorization: Bearer tokenvalue123456789 sk-abcdefghijklmnop",
        );

        assert_eq!(redacted.matches("[REDACTED_SECRET]").count(), 3);
        assert!(!redacted.contains("abcdefghijklmnop"));
        assert!(!redacted.contains("tokenvalue123456789"));
    }
}
