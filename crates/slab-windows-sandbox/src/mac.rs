//! HMAC-SHA256 sign/verify over IPC payloads, via `ring::hmac`. Both the orchestrator
//! (non-elevated) and the helper (elevated) share the key (same-user DPAPI-sealed), so the tag
//! proves the payload was produced by a party holding the key — integrity, not caller-auth.

use ring::hmac;

/// Sign `bytes` with `key`, returning the raw 32-byte tag.
pub(crate) fn sign(key: &[u8], bytes: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, bytes).as_ref().to_vec()
}

/// Verify `bytes` against `tag`. Constant-time via `ring`.
pub(crate) fn verify(key: &[u8], bytes: &[u8], tag: &[u8]) -> bool {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::verify(&key, bytes, tag).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let key = b"test-key-32-bytes-_______________"[..32].to_vec();
        let msg = b"hello slab-sandbox";
        let tag = sign(&key, msg);
        assert!(verify(&key, msg, &tag));
    }

    #[test]
    fn verify_rejects_tampered() {
        let key = b"different-test-key-______________"[..32].to_vec();
        let msg = b"hello slab-sandbox";
        let tag = sign(&key, msg);
        // Wrong key must not verify.
        let other = b"other-key-32-bytes-_______________"[..32].to_vec();
        assert!(!verify(&other, msg, &tag));
        // Tampered message must not verify.
        assert!(!verify(&key, b"hello slab-SANDBOX", &tag));
    }
}
