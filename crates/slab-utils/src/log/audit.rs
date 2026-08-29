//! Self-contained, rotating JSON audit log for the sandbox subsystem.
//!
//! Writes one structured line per sandbox event to `{app_home}/logs/slab-sandbox.log`
//! via the shared [`SizeRotatingLogFile`] (so it rotates and redacts the same way
//! as the main logs). Self-contained on purpose: the elevated sandbox daemon runs
//! without a host tracing subscriber, so this writer must not depend on one.

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::app_home::sandbox_log_file;
use crate::log::redaction::redact_log_text;
use crate::log::rotating::{DEFAULT_MAX_LOG_BYTES, DEFAULT_MAX_LOG_FILES, SizeRotatingLogFile};

/// Cap raw `args` length so a single audit line can never grow unbounded.
const MAX_ARG_CHARS: usize = 256;

/// What kind of sandbox event this record describes.
#[derive(Copy, Clone, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum AuditKind {
    /// A spawn policy was resolved (allow/deny decision reached).
    SpawnDecided,
    /// A sandboxed child process was spawned.
    Spawned,
    /// A sandboxed child process exited normally.
    Exited,
    /// A sandboxed child process was killed (e.g. timeout).
    Killed,
    /// The sandbox environment was provisioned (ACLs/WFP/seccomp applied).
    Provisioned,
    /// Spawning a sandboxed child failed.
    SpawnFailed,
    /// The daemon lost its connection to a sandbox client.
    DaemonConnectionFailed,
    /// The daemon shut down because its owner process (slab-server) exited.
    DaemonOwnerExited,
}

/// Outcome of a sandbox decision.
#[derive(Copy, Clone, Debug, Serialize)]
pub enum AuditDecision {
    Allow,
    Deny,
}

/// A single sandbox audit record. Build with [`SandboxAudit::new`] and the field
/// setters, then call [`.record()`](SandboxAudit::record) to append it.
#[derive(Serialize)]
pub struct SandboxAudit {
    kind: AuditKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<AuditDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    program: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    module: &'static str,
}

impl SandboxAudit {
    pub fn new(kind: AuditKind, module: &'static str) -> Self {
        Self {
            kind,
            module,
            decision: None,
            tier: None,
            program: None,
            args: None,
            exit_code: None,
            error: None,
        }
    }

    pub fn decision(mut self, decision: AuditDecision) -> Self {
        self.decision = Some(decision);
        self
    }

    /// Short, stable isolation-tier tag (e.g. `"AclTokenWfp"`, `"SeccompLandlock"`,
    /// `"Passthrough"`).
    pub fn tier(mut self, tier: &'static str) -> Self {
        self.tier = Some(tier);
        self
    }

    pub fn program(mut self, program: impl Into<String>) -> Self {
        self.program = Some(program.into());
        self
    }

    /// Raw argument blob; truncated to [`MAX_ARG_CHARS`] chars to bound log size.
    pub fn args(mut self, args: impl Into<String>) -> Self {
        let mut value = args.into();
        if value.chars().count() > MAX_ARG_CHARS {
            value = format!("{}…", value.chars().take(MAX_ARG_CHARS).collect::<String>());
        }
        self.args = Some(value);
        self
    }

    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = Some(exit_code);
        self
    }

    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Append this record to `{app_home}/logs/slab-sandbox.log`. Best-effort:
    /// I/O or serialization failures are swallowed so the sandbox spawn path can
    /// never panic on logging.
    pub fn record(self) {
        let Some(writer) = shared_writer() else { return };
        if let Ok(mut guard) = writer.lock() {
            write_record(&mut guard, &self);
        }
    }
}

/// Render `record` to a redacted JSON line and append it to `writer`.
fn write_record(writer: &mut SizeRotatingLogFile, record: &SandboxAudit) {
    let Some(line) = render_line(record) else { return };
    let _ = writer.write_redacted(line.as_bytes());
    let _ = writer.flush();
}

/// Serialize the record to a JSON object, stamp it with `ts_ms`, run it through
/// secret redaction, and append a trailing newline. Returns `None` on a
/// serialization failure (should be impossible for these field types).
fn render_line(record: &SandboxAudit) -> Option<String> {
    let mut value = serde_json::to_value(record).ok()?;
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    if let Some(object) = value.as_object_mut() {
        object.insert("ts_ms".to_string(), serde_json::Value::from(ts_ms));
    }
    let mut line = serde_json::to_string(&value).ok()?;
    line.push('\n');
    Some(redact_log_text(&line))
}

/// Lazily open the shared rotating audit file. Returns `None` if the file cannot
/// be opened (e.g. read-only home); callers silently skip the record.
fn shared_writer() -> Option<&'static Mutex<SizeRotatingLogFile>> {
    static LOG: OnceLock<Option<Mutex<SizeRotatingLogFile>>> = OnceLock::new();
    LOG.get_or_init(|| {
        SizeRotatingLogFile::new(sandbox_log_file(), DEFAULT_MAX_LOG_BYTES, DEFAULT_MAX_LOG_FILES)
            .ok()
            .map(Mutex::new)
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_json_line_with_kind_and_redaction() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut file =
            SizeRotatingLogFile::new(temp.path().join("slab-sandbox.log"), 1024, 5).expect("log");
        let record = SandboxAudit::new(AuditKind::Spawned, "slab-sandboxing::driver")
            .decision(AuditDecision::Allow)
            .tier("AclTokenWfp")
            .program("cmd.exe")
            .args("/c echo token=secret-value");

        write_record(&mut file, &record);

        let output = std::fs::read_to_string(temp.path().join("slab-sandbox.log")).expect("read");
        assert!(output.contains("\"kind\":\"Spawned\""), "{output}");
        assert!(output.contains("\"decision\":\"Allow\""), "{output}");
        assert!(output.contains("\"tier\":\"AclTokenWfp\""), "{output}");
        assert!(output.contains("\"program\":\"cmd.exe\""), "{output}");
        assert!(output.contains("\"module\":\"slab-sandboxing::driver\""), "{output}");
        assert!(output.contains("\"ts_ms\":"), "{output}");
        assert!(output.contains("token=<redacted>"), "{output}");
        assert!(!output.contains("secret-value"), "secret leaked: {output}");
    }

    #[test]
    fn truncates_long_args() {
        let record =
            SandboxAudit::new(AuditKind::Spawned, "m").args("x".repeat(MAX_ARG_CHARS + 100));
        let value = serde_json::to_value(&record).expect("serialize");
        let args = value.get("args").and_then(|v| v.as_str()).expect("args");
        // truncated body (MAX_ARG_CHARS) + ellipsis
        assert_eq!(args.chars().count(), MAX_ARG_CHARS + 1);
    }
}
