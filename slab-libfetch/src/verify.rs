use crate::error::FetchError;
use sha2::{Digest, Sha256};

/// Verify the SHA256 checksum of `data` against `expected`.
///
/// `expected` must be in either `"sha256:<hex>"` or plain `"<hex>"` format.
/// Returns `Ok(())` on a match or `Err(FetchError::ChecksumMismatch)` on
/// mismatch.
pub fn verify_sha256(data: &[u8], expected: &str) -> Result<(), FetchError> {
    let hex_expected = expected.strip_prefix("sha256:").unwrap_or(expected);

    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = format!("{:x}", hasher.finalize());

    if digest.eq_ignore_ascii_case(hex_expected) {
        Ok(())
    } else {
        Err(FetchError::ChecksumMismatch {
            expected: hex_expected.to_string(),
            actual: digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // echo -n "hello" | sha256sum
    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn test_verify_sha256_match_plain() {
        let result = verify_sha256(b"hello", HELLO_SHA256);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn test_verify_sha256_match_with_prefix() {
        let expected = format!("sha256:{}", HELLO_SHA256);
        let result = verify_sha256(b"hello", &expected);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let result = verify_sha256(b"hello", "sha256:0000000000000000000000000000000000000000000000000000000000000000");
        match result {
            Err(FetchError::ChecksumMismatch { expected, actual }) => {
                assert_eq!(expected, "0000000000000000000000000000000000000000000000000000000000000000");
                assert_eq!(actual, HELLO_SHA256);
            }
            other => panic!("expected ChecksumMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_sha256_empty_bytes() {
        // SHA256 of empty input
        let empty_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(verify_sha256(b"", empty_sha256).is_ok());
    }
}
