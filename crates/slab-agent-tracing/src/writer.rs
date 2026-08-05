//! `TraceWriter` — appends events to a bundle's `trace.jsonl` with the
//! payload-before-event invariant.
//!
//! Invariant: a payload file under `payloads/` is ALWAYS flushed BEFORE the
//! event referencing it is appended to `trace.jsonl`.
//! [`TraceWriter::write_json_payload`] writes and flushes the payload file
//! before returning the [`RawPayloadRef`]; only then does the caller hand the
//! ref to [`TraceWriter::append`]. A reducer replaying the bundle can therefore
//! assume any referenced payload exists.
//!
//! ## Uniqueness of payload ids (no collision across writers / turns / processes)
//!
//! Each payload id is a fresh uuid v4 (`p-<uuid>`). Because the id is globally
//! unique, two writers sharing one bundle (a root thread + a subagent thread,
//! or a fresh writer opened per turn — the [`TraceWriter::for_thread`] pattern)
//! can NEVER collide on a payload filename, so a later write can NEVER
//! truncate an earlier payload. This replaces the per-instance ordinal counter
//! that reset to 0 on every `TraceWriter::open` and silently overwrote earlier
//! payloads.
//!
//! ## Flush vs fsync (durability)
//!
//! The payload file is flushed (buffer drained to the OS page cache) before the
//! ref is returned, and `trace.jsonl` appends are flushed in [`TraceWriter::append`].
//! This is sufficient for the payload-before-event *exists-invariant* and for
//! diagnostic replay within a running machine. It is NOT an `fsync`: a
//! power-loss crash may lose the tail of recently-written data. True
//! power-loss durability is intentionally not provided — the bundle is a
//! diagnostic artifact, not a transactional store.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use serde_json::Value;

use crate::bundle::ThreadTraceContext;
use crate::bundle::{PAYLOADS_DIR, TRACE_FILE};
use crate::event::{RawPayloadKind, RawPayloadRef, RawTraceEvent, RawTraceEventPayload};

/// Owns the bundle directory for writing: opens `trace.jsonl` for append and
/// hands out flushed payload references.
///
/// One `TraceWriter` per thread writing into a bundle. The payload-before-event
/// invariant is enforced by ordering inside `write_json_payload` — the payload
/// is written and flushed before the ref is returned, and `append` only writes
/// the event line afterwards (after also defensively dropping any ref whose
/// payload file is missing).
///
/// See the module docs for the full invariant statement and the uniqueness
/// guarantee on payload ids.
pub struct TraceWriter {
    bundle_dir: PathBuf,
    payloads_dir: PathBuf,
    thread_id: Option<String>,
    turn_index: Option<u32>,
    parent_span_id: Option<String>,
    trace_writer: Mutex<BufWriter<File>>,
}

impl TraceWriter {
    /// Open (creating if absent) `trace.jsonl` for a thread writing into an
    /// existing bundle described by [`ThreadTraceContext`].
    ///
    /// This is the documented fresh-writer-per-turn / per-thread pattern: each
    /// call reopens the same `trace.jsonl` in append mode (cheap) and stamps a
    /// new `thread_id` / `turn_index`. Because payload ids are uuid-based (not a
    /// per-instance counter), opening a fresh writer per turn or per subagent
    /// thread is safe — payload files are NEVER overwritten.
    pub fn for_thread(ctx: &ThreadTraceContext) -> std::io::Result<Self> {
        Self::open(
            ctx.bundle_dir.clone(),
            Some(ctx.thread_id.clone()),
            ctx.turn_index,
            ctx.parent_span_id.clone(),
        )
    }

    /// Open a writer with explicit fields. `for_thread` is the usual entry point.
    pub fn open(
        bundle_dir: PathBuf,
        thread_id: Option<String>,
        turn_index: Option<u32>,
        parent_span_id: Option<String>,
    ) -> std::io::Result<Self> {
        let payloads_dir = bundle_dir.join(PAYLOADS_DIR);
        std::fs::create_dir_all(&payloads_dir)?;
        std::fs::create_dir_all(&bundle_dir)?;

        let trace_path = bundle_dir.join(TRACE_FILE);
        let file = OpenOptions::new().create(true).append(true).open(&trace_path)?;
        Ok(Self {
            bundle_dir,
            payloads_dir,
            thread_id,
            turn_index,
            parent_span_id,
            trace_writer: Mutex::new(BufWriter::new(file)),
        })
    }

    /// Bundle directory this writer owns.
    pub fn bundle_dir(&self) -> &std::path::Path {
        &self.bundle_dir
    }

    /// Turn index currently stamped onto appended events. Callers that need to
    /// advance it open a fresh writer per turn (cheap — reopens the same
    /// `trace.jsonl` in append mode, and payload ids stay unique because they
    /// are uuid-based). A mutable setter is not yet provided.
    pub fn turn_index(&self) -> Option<u32> {
        self.turn_index
    }

    /// Write a payload to `payloads/<raw_payload_id>.json` FIRST (create new,
    /// write, flush), THEN return a flushed reference. The returned
    /// [`RawPayloadRef`] can be embedded in an event passed to [`Self::append`].
    ///
    /// This is the heart of the payload-before-event invariant: the payload
    /// file is on disk (flushed to the OS page cache) before the caller can
    /// append any event that references it.
    ///
    /// The id is a fresh uuid v4 (`p-<uuid>`), so it is unique across all
    /// writers, turns, and processes writing into the same bundle. The file is
    /// opened with `create_new` so an (astronomically unlikely) uuid collision
    /// fails loud instead of silently truncating an earlier payload.
    pub fn write_json_payload(
        &self,
        kind: RawPayloadKind,
        value: &Value,
    ) -> std::io::Result<RawPayloadRef> {
        let raw_payload_id = format!("p-{}", uuid::Uuid::new_v4());
        let file_name = format!("{raw_payload_id}.json");
        let abs_path = self.payloads_dir.join(&file_name);

        let file = OpenOptions::new().write(true).create_new(true).open(&abs_path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        // Path stored relative to the bundle dir so bundles stay relocatable.
        let rel_path = PathBuf::from(PAYLOADS_DIR).join(file_name);
        Ok(RawPayloadRef { raw_payload_id, kind, path: rel_path })
    }

    /// Append a typed event to `trace.jsonl`. The event MAY reference payload
    /// refs returned by [`Self::write_json_payload`]; those payload files are
    /// already flushed by construction.
    ///
    /// Defensive invariant enforcement: before serializing, every ref attached
    /// to the event is checked for existence. A ref whose payload file is
    /// missing (a caller fabricated a [`RawPayloadRef`], or wrote out of order)
    /// is logged at `error!` and dropped from the line written to
    /// `trace.jsonl`, so a dangling reference can never reach a reducer. The
    /// rest of the event is still recorded.
    pub fn append(&self, mut event: RawTraceEventPayload) -> std::io::Result<()> {
        event.retain_existing_payload_refs(|reff| {
            let abs = self.bundle_dir.join(&reff.path);
            if abs.is_file() {
                true
            } else {
                tracing::error!(
                    raw_payload_id = %reff.raw_payload_id,
                    path = %reff.path.display(),
                    "payload referenced by trace event does not exist; \
                     dropping dangling ref from trace.jsonl (payload-before-event invariant)",
                );
                false
            }
        });

        let line = RawTraceEvent::new(
            Utc::now().to_rfc3339(),
            self.thread_id.clone(),
            self.turn_index,
            self.parent_span_id.clone(),
            event,
        );
        let json = serde_json::to_string(&line)?;
        let mut guard = self.trace_writer.lock().expect("trace writer lock poisoned");
        guard.write_all(json.as_bytes())?;
        guard.write_all(b"\n")?;
        guard.flush()?;
        Ok(())
    }

    /// Flush the underlying trace.jsonl buffer.
    pub fn flush(&self) -> std::io::Result<()> {
        let mut guard = self.trace_writer.lock().expect("trace writer lock poisoned");
        guard.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{BundleStart, TraceBundle};
    use serde_json::json;

    fn make_bundle(temp: &tempfile::TempDir) -> TraceBundle {
        TraceBundle::create_at(
            temp.path().to_path_buf(),
            BundleStart::new("root-thread-1", "trace-uuid-1", None),
        )
        .expect("create bundle")
    }

    #[test]
    fn payload_before_event_invariant_holds() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let ctx = ThreadTraceContext::new(bundle.dir(), "root-thread-1", Some(2), None);
        let writer = TraceWriter::for_thread(&ctx).expect("open writer");

        // Write two payloads, then append events referencing them.
        let req_ref = writer
            .write_json_payload(RawPayloadKind::Request, &json!({ "prompt": "hi" }))
            .expect("write request payload");
        let other_ref = writer
            .write_json_payload(RawPayloadKind::Other, &json!({ "x": 1 }))
            .expect("write other payload");

        writer
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(req_ref.clone()),
            })
            .expect("append inference started");
        writer
            .append(RawTraceEventPayload::Other {
                source: "slab-agent".into(),
                event: "custom".into(),
                payload: Some(other_ref.clone()),
            })
            .expect("append other");
        writer.append(RawTraceEventPayload::TurnCompleted).expect("append turn completed");
        writer.flush().expect("flush");

        // Read trace.jsonl back, parse each event, and assert every referenced
        // payload file exists on disk.
        let trace_contents =
            std::fs::read_to_string(bundle.trace_path()).expect("read trace.jsonl");
        let lines: Vec<&str> = trace_contents.trim().lines().collect();
        assert_eq!(lines.len(), 3, "one line per append");

        for line in &lines {
            let event: RawTraceEvent =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("parse line: {e}: {line}"));
            for reff in event.event.payload_refs() {
                let abs = bundle.dir().join(&reff.path);
                assert!(abs.is_file(), "referenced payload missing: {}", abs.display());
            }
        }

        // The two payload files exist.
        assert!(bundle.dir().join(&req_ref.path).is_file());
        assert!(bundle.dir().join(&other_ref.path).is_file());
    }

    #[test]
    fn write_json_payload_flushes_before_returning_ref() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let ctx = ThreadTraceContext::new(bundle.dir(), "t", None, None);
        let writer = TraceWriter::for_thread(&ctx).expect("open writer");

        let reff =
            writer.write_json_payload(RawPayloadKind::Response, &json!({ "r": 1 })).expect("write");

        // Without ever calling append, the payload file is already flushed.
        let abs = bundle.dir().join(&reff.path);
        assert!(abs.is_file(), "payload flushed before any event");
        let body = std::fs::read_to_string(&abs).expect("read payload");
        let parsed: serde_json::Value = serde_json::from_str(body.trim()).expect("json");
        assert_eq!(parsed["r"], 1);
        assert!(reff.path.starts_with(PAYLOADS_DIR));
        assert!(reff.raw_payload_id.starts_with("p-"));
        assert!(reff.raw_payload_id.len() > "p-".len(), "uuid suffix present");
    }

    #[test]
    fn append_serializes_tagged_event() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let ctx = ThreadTraceContext::new(bundle.dir(), "t", None, None);
        let writer = TraceWriter::for_thread(&ctx).expect("open writer");

        writer.append(RawTraceEventPayload::TurnStarted).expect("append");
        writer.flush().expect("flush");

        let line = std::fs::read_to_string(bundle.trace_path()).expect("read").trim().to_owned();
        assert!(line.contains("\"kind\":\"turn_started\""), "{line}");
    }

    // ---- I1 regression tests: payload ids must be unique across writers/turns ----

    /// Helper: read trace.jsonl into parsed events.
    fn read_events(bundle: &TraceBundle) -> Vec<RawTraceEvent> {
        let contents = std::fs::read_to_string(bundle.trace_path()).expect("read trace.jsonl");
        contents
            .trim()
            .lines()
            .map(|line| serde_json::from_str(line).expect("parse line"))
            .collect()
    }

    /// Read the payload JSON a ref points at, parsed.
    fn read_payload(bundle: &TraceBundle, reff: &RawPayloadRef) -> serde_json::Value {
        let body = std::fs::read_to_string(bundle.dir().join(&reff.path)).expect("read payload");
        serde_json::from_str(body.trim()).expect("parse payload")
    }

    #[test]
    fn two_writers_one_bundle_never_collide_on_payload_ids() {
        // Two writers on the SAME bundle dir (root + subagent, both via
        // ThreadTraceContext). Pre-fix (per-instance ordinal counter) both
        // emitted p000000 and the second truncated the first's content.
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);

        let root_ctx = ThreadTraceContext::new(bundle.dir(), "root-thread-1", Some(0), None);
        let sub_ctx =
            ThreadTraceContext::new(bundle.dir(), "subagent-1", Some(0), Some("span-root".into()));

        let root_writer = TraceWriter::for_thread(&root_ctx).expect("open root writer");
        let sub_writer = TraceWriter::for_thread(&sub_ctx).expect("open sub writer");

        let payload_a = root_writer
            .write_json_payload(RawPayloadKind::Request, &json!({ "who": "A" }))
            .expect("write A");
        root_writer
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(payload_a.clone()),
            })
            .expect("append A event");

        let payload_b = sub_writer
            .write_json_payload(RawPayloadKind::Request, &json!({ "who": "B" }))
            .expect("write B");
        sub_writer
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(payload_b.clone()),
            })
            .expect("append B event");

        root_writer.flush().expect("flush root");
        sub_writer.flush().expect("flush sub");

        // (1) ids are distinct.
        assert_ne!(
            payload_a.raw_payload_id, payload_b.raw_payload_id,
            "payload ids must be unique across writers",
        );

        // (2) BOTH payload files exist (no truncation).
        let abs_a = bundle.dir().join(&payload_a.path);
        let abs_b = bundle.dir().join(&payload_b.path);
        assert!(abs_a.is_file(), "payload A still present: {}", abs_a.display());
        assert!(abs_b.is_file(), "payload B present: {}", abs_b.display());

        // (3) Each event replays with ITS OWN content (no cross-contamination).
        let events = read_events(&bundle);
        assert_eq!(events.len(), 2, "two events in trace.jsonl");

        let refs: Vec<&RawPayloadRef> =
            events.iter().flat_map(|e| e.event.payload_refs()).collect();
        assert_eq!(refs.len(), 2);
        for reff in &refs {
            let body = read_payload(&bundle, reff);
            // Each ref's own id must match its file content's identity.
            let id = reff.raw_payload_id.as_str();
            if id == payload_a.raw_payload_id {
                assert_eq!(body["who"], "A", "A ref reads A content");
            } else if id == payload_b.raw_payload_id {
                assert_eq!(body["who"], "B", "B ref reads B content");
            } else {
                panic!("unexpected payload id {id}");
            }
        }
    }

    #[test]
    fn fresh_writer_per_turn_preserves_earlier_payload() {
        // The docstring-blessed pattern: open a fresh TraceWriter per turn.
        // Pre-fix, turn 2's first payload reused p000000 and truncated turn 1's
        // payload. Post-fix, ids are uuid-based so turn-1 content survives.
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);

        // Turn 1.
        let ctx1 = ThreadTraceContext::new(bundle.dir(), "root-thread-1", Some(1), None);
        let writer1 = TraceWriter::for_thread(&ctx1).expect("open turn-1 writer");
        let turn1_ref = writer1
            .write_json_payload(RawPayloadKind::Request, &json!({ "turn": 1 }))
            .expect("write turn-1 payload");
        writer1
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(turn1_ref.clone()),
            })
            .expect("append turn-1 event");
        writer1.flush().expect("flush turn 1");

        // Turn 2 — a FRESH writer on the same bundle.
        let ctx2 = ThreadTraceContext::new(bundle.dir(), "root-thread-1", Some(2), None);
        let writer2 = TraceWriter::for_thread(&ctx2).expect("open turn-2 writer");
        let turn2_ref = writer2
            .write_json_payload(RawPayloadKind::Request, &json!({ "turn": 2 }))
            .expect("write turn-2 payload");
        writer2
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(turn2_ref.clone()),
            })
            .expect("append turn-2 event");
        writer2.flush().expect("flush turn 2");

        // ids distinct + turn-1 content intact after turn-2 writes.
        assert_ne!(turn1_ref.raw_payload_id, turn2_ref.raw_payload_id);
        let body1 = read_payload(&bundle, &turn1_ref);
        assert_eq!(body1["turn"], 1, "turn-1 payload content intact (not overwritten)");
        let body2 = read_payload(&bundle, &turn2_ref);
        assert_eq!(body2["turn"], 2, "turn-2 payload content correct");

        // Both events replay with correct content.
        let events = read_events(&bundle);
        assert_eq!(events.len(), 2);
        let bodies: Vec<serde_json::Value> = events
            .iter()
            .flat_map(|e| e.event.payload_refs())
            .map(|r| read_payload(&bundle, r))
            .collect();
        assert!(bodies.iter().any(|b| b["turn"] == 1), "turn-1 content reachable via replay",);
        assert!(bodies.iter().any(|b| b["turn"] == 2), "turn-2 content reachable via replay",);
    }

    // ---- I3 test: append defensively rejects dangling payload refs ----

    #[test]
    fn append_drops_dangling_payload_ref() {
        let temp = tempfile::tempdir().expect("temp dir");
        let bundle = make_bundle(&temp);
        let ctx = ThreadTraceContext::new(bundle.dir(), "t", None, None);
        let writer = TraceWriter::for_thread(&ctx).expect("open writer");

        // A valid payload + a fabricated (non-existent) one.
        let good = writer
            .write_json_payload(RawPayloadKind::Response, &json!({ "ok": true }))
            .expect("write good");
        let dangling = RawPayloadRef {
            raw_payload_id: "p-fabricated".into(),
            kind: RawPayloadKind::Request,
            path: PathBuf::from("payloads/p-fabricated.json"), // does NOT exist
        };

        writer
            .append(RawTraceEventPayload::InferenceStarted {
                request_payload: Some(dangling.clone()),
            })
            .expect("append with dangling ref");
        writer
            .append(RawTraceEventPayload::InferenceCompleted {
                response_payload: Some(good.clone()),
            })
            .expect("append with valid ref");
        writer.flush().expect("flush");

        let contents = std::fs::read_to_string(bundle.trace_path()).expect("read trace.jsonl");

        // The dangling ref id must NOT appear in trace.jsonl (dropped).
        assert!(
            !contents.contains(&dangling.raw_payload_id),
            "dangling payload id leaked into trace.jsonl: {contents}",
        );
        // The valid ref IS recorded.
        assert!(
            contents.contains(&good.raw_payload_id),
            "valid payload id missing from trace.jsonl: {contents}",
        );

        // The InferenceStarted event still landed (its ref was nulled, not the
        // whole event dropped), so both event kinds are present.
        assert!(contents.contains("\"kind\":\"inference_started\""), "{contents}");
        assert!(contents.contains("\"kind\":\"inference_completed\""), "{contents}");
    }
}
