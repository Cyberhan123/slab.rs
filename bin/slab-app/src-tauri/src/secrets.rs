//! Host-only secret resolution (INFRA-02 / ADR-014).
//!
//! `crates/slab-config` owns the pure [`SecretPort`] trait and the in-process
//! [`EnvSecretAdapter`]; this module adds the OS keyring adapter that lives in
//! the composition root (the desktop host), so the `crates/*` layer never
//! depends on a keyring binary. Config files store only
//! `secret://keyring/<service>/<key>` handles; this adapter resolves them
//! against the platform credential store (Windows Credential Manager /
//! macOS Keychain / Linux kernel keyring).
//!
//! `secret://env/<VAR>` handles keep working everywhere (resolved in-process by
//! the env adapter), so provider configs can stop storing plaintext keys today;
//! `secret://keyring/...` handles resolve host-side via this adapter. Future
//! work: the host materialises resolved keyring secrets into the server
//! sidecar environment at launch.

use slab_config::secret_port::{EnvSecretAdapter, SECRET_HANDLE_PREFIX, SecretPort};

/// Backend kind for OS-keyring secret handles.
const KEYRING_BACKEND: &str = "keyring";

/// OS keyring-backed [`SecretPort`] for the desktop host.
///
/// Resolves `secret://keyring/<service>/<key>` via the platform credential
/// store, and delegates every other handle to [`EnvSecretAdapter`] (so a single
/// port resolves both `secret://keyring/...` and `secret://env/...`).
#[derive(Default)]
pub struct KeyringSecretAdapter {
    env_fallback: EnvSecretAdapter,
}

impl KeyringSecretAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretPort for KeyringSecretAdapter {
    fn resolve(&self, handle: &str) -> Result<Option<String>, String> {
        if let Some((service, key)) = parse_keyring_handle(handle) {
            return resolve_keyring(&service, &key);
        }
        // `secret://env/...` (and a clear error for unknown backends) is handled
        // by the in-process env adapter from slab-config.
        self.env_fallback.resolve(handle)
    }
}

/// Verify a `secret://...` handle resolves to a present secret, **without ever
/// returning the secret value** across the IPC boundary.
///
/// Returns `true` when the keyring entry / env var is present, `false` when it
/// is absent, and an error string for malformed handles or an unavailable
/// backend. Lets a settings UI confirm a keyring reference is wired without
/// exposing the credential itself.
#[tauri::command]
pub fn verify_secret_handle(
    adapter: tauri::State<'_, KeyringSecretAdapter>,
    handle: String,
) -> Result<bool, String> {
    Ok(adapter.resolve(&handle)?.is_some())
}

/// Parse `secret://keyring/<service>/<key>` into `(service, key)`.
///
/// Returns `None` for any other shape, including non-keyring backends (which
/// are delegated to the env adapter) and handles missing a service or key.
pub(crate) fn parse_keyring_handle(handle: &str) -> Option<(String, String)> {
    let rest = handle.trim().strip_prefix(SECRET_HANDLE_PREFIX)?;
    let (kind, name) = rest.split_once('/')?;
    if kind != KEYRING_BACKEND {
        return None;
    }
    let (service, key) = name.split_once('/')?;
    let service = service.trim();
    let key = key.trim();
    if service.is_empty() || key.is_empty() {
        return None;
    }
    Some((service.to_owned(), key.to_owned()))
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn resolve_keyring(service: &str, key: &str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(service, key).map_err(|error| {
        format!("keyring entry for '{service}/{key}' could not be created: {error}")
    })?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("keyring read for '{service}/{key}' failed: {error}")),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn resolve_keyring(_service: &str, _key: &str) -> Result<Option<String>, String> {
    Err("keyring backend is not available on this platform".to_owned())
}

#[cfg(test)]
mod tests {
    use slab_config::secret_port::SecretPort;

    use super::{KeyringSecretAdapter, parse_keyring_handle};

    #[test]
    fn parse_keyring_handle_valid_and_invalid() {
        assert_eq!(
            parse_keyring_handle("secret://keyring/slab-server/admin"),
            Some(("slab-server".to_owned(), "admin".to_owned()))
        );
        // Whitespace is trimmed.
        assert_eq!(
            parse_keyring_handle("  secret://keyring/svc/k  "),
            Some(("svc".to_owned(), "k".to_owned()))
        );
        // Non-keyring backends are not keyring handles.
        assert_eq!(parse_keyring_handle("secret://env/FOO"), None);
        assert_eq!(parse_keyring_handle("secret://unknownbackend/x/y"), None);
        // Missing service or key.
        assert_eq!(parse_keyring_handle("secret://keyring/onlyone"), None);
        assert_eq!(parse_keyring_handle("secret://keyring//k"), None);
        assert_eq!(parse_keyring_handle("secret://keyring/s/"), None);
        // Not a secret handle at all.
        assert_eq!(parse_keyring_handle("plaintext-token"), None);
    }

    #[test]
    fn resolve_missing_keyring_handle_yields_no_secret() {
        // Nothing is stored under this (service, key) in the OS store, so the
        // adapter must never surface a secret. It returns Ok(None) where the
        // backend is available (NoEntry) or an error where the platform has no
        // usable keyring — both are acceptable; a real secret would be a bug.
        let adapter = KeyringSecretAdapter::new();
        let result = adapter.resolve("secret://keyring/slab-infra02-missing/nope");
        assert!(
            matches!(result, Ok(None) | Err(_)),
            "missing keyring handle must not resolve to a secret, got {result:?}"
        );
    }

    #[test]
    fn resolve_delegates_env_handle_to_env_adapter() {
        let adapter = KeyringSecretAdapter::new();
        // An env var name that is effectively guaranteed unset resolves to None.
        let result = adapter.resolve("secret://env/SLAB_INFRA02_DEFINITELY_MISSING").unwrap();
        assert!(result.is_none(), "unset env handle must resolve to None");
    }

    #[test]
    fn resolve_rejects_non_handle_and_unknown_backend() {
        let adapter = KeyringSecretAdapter::new();
        assert!(adapter.resolve("not-a-handle").is_err());
        assert!(adapter.resolve("secret://unknownbackend/x/y").is_err());
    }
}
