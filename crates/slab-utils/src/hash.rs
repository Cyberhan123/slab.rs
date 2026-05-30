use std::fs;
use std::io::{self, BufReader, Read};
use std::path::Path;

use ring::digest;

/// A SHA256 mismatch with normalized expected and actual hex values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256Mismatch {
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for Sha256Mismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "sha256 mismatch: expected {}, got {}", self.expected, self.actual)
    }
}

impl std::error::Error for Sha256Mismatch {}

/// Streaming SHA256 context that returns lowercase hex output.
pub struct Sha256HexContext {
    context: digest::Context,
}

impl Sha256HexContext {
    pub fn new() -> Self {
        Self { context: digest::Context::new(&digest::SHA256) }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.context.update(bytes);
    }

    pub fn finish(self) -> String {
        hex::encode(self.context.finish().as_ref())
    }
}

impl Default for Sha256HexContext {
    fn default() -> Self {
        Self::new()
    }
}

pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    hex::encode(digest::digest(&digest::SHA256, bytes).as_ref())
}

pub fn sha256_hex_reader(reader: &mut impl Read) -> io::Result<String> {
    let mut hasher = Sha256HexContext::new();
    let mut buffer = [0_u8; 1024 * 64];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finish())
}

pub fn sha256_hex_file(path: &Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    sha256_hex_reader(&mut reader)
}

pub fn verify_sha256_hex_expected(actual_hex: &str, expected: &str) -> Result<(), Sha256Mismatch> {
    let expected_hex = normalize_sha256_hex(expected);
    let actual_hex = normalize_sha256_hex(actual_hex);
    if actual_hex.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(Sha256Mismatch { expected: expected_hex.to_owned(), actual: actual_hex.to_owned() })
    }
}

fn normalize_sha256_hex(expected: &str) -> &str {
    let expected = expected.trim();
    let prefix = "sha256:";
    if expected.get(..prefix.len()).is_some_and(|value| value.eq_ignore_ascii_case(prefix)) {
        &expected[prefix.len()..]
    } else {
        expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn sha256_hex_bytes_hashes_known_values() {
        assert_eq!(sha256_hex_bytes(b"hello"), HELLO_SHA256);
        assert_eq!(sha256_hex_bytes(b""), EMPTY_SHA256);
    }

    #[test]
    fn sha256_hex_reader_matches_bytes_hash() {
        let mut reader = std::io::Cursor::new(b"hello");
        assert_eq!(sha256_hex_reader(&mut reader).unwrap(), HELLO_SHA256);
    }

    #[test]
    fn sha256_hex_file_matches_bytes_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.txt");
        fs::write(&path, b"hello").unwrap();
        assert_eq!(sha256_hex_file(&path).unwrap(), HELLO_SHA256);
    }

    #[test]
    fn sha256_hex_context_hashes_streamed_chunks() {
        let mut context = Sha256HexContext::new();
        context.update(b"he");
        context.update(b"llo");
        assert_eq!(context.finish(), HELLO_SHA256);
    }

    #[test]
    fn generated_hex_is_lowercase() {
        assert!(
            sha256_hex_bytes(b"hello")
                .chars()
                .all(|character| { character.is_ascii_digit() || character.is_ascii_lowercase() })
        );
    }

    #[test]
    fn verify_accepts_plain_prefixed_uppercase_and_whitespace() {
        assert!(verify_sha256_hex_expected(HELLO_SHA256, HELLO_SHA256).is_ok());
        assert!(
            verify_sha256_hex_expected(&format!("sha256:{HELLO_SHA256}"), HELLO_SHA256).is_ok()
        );
        assert!(
            verify_sha256_hex_expected(HELLO_SHA256, &format!("sha256:{HELLO_SHA256}")).is_ok()
        );
        assert!(
            verify_sha256_hex_expected(
                HELLO_SHA256,
                &format!("SHA256:{}", HELLO_SHA256.to_ascii_uppercase())
            )
            .is_ok()
        );
        assert!(
            verify_sha256_hex_expected(HELLO_SHA256, &format!("  sha256:{HELLO_SHA256}\n")).is_ok()
        );
    }

    #[test]
    fn verify_reports_normalized_mismatch_details() {
        let error = verify_sha256_hex_expected(
            HELLO_SHA256,
            " sha256:0000000000000000000000000000000000000000000000000000000000000000\n",
        )
        .unwrap_err();
        assert_eq!(
            error.expected,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(error.actual, HELLO_SHA256);
    }
}
