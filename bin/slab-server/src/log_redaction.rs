use regex::{Captures, Regex};
use std::sync::LazyLock;

static BEARER_SECRET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?i)\\b(Bearer\\s+)([A-Za-z0-9._~+/-]{8,})").unwrap());
static SECRET_URI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("secret://[A-Za-z0-9._~+/-]+").unwrap());
static OPENAI_SECRET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("\\bsk-[A-Za-z0-9][A-Za-z0-9_-]{8,}").unwrap());
static KEY_VALUE_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        "(?i)(\"?)(token|api[_-]?key|secret|password)(\"?)(\\s*[=:]\\s*)(\"?)([A-Za-z0-9][^\"\\s,;}]{5,})(\"?)",
    )
    .unwrap()
});

pub(crate) fn redact_log_text(input: &str) -> String {
    let redacted = BEARER_SECRET.replace_all(input, "${1}<redacted>");
    let redacted = OPENAI_SECRET.replace_all(&redacted, "sk-<redacted>");
    let redacted = SECRET_URI.replace_all(&redacted, "secret://<redacted>");
    KEY_VALUE_SECRET
        .replace_all(&redacted, |captures: &Captures<'_>| {
            format!(
                "{}{}{}{}{}<redacted>{}",
                &captures[1], &captures[2], &captures[3], &captures[4], &captures[5], &captures[7]
            )
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_shapes() {
        let input = concat!(
            "Authorization: Bearer secret-token-value ",
            "api_key=abcdef123456789 ",
            "token: \"ghp_1234567890abcdef\" ",
            "\"password\":\"hunter2-secret\" ",
            "secret://provider/openai"
        );

        let output = redact_log_text(input);

        assert!(output.contains("Bearer <redacted>"));
        assert!(output.contains("api_key=<redacted>"));
        assert!(output.contains("token: \"<redacted>\""));
        assert!(output.contains("\"password\":\"<redacted>\""));
        assert!(output.contains("secret://<redacted>"));
        assert!(!output.contains("secret-token-value"));
        assert!(!output.contains("abcdef123456789"));
        assert!(!output.contains("ghp_1234567890abcdef"));
        assert!(!output.contains("hunter2-secret"));
        assert!(!output.contains("provider/openai"));
    }

    #[test]
    fn redacts_openai_style_keys() {
        let output = redact_log_text("key sk-proj_abcdefghijklmnop");

        assert!(output.contains("sk-<redacted>"));
        assert!(!output.contains("abcdefghijklmnop"));
    }
}
