//! On-disk persistence for the ggml.llama kv-cache.
//!
//! Each agent thread gets a per-(model, session) snapshot directory under the
//! configured kv-cache root. The snapshot is the per-sequence state byte blob
//! (the same `state: Arc<[u8]>` the in-process snapshot path in `runtime.rs`
//! produces/consumes) plus a small JSON sidecar carrying the cached prompt
//! prefix, grammar, `n_past` and worker id.
//!
//! Layout: `<root>/<model_fp>/<sanitized_session_key>/{snapshot.bin, sidecar.json}`
//!
//! All I/O is best-effort: failures are logged and degrade to a cache miss /
//! skipped write. They never fail an inference turn.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use slab_llama::LlamaSessionSnapshot;
use tracing::warn;

const SNAPSHOT_FILE: &str = "snapshot.bin";
const SIDECAR_FILE: &str = "sidecar.json";
/// Best-effort per-model session cap; oldest entries (by mtime) are evicted.
const MAX_SESSIONS_PER_MODEL: usize = 16;

/// Stable identity for a loaded model, used as the top-level kv-cache directory.
///
/// Composed from the model path string + parameter count + byte size and
/// reduced to a short hex digest. A collision only ever causes a cache miss.
#[derive(Debug, Clone)]
pub(crate) struct ModelFingerprint(String);

impl ModelFingerprint {
    pub(crate) fn compute(model_path: &str, n_params: u64, model_size: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        model_path.hash(&mut hasher);
        n_params.hash(&mut hasher);
        model_size.hash(&mut hasher);
        Self(format!("{:016x}", hasher.finish()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// A snapshot restored from disk, ready to seed an in-process session binding.
pub(crate) struct CachedSession {
    pub snapshot: LlamaSessionSnapshot,
    /// The prompt prefix already prefilled when the snapshot was taken.
    pub cached_prompt: String,
    pub grammar: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    cached_prompt: String,
    grammar: Option<String>,
    n_past: i32,
    worker_id: usize,
}

/// Best-effort on-disk store for kv-cache snapshots.
#[derive(Debug)]
pub(crate) struct KvCacheStore {
    root: PathBuf,
}

impl KvCacheStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn session_dir(&self, model_fp: &ModelFingerprint, session_key: &str) -> PathBuf {
        self.root.join(model_fp.as_str()).join(sanitize_segment(session_key))
    }

    /// Load a cached snapshot for the (model, session) pair, if present and
    /// readable. Any I/O or parse error is logged and treated as a miss.
    pub(crate) fn load(
        &self,
        model_fp: &ModelFingerprint,
        session_key: &str,
    ) -> Option<CachedSession> {
        let dir = self.session_dir(model_fp, session_key);

        let blob = read_or_miss(&dir.join(SNAPSHOT_FILE), "snapshot")?;
        let sidecar_bytes = read_or_miss(&dir.join(SIDECAR_FILE), "sidecar")?;
        let sidecar: Sidecar = match serde_json::from_slice(&sidecar_bytes) {
            Ok(value) => value,
            Err(error) => {
                warn!(%error, dir = %dir.display(), "kv-cache: sidecar parse failed; miss");
                return None;
            }
        };

        Some(CachedSession {
            snapshot: LlamaSessionSnapshot {
                worker_id: sidecar.worker_id,
                n_past: sidecar.n_past,
                state: Arc::from(blob),
            },
            cached_prompt: sidecar.cached_prompt,
            grammar: sidecar.grammar,
        })
    }

    /// Persist a snapshot atomically (per file: tmp + rename). Best-effort.
    /// After writing, opportunistically evicts oldest sessions for this model.
    pub(crate) fn save(
        &self,
        model_fp: &ModelFingerprint,
        session_key: &str,
        snapshot: &LlamaSessionSnapshot,
        cached_prompt: &str,
        grammar: Option<&str>,
    ) {
        let dir = self.session_dir(model_fp, session_key);
        if let Err(error) = fs::create_dir_all(&dir) {
            warn!(%error, dir = %dir.display(), "kv-cache: create_dir_all failed; skipping write");
            return;
        }

        let sidecar = Sidecar {
            cached_prompt: cached_prompt.to_owned(),
            grammar: grammar.map(str::to_owned),
            n_past: snapshot.n_past,
            worker_id: snapshot.worker_id,
        };
        let sidecar_bytes = match serde_json::to_vec(&sidecar) {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!(%error, "kv-cache: sidecar serialize failed; skipping write");
                return;
            }
        };

        if let Err(error) = write_atomic(&dir.join(SNAPSHOT_FILE), snapshot.state.as_ref()) {
            warn!(%error, dir = %dir.display(), "kv-cache: snapshot write failed");
            return;
        }
        if let Err(error) = write_atomic(&dir.join(SIDECAR_FILE), &sidecar_bytes) {
            warn!(%error, dir = %dir.display(), "kv-cache: sidecar write failed");
        }

        self.evict_lru(model_fp);
    }

    /// Best-effort per-model LRU eviction: keep the newest `MAX_SESSIONS_PER_MODEL`
    /// sessions (by modification time), remove the rest.
    fn evict_lru(&self, model_fp: &ModelFingerprint) {
        let model_dir = self.root.join(model_fp.as_str());
        let Some(mut entries) = collect_sessions_with_mtime(&model_dir) else {
            return;
        };
        if entries.len() <= MAX_SESSIONS_PER_MODEL {
            return;
        }
        entries.sort_by_key(|(_, mtime)| *mtime);
        let to_remove = entries.len().saturating_sub(MAX_SESSIONS_PER_MODEL);
        for (path, _) in entries.into_iter().take(to_remove) {
            if let Err(error) = fs::remove_dir_all(&path) {
                warn!(%error, dir = %path.display(), "kv-cache: evict remove_dir_all failed");
            }
        }
    }
}

/// Read a file fully, returning `None` (miss) on NotFound / any read error
/// (after logging the non-NotFound errors).
fn read_or_miss(path: &Path, label: &str) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            warn!(%error, path = %path.display(), label, "kv-cache: {label} read failed; miss");
            None
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn collect_sessions_with_mtime(dir: &Path) -> Option<Vec<(PathBuf, SystemTime)>> {
    let mut out = Vec::new();
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            warn!(%error, dir = %dir.display(), "kv-cache: read_dir for eviction failed");
            return None;
        }
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        match metadata.modified() {
            Ok(mtime) => out.push((path, mtime)),
            Err(_) => continue,
        }
    }
    Some(out)
}

/// Turn an arbitrary session_key into a single safe path segment.
fn sanitize_segment(key: &str) -> String {
    key.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(n_past: i32, state: &[u8]) -> LlamaSessionSnapshot {
        LlamaSessionSnapshot { worker_id: 0, n_past, state: Arc::from(state) }
    }

    #[test]
    fn fingerprint_is_deterministic_and_sensitive() {
        let a = ModelFingerprint::compute("/models/x.gguf", 1_000, 5_000);
        let a2 = ModelFingerprint::compute("/models/x.gguf", 1_000, 5_000);
        assert_eq!(a.as_str(), a2.as_str(), "same inputs -> same fingerprint");

        let b = ModelFingerprint::compute("/models/x.gguf", 1_001, 5_000);
        assert_ne!(a.as_str(), b.as_str(), "n_params changes the fingerprint");

        let c = ModelFingerprint::compute("/models/y.gguf", 1_000, 5_000);
        assert_ne!(a.as_str(), c.as_str(), "path changes the fingerprint");

        assert!(a.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize_segment("agent:thread-1"), "agent_thread-1");
        assert_eq!(sanitize_segment("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_segment("ok_123"), "ok_123");
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp = ModelFingerprint::compute("/models/x.gguf", 100, 500);

        assert!(store.load(&fp, "agent:thread-1").is_none(), "no snapshot before save");

        store.save(
            &fp,
            "agent:thread-1",
            &snapshot(42, &[1, 2, 3, 4]),
            "prefilled-prefix",
            Some("grammar"),
        );

        let cached = store.load(&fp, "agent:thread-1").expect("snapshot should load after save");
        assert_eq!(cached.snapshot.n_past, 42);
        assert_eq!(cached.snapshot.state.as_ref(), &[1, 2, 3, 4]);
        assert_eq!(cached.snapshot.worker_id, 0);
        assert_eq!(cached.cached_prompt, "prefilled-prefix");
        assert_eq!(cached.grammar.as_deref(), Some("grammar"));
    }

    #[test]
    fn different_sessions_are_isolated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp = ModelFingerprint::compute("/models/x.gguf", 100, 500);

        store.save(&fp, "agent:a", &snapshot(1, &[1]), "pa", None);
        store.save(&fp, "agent:b", &snapshot(2, &[2, 2]), "pb", None);

        let a = store.load(&fp, "agent:a").expect("a present");
        let b = store.load(&fp, "agent:b").expect("b present");
        assert_eq!(a.snapshot.state.as_ref(), &[1]);
        assert_eq!(b.snapshot.state.as_ref(), &[2, 2]);
    }

    #[test]
    fn different_models_are_isolated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp1 = ModelFingerprint::compute("/models/a.gguf", 100, 500);
        let fp2 = ModelFingerprint::compute("/models/b.gguf", 100, 500);

        store.save(&fp1, "agent:x", &snapshot(1, &[1]), "p", None);
        assert!(store.load(&fp2, "agent:x").is_none(), "different model -> miss");
        assert!(store.load(&fp1, "agent:x").is_some());
    }

    #[test]
    fn load_treats_corrupt_sidecar_as_miss() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp = ModelFingerprint::compute("/models/x.gguf", 100, 500);
        let dir = store.session_dir(&fp, "agent:c");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SNAPSHOT_FILE), b"bytes").unwrap();
        fs::write(dir.join(SIDECAR_FILE), b"not json").unwrap();

        assert!(store.load(&fp, "agent:c").is_none(), "corrupt sidecar should be a miss");
    }

    #[test]
    fn save_overwrites_previous_snapshot() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp = ModelFingerprint::compute("/models/x.gguf", 100, 500);

        store.save(&fp, "agent:k", &snapshot(1, &[1]), "p1", None);
        store.save(&fp, "agent:k", &snapshot(9, &[9, 9, 9]), "p2", Some("g"));

        let cached = store.load(&fp, "agent:k").expect("present after overwrite");
        assert_eq!(cached.snapshot.n_past, 9);
        assert_eq!(cached.snapshot.state.as_ref(), &[9, 9, 9]);
        assert_eq!(cached.cached_prompt, "p2");
    }

    #[test]
    fn evict_lru_keeps_newest_sessions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = KvCacheStore::new(tmp.path().to_path_buf());
        let fp = ModelFingerprint::compute("/models/x.gguf", 100, 500);

        // Write MAX+2 sessions; each save triggers an eviction pass.
        for index in 0..(MAX_SESSIONS_PER_MODEL + 2) {
            store.save(
                &fp,
                &format!("agent:{index}"),
                &snapshot(index as i32, &[index as u8]),
                "p",
                None,
            );
        }

        // Eviction caps the per-model session count at MAX (modulo mtime
        // resolution ties — assert the upper bound, not an exact count).
        let model_dir = tmp.path().join(fp.as_str());
        let remaining = fs::read_dir(&model_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();
        assert!(
            remaining <= MAX_SESSIONS_PER_MODEL,
            "eviction should cap sessions per model at {MAX_SESSIONS_PER_MODEL}, got {remaining}"
        );

        // The newest session must always survive eviction.
        let last = MAX_SESSIONS_PER_MODEL + 1;
        assert!(store.load(&fp, &format!("agent:{last}")).is_some());
    }
}
