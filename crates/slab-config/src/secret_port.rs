//! Secret-port abstraction for the unified secret store (INFRA-02 / ADR-014).
//!
//! `crates/slab-config` owns the pure [`SecretPort`] trait; concrete adapters
//! (OS keyring, encrypted-file fallback) live in the composition roots
//! (`bin/slab-app/src-tauri` / `bin/slab-runtime`), so the `crates/` layer never
//! depends on a keyring binary. Config files store only the `secret://...`
//! handle, never the plaintext value.

use std::collections::HashMap;

/// Prefix marking a config value as a secret handle rather than plaintext.
pub const SECRET_HANDLE_PREFIX: &str = "secret://";

/// Resolves `secret://<kind>/<key>` handles to their secret value.
///
/// Implementations live in composition roots (host/runtime). The reference
/// [`EnvSecretAdapter`] resolves `secret://env/<VAR>` from the process
/// environment, useful for tests and as a non-keyring fallback.
pub trait SecretPort: Send + Sync {
    /// Resolve a secret handle. Returns `Ok(None)` when the backend has no
    /// value for the handle (distinct from a backend error).
    fn resolve(&self, handle: &str) -> Result<Option<String>, String>;
}

/// Reference [`SecretPort`] that resolves `secret://env/<VAR>` handles from the
/// process environment, with optional in-process overrides (for tests).
#[derive(Default)]
pub struct EnvSecretAdapter {
    overrides: HashMap<String, String>,
}

impl EnvSecretAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an in-process override for `secret://env/<key>` resolution.
    pub fn with_override(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.overrides.insert(key.into(), value.into());
        self
    }
}

impl SecretPort for EnvSecretAdapter {
    fn resolve(&self, handle: &str) -> Result<Option<String>, String> {
        let trimmed = handle.trim();
        let rest = trimmed.strip_prefix(SECRET_HANDLE_PREFIX).ok_or_else(|| {
            format!("not a secret handle (expected '{SECRET_HANDLE_PREFIX}...'): {trimmed}")
        })?;
        let (kind, name) = rest.split_once('/').ok_or_else(|| {
            format!(
                "invalid secret handle (expected '{SECRET_HANDLE_PREFIX}<kind>/<key>'): {trimmed}"
            )
        })?;

        match kind {
            "env" => {
                if let Some(value) = self.overrides.get(name) {
                    return Ok(Some(value.clone()));
                }
                Ok(std::env::var(name).ok())
            }
            other => Err(format!(
                "unsupported secret backend '{other}' for handle: {trimmed} (host keyring adapter not wired)"
            )),
        }
    }
}

/// True when `value` is a `secret://` handle (rather than plaintext).
pub fn is_secret_handle(value: &str) -> bool {
    value.trim().starts_with(SECRET_HANDLE_PREFIX)
}

/// Resolve a config value that may be a `secret://` handle or a plaintext
/// fallback (backward compatibility). Plaintext is returned verbatim; a handle
/// is resolved through `port`. A handle that resolves to nothing is an error.
pub fn resolve_secret_or_plain(port: &dyn SecretPort, value: &str) -> Result<String, String> {
    if !is_secret_handle(value) {
        return Ok(value.to_owned());
    }
    port.resolve(value)?.ok_or_else(|| format!("secret not found for handle: {}", value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_adapter_resolves_override_and_env_handles() {
        let port = EnvSecretAdapter::new().with_override("SLAB_TEST_OVERRIDE", "override-value");

        assert_eq!(
            port.resolve("secret://env/SLAB_TEST_OVERRIDE").unwrap(),
            Some("override-value".to_owned())
        );
        // Falls back to the process env when no override is registered.
        assert!(port.resolve("secret://env/PATH").unwrap().is_some());
    }

    #[test]
    fn env_adapter_rejects_unsupported_backend() {
        let port = EnvSecretAdapter::new();
        let error = port.resolve("secret://keyring/admin_token").expect_err("keyring not wired");

        assert!(error.contains("unsupported secret backend 'keyring'"));
    }

    #[test]
    fn invalid_handles_are_reported() {
        let port = EnvSecretAdapter::new();

        let missing_prefix = port.resolve("admin-token").expect_err("not a handle");
        assert!(missing_prefix.contains("not a secret handle"));

        let no_kind = port.resolve("secret://admin_token").expect_err("no kind/key");
        assert!(no_kind.contains("invalid secret handle"));
    }

    #[test]
    fn is_secret_handle_detects_handles() {
        assert!(is_secret_handle("secret://env/OPENAI_API_KEY"));
        assert!(is_secret_handle("  secret://env/X "));
        assert!(!is_secret_handle("sk-abcdef"));
        assert!(!is_secret_handle(""));
    }

    #[test]
    fn resolve_secret_or_plain_passes_plaintext_through() {
        let port = EnvSecretAdapter::new();
        assert_eq!(resolve_secret_or_plain(&port, "plaintext-token").unwrap(), "plaintext-token");
        assert_eq!(resolve_secret_or_plain(&port, "").unwrap(), "");
    }

    #[test]
    fn resolve_secret_or_plain_resolves_handles_and_errors_when_missing() {
        let port = EnvSecretAdapter::new().with_override("PRESENT", "value");

        assert_eq!(resolve_secret_or_plain(&port, "secret://env/PRESENT").unwrap(), "value");

        let error = resolve_secret_or_plain(&port, "secret://env/SLAB_DEFINITELY_MISSING")
            .expect_err("missing secret");
        assert!(error.contains("secret not found"));
    }
}
