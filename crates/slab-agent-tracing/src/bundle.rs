//! Trace bundle directory layout (L3 semantic replay).
//!
//! A trace bundle groups all raw evidence for one ROOT thread under a single
//! directory so a future reducer (Slice 10) can reconstruct the conversation
//! the model actually saw. Layout:
//!
//! ```text
//! <logs_dir>/agent_trace/trace-<trace_uuid>-<root_thread_id>/
//!     manifest.json     # written ONCE at creation (5 fields)
//!     trace.jsonl       # append-only event stream (see writer::TraceWriter)
//!     payloads/*.json   # large bodies referenced by events (request/response/...)
//!     state.json        # OPTIONAL reducer cache (Slice 10), not written in Slice 9
//! ```
//!
//! ## Multi-writer / multi-turn / cross-process safety
//!
//! `slab-runtime` is a SEPARATE process. Today it carries an
//! [`crate::AgentTraceContext`] across the FFI boundary and writes the simple
//! per-session JSONL via [`crate::record_json_from_context`]; it does NOT
//! depend on this crate at the FFI boundary and does NOT write into a bundle.
//!
//! Within the main process (`slab-app-core` / `slab-agent`) multiple writers
//! share one bundle: a root thread + subagent threads, or a fresh
//! [`crate::writer::TraceWriter`] opened per turn (the documented pattern in
//! [`crate::writer::TraceWriter::for_thread`]). Payload ids are uuid v4
//! (`p-<uuid>`), so they are unique across ALL writers, turns, and processes —
//! a later write can NEVER truncate an earlier payload's content. This was a
//! real same-process defect when the id was a per-instance ordinal counter
//! (reset to 0 on every `open`); the unique-id design fixes it outright.
//!
//! `trace.jsonl` itself is append-only. Within ONE process the live
//! [`crate::BundleAgentTraceSink`] serializes same-bundle appends under a
//! per-bundle lock (see its module docs), so concurrent root + subagent
//! records produce clean, parseable lines. That lock does NOT cross processes:
//! if a future slice lets `slab-runtime` write into the same bundle from a
//! second process, `trace.jsonl` cross-process append ordering would need care
//! (file locking or per-process files) — a raw `TraceWriter` opened directly
//! (without the sink) is only atomic to the extent a single OS append of one
//! buffered line is, so direct multi-writer use without the sink's lock is NOT
//! guaranteed line-atomic. Payload uniqueness is already process-independent.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Logical name of the top-level directory holding all agent trace bundles
/// (relative to the slab logs dir).
pub const AGENT_TRACE_DIR_NAME: &str = "agent_trace";

/// File names inside a bundle.
pub const MANIFEST_FILE: &str = "manifest.json";
pub const TRACE_FILE: &str = "trace.jsonl";
pub const PAYLOADS_DIR: &str = "payloads";
/// Reserved for the Slice 10 reducer cache. Not written in Slice 9.
pub const STATE_FILE: &str = "state.json";

/// Bundle manifest format version. Bumped only on breaking layout changes.
pub const BUNDLE_FORMAT_VERSION: u32 = 1;

/// Top-level directory holding all agent trace bundles:
/// `<logs_dir>/agent_trace`.
pub fn agent_trace_root() -> PathBuf {
    slab_utils::app_home::logs_dir().join(AGENT_TRACE_DIR_NAME)
}

/// Sanitize a trace id for use in a directory segment.
pub fn sanitize_trace_segment(value: &str) -> String {
    let mut safe = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }
    let safe = safe.trim_matches('_');
    if safe.is_empty() { "unknown".to_owned() } else { safe.to_owned() }
}

/// Per-root-thread bundle directory name: `trace-<trace_id>-<root_thread_id>`.
pub fn bundle_dir_name(trace_id: &str, root_thread_id: &str) -> String {
    format!("trace-{}-{}", sanitize_trace_segment(trace_id), sanitize_trace_segment(root_thread_id))
}

/// Deterministic per-root-thread bundle directory under a configured trace dir:
/// `<trace_dir>/agent_trace/trace-<root_thread_id>-<root_thread_id>`.
///
/// The `trace_id` is derived from the `root_thread_id` (rather than a random
/// uuid) so the path is DETERMINISTIC: the live [`crate::BundleAgentTraceSink`]
/// and the rollout `build_session_meta` (in `slab-app-core`) use this EXACT
/// formula, so the directory the sink writes into is identical to the path
/// stamped onto the root thread's `SessionMeta.trace_path`. `trace_dir` is the
/// configured agent-trace base directory (the legacy log dir, NOT yet joined
/// with `agent_trace`).
pub fn bundle_dir_for_root_thread(trace_dir: &Path, root_thread_id: &str) -> PathBuf {
    trace_dir.join(AGENT_TRACE_DIR_NAME).join(bundle_dir_name(root_thread_id, root_thread_id))
}

/// Manifest written once when a bundle is created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleManifest {
    pub trace_id: String,
    pub root_thread_id: String,
    pub created_at: String,
    /// Optional pointer to the Part-1 rollout JSONL file for this thread. The
    /// rollout file links back the other way via `SessionMeta.trace_path`
    /// (wired in Slice 11).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout_path: Option<String>,
    pub format_version: u32,
}

/// Inputs needed to start a trace bundle. Kept as a struct so
/// [`start_root_or_disabled`] stays a thin gate.
#[derive(Debug, Clone)]
pub struct BundleStart {
    pub root_thread_id: String,
    pub trace_id: String,
    pub rollout_path: Option<String>,
}

impl BundleStart {
    pub fn new(
        root_thread_id: impl Into<String>,
        trace_id: impl Into<String>,
        rollout_path: Option<String>,
    ) -> Self {
        Self { root_thread_id: root_thread_id.into(), trace_id: trace_id.into(), rollout_path }
    }
}

/// A created trace bundle: the directory + the manifest that was written once.
///
/// Cheap to clone/share. The actual event stream is owned by a
/// [`crate::writer::TraceWriter`] opened against [`TraceBundle::dir`].
#[derive(Debug, Clone)]
pub struct TraceBundle {
    dir: PathBuf,
    manifest: BundleManifest,
}

impl TraceBundle {
    /// Create the bundle directory and write `manifest.json` exactly once.
    /// `payloads/` is also created eagerly so the writer can drop payload files
    /// without an extra `create_dir_all` per write.
    pub fn create(
        root_thread_id: impl Into<String>,
        trace_id: impl Into<String>,
        rollout_path: Option<String>,
    ) -> std::io::Result<Self> {
        Self::create_at(
            agent_trace_root(),
            BundleStart::new(root_thread_id, trace_id, rollout_path),
        )
    }

    /// Same as [`TraceBundle::create`] but rooted at a caller-supplied
    /// directory (used by tests and explicit log-dir configuration).
    ///
    /// The manifest is written ONCE per bundle. If a manifest already exists
    /// for this `trace_id` + `root_thread_id` pair (the bundle directory was
    /// created earlier), the EXISTING manifest is preserved — its `created_at`
    /// and `rollout_path` are NOT overwritten. Reusing a `trace_id` +
    /// `root_thread_id` pair therefore never rewrites history; the original
    /// creation metadata wins.
    pub fn create_at(root: PathBuf, start: BundleStart) -> std::io::Result<Self> {
        let dir = root.join(bundle_dir_name(&start.trace_id, &start.root_thread_id));
        std::fs::create_dir_all(&dir)?;
        std::fs::create_dir_all(dir.join(PAYLOADS_DIR))?;

        let manifest_path = dir.join(MANIFEST_FILE);

        // If a manifest already exists, the bundle was created earlier —
        // preserve the original (including created_at) instead of rewriting it.
        if manifest_path.exists()
            && let Ok(existing) = std::fs::read_to_string(&manifest_path)
            && let Ok(manifest) = serde_json::from_str::<BundleManifest>(existing.trim())
        {
            return Ok(TraceBundle { dir, manifest });
        }
        // No manifest, or it was missing/unreadable/corrupt: fall through and
        // write a fresh one so the bundle self-heals.

        let manifest = BundleManifest {
            trace_id: start.trace_id,
            root_thread_id: start.root_thread_id,
            created_at: Utc::now().to_rfc3339(),
            rollout_path: start.rollout_path,
            format_version: BUNDLE_FORMAT_VERSION,
        };

        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&manifest_path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, &manifest)?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        Ok(TraceBundle { dir, manifest })
    }

    /// Bundle directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The manifest written at creation.
    pub fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    /// `trace.jsonl` path inside the bundle.
    pub fn trace_path(&self) -> PathBuf {
        self.dir.join(TRACE_FILE)
    }

    /// `payloads/` directory inside the bundle.
    pub fn payloads_dir(&self) -> PathBuf {
        self.dir.join(PAYLOADS_DIR)
    }

    /// Reserved `state.json` path (Slice 10 reducer cache). Not written here.
    pub fn state_path(&self) -> PathBuf {
        self.dir.join(STATE_FILE)
    }
}

/// Cheap handle for a thread writing into an existing bundle. Carries just
/// what the [`crate::writer::TraceWriter`] needs to stamp events.
#[derive(Debug, Clone)]
pub struct ThreadTraceContext {
    pub bundle_dir: PathBuf,
    pub thread_id: String,
    pub turn_index: Option<u32>,
    pub parent_span_id: Option<String>,
}

impl ThreadTraceContext {
    pub fn new(
        bundle_dir: impl Into<PathBuf>,
        thread_id: impl Into<String>,
        turn_index: Option<u32>,
        parent_span_id: Option<String>,
    ) -> Self {
        Self {
            bundle_dir: bundle_dir.into(),
            thread_id: thread_id.into(),
            turn_index,
            parent_span_id,
        }
    }

    /// Build a handle from a bundle + an [`crate::AgentTraceContext`].
    pub fn from_bundle(bundle: &TraceBundle, ctx: &crate::AgentTraceContext) -> Self {
        Self::new(
            bundle.dir(),
            ctx.thread_id.clone().unwrap_or_else(|| ctx.session_id.clone()),
            ctx.turn_index,
            ctx.parent_span_id.clone(),
        )
    }

    pub fn with_turn(mut self, turn_index: u32) -> Self {
        self.turn_index = Some(turn_index);
        self
    }
}

/// Gate that returns a [`TraceBundle`] when tracing is enabled, or `None` when
/// disabled (caller falls back to [`crate::NoopAgentTraceSink`] as today).
///
/// Errors creating the bundle are logged and treated as "disabled" so a
/// trace-directory failure never blocks agent execution.
pub fn start_root_or_disabled(start: BundleStart, enabled: bool) -> Option<TraceBundle> {
    start_root_or_disabled_at(agent_trace_root(), start, enabled)
}

/// Same as [`start_root_or_disabled`] but rooted at a caller-supplied directory
/// (used by tests so they do not touch the real app home `logs_dir()`).
pub fn start_root_or_disabled_at(
    root: PathBuf,
    start: BundleStart,
    enabled: bool,
) -> Option<TraceBundle> {
    if !enabled {
        return None;
    }
    match TraceBundle::create_at(root, start) {
        Ok(bundle) => Some(bundle),
        Err(error) => {
            tracing::warn!(error = %error, "failed to create agent trace bundle; tracing disabled");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_writes_manifest_once_with_five_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().to_path_buf();
        let bundle = TraceBundle::create_at(
            root.clone(),
            BundleStart::new("root-thread-1", "trace-uuid-1", Some("/data/x.rollout.jsonl".into())),
        )
        .expect("create bundle");

        // manifest.json exists at the expected path.
        let manifest_path = bundle.dir().join(MANIFEST_FILE);
        let content = std::fs::read_to_string(&manifest_path).expect("read manifest");
        let parsed: serde_json::Value =
            serde_json::from_str(content.trim()).expect("manifest is JSON");

        assert_eq!(parsed["trace_id"], "trace-uuid-1");
        assert_eq!(parsed["root_thread_id"], "root-thread-1");
        assert_eq!(parsed["rollout_path"], "/data/x.rollout.jsonl");
        assert_eq!(parsed["format_version"], BUNDLE_FORMAT_VERSION);
        assert!(parsed["created_at"].is_string());

        // 5 fields (created_at, format_version, rollout_path, root_thread_id, trace_id).
        let object = parsed.as_object().expect("object");
        assert_eq!(object.len(), 5, "manifest has exactly 5 fields: {object:?}");

        // payloads/ created eagerly.
        assert!(bundle.payloads_dir().is_dir());

        let original_created_at = parsed["created_at"].as_str().expect("str").to_owned();

        // Re-creating with the SAME trace_id + root_thread_id (but a different
        // rollout_path) must PRESERVE the original manifest: the manifest is
        // written ONCE. created_at is unchanged, and the original rollout_path
        // wins over the new (None) one.
        let again =
            TraceBundle::create_at(root, BundleStart::new("root-thread-1", "trace-uuid-1", None))
                .expect("recreate");
        let reread: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(again.dir().join(MANIFEST_FILE)).expect("read").trim(),
        )
        .expect("manifest is JSON");
        assert_eq!(
            reread["created_at"].as_str().unwrap(),
            original_created_at,
            "created_at preserved across re-create (manifest written once)",
        );
        assert_eq!(
            reread["rollout_path"], "/data/x.rollout.jsonl",
            "original rollout_path preserved (re-create does not rewrite manifest)",
        );
        // And still exactly one JSON document on disk.
        let lines = std::fs::read_to_string(again.dir().join(MANIFEST_FILE))
            .expect("read")
            .trim()
            .lines()
            .count();
        assert_eq!(lines, 1, "manifest is one JSON document");
    }

    #[test]
    fn bundle_dir_name_sanitizes_segments() {
        // Non-alphanumeric chars become `_`, then leading/trailing `_` are trimmed.
        assert_eq!(bundle_dir_name("abc/..", "rt"), "trace-abc-rt");
        assert_eq!(bundle_dir_name("uuid-1", "rt-1"), "trace-uuid-1-rt-1");
        assert_eq!(bundle_dir_name("a b", "c d"), "trace-a_b-c_d");
    }

    #[test]
    fn bundle_dir_for_root_thread_is_deterministic() {
        // trace_id is derived from root_thread_id, so the same root always maps
        // to the same dir under <trace_dir>/agent_trace.
        let dir = bundle_dir_for_root_thread(Path::new("/logs"), "rt-1");
        assert_eq!(dir, Path::new("/logs").join(AGENT_TRACE_DIR_NAME).join("trace-rt-1-rt-1"));
        // Idempotent: same root → same dir.
        assert_eq!(dir, bundle_dir_for_root_thread(Path::new("/logs"), "rt-1"));
        // Different root → different dir.
        assert_ne!(dir, bundle_dir_for_root_thread(Path::new("/logs"), "rt-2"));
    }

    #[test]
    fn start_root_or_disabled_returns_none_when_disabled() {
        let start = BundleStart::new("rt", "tid", None);
        assert!(start_root_or_disabled(start.clone(), false).is_none());
    }

    #[test]
    fn start_root_or_disabled_creates_bundle_when_enabled() {
        // Route through the *_at variant rooted at a tempfile so the test never
        // touches the real app home logs_dir().
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().to_path_buf();
        let start = BundleStart::new("rt", "tid", None);
        let bundle =
            start_root_or_disabled_at(root.clone(), start, true).expect("enabled -> bundle");

        // Manifest exists UNDER the temp root, not the real app home.
        let manifest_in_temp = bundle.dir().join(MANIFEST_FILE);
        assert!(
            manifest_in_temp.starts_with(&root),
            "bundle must live under the temp root, got {}",
            manifest_in_temp.display(),
        );
        assert!(manifest_in_temp.is_file(), "manifest written at temp root");

        // And NOT in the real agent_trace_root.
        let real_root = agent_trace_root();
        assert!(
            !manifest_in_temp.starts_with(&real_root),
            "test leaked a bundle into the real app home {}",
            real_root.display(),
        );
    }

    #[test]
    fn trace_bundle_paths_are_inside_dir() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle =
            TraceBundle::create_at(temp.path().to_path_buf(), BundleStart::new("rt", "tid", None))
                .expect("create");
        assert_eq!(bundle.trace_path(), bundle.dir().join(TRACE_FILE));
        assert_eq!(bundle.payloads_dir(), bundle.dir().join(PAYLOADS_DIR));
        assert_eq!(bundle.state_path(), bundle.dir().join(STATE_FILE));
    }
}
