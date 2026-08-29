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
        // Generic key=value assignments with keyword names.
        r#"(?i)\b((?:api[_-]?key|token|secret|password|passwd|authorization)\s*[:=]\s*["']?)([A-Za-z0-9_\-./+=]{12,})(["']?)"#,
        r#"(?i)\b(bearer\s+)([A-Za-z0-9_\-./+=]{12,})"#,
        // Provider-prefixed tokens first: the prefix itself identifies the
        // secret class, so no minimum length beyond the provider's own
        // shape. `sk-ant-` must run before the generic `sk-` below or the
        // generic pattern swallows the more specific prefix.
        r#"\b(sk-ant-)([A-Za-z0-9_\-]{16,})"#,
        r#"\b(gh[pousr]_)([A-Za-z0-9]{30,})"#,
        r#"\b(github_pat_)([A-Za-z0-9_]{22,})"#,
        r#"\b(AKIA)([0-9A-Z]{16})"#,
        r#"\b(xox[baprs]-)([A-Za-z0-9\-]{10,})"#,
        r#"\b(glpat-)([A-Za-z0-9_\-]{20,})"#,
        r#"\b(AIza)([0-9A-Za-z_\-]{35})"#,
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

    #[test]
    fn redacts_provider_prefixed_tokens() {
        // Like the generic shapes, the provider PREFIX survives (it names
        // the secret class); only the credential body is redacted.
        let cases = [
            ("ghp_0123456789abcdefghijklmnopqrstuvwxyzAB", "ghp_[REDACTED_SECRET]"),
            ("github_pat_0123456789ABCDEFGHIJKLMNOPQR", "github_pat_[REDACTED_SECRET]"),
            ("AKIA0123456789ABCDEF", "AKIA[REDACTED_SECRET]"),
            ("xoxb-0123456789abcdef-ABCDEF", "xoxb-[REDACTED_SECRET]"),
            ("glpat-0123456789abcdefghij", "glpat-[REDACTED_SECRET]"),
            ("sk-ant-api03-0123456789abcdef", "sk-ant-[REDACTED_SECRET]"),
            ("AIza0123456789abcdefghijklmnopqrstuvwxy", "AIza[REDACTED_SECRET]"),
        ];

        for (input, expected) in cases {
            let redacted = redact_secrets(input);
            assert_eq!(redacted, expected, "input: {input}");
        }
    }
}
