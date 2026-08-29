//! Run-scoped guard bounding every tool result before it enters the
//! conversation — the last-resort net behind the per-tool byte caps.
//!
//! Built-in tools (grep/shell/read_file/...) already emit bounded payloads;
//! this net exists for tools the loop does not control (MCP proxies, plugin
//! adapters) whose oversized outputs would otherwise be re-sent to the model
//! on every subsequent turn. Every transformation carries an explicit marker
//! — nothing is dropped silently.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Mutex,
};

use slab_utils::string::truncate_middle_bytes;

/// Hard per-result cap at the dispatch choke point.
const MAX_TOOL_RESULT_BYTES: usize = 64 * 1024;
/// Head fraction of the kept budget when the net truncates.
const NET_HEAD_RATIO: f32 = 0.7;
/// Results below this size are never deduplicated.
const DEDUP_MIN_BYTES: usize = 2048;

pub(crate) struct BoundedToolResult {
    pub content: String,
    pub original_bytes: usize,
    pub truncated: bool,
    pub duplicate_of: Option<String>,
}

/// Bounds tool results for one agent run. Tool calls within a batch execute
/// concurrently, so the dedup map is mutex-guarded; the guard itself borrows
/// nothing from the message history.
#[derive(Default)]
pub(crate) struct ToolResultGuard {
    /// (hash, byte length) -> call_id of the first occurrence. The length is
    /// checked alongside the 64-bit hash so a plain hash collision cannot
    /// falsely deduplicate distinct results.
    seen: Mutex<HashMap<u64, (String, usize)>>,
}

impl ToolResultGuard {
    /// Bound one tool result: exact-duplicate large outputs collapse to an
    /// explicit reference to their first occurrence, then anything still over
    /// the byte cap is middle-truncated with an omission marker.
    pub(crate) fn bound(&self, call_id: &str, content: &str) -> BoundedToolResult {
        if content.len() >= DEDUP_MIN_BYTES {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            let key = hasher.finish();
            let mut seen = self.seen.lock().expect("tool result guard lock poisoned");
            if let Some((first, len)) = seen.get(&key)
                && *len == content.len()
            {
                let note = format!(
                    "[duplicate of tool result {first}: {} bytes identical, omitted]",
                    content.len()
                );
                return BoundedToolResult {
                    content: note,
                    original_bytes: content.len(),
                    truncated: false,
                    duplicate_of: Some(first.clone()),
                };
            }
            seen.insert(key, (call_id.to_owned(), content.len()));
        }

        let original_bytes = content.len();
        let (bounded, omitted) =
            truncate_middle_bytes(content, MAX_TOOL_RESULT_BYTES, NET_HEAD_RATIO);
        BoundedToolResult {
            content: bounded,
            original_bytes,
            truncated: omitted > 0,
            duplicate_of: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_guard_bounds_oversized_output_with_marker() {
        let guard = ToolResultGuard::default();
        let payload = format!("head {}", "m".repeat(200 * 1024)); // 200KB
        let result = guard.bound("call-1", &payload);

        assert!(result.truncated);
        assert_eq!(result.original_bytes, payload.len());
        assert!(result.content.len() < MAX_TOOL_RESULT_BYTES + 128);
        assert!(result.content.starts_with("head "), "head must survive");
        assert!(result.content.contains("bytes omitted"), "marker missing");
        assert!(result.duplicate_of.is_none());
    }

    #[test]
    fn tool_result_guard_dedups_identical_large_results_across_calls() {
        let guard = ToolResultGuard::default();
        let payload = "d".repeat(5 * 1024); // above DEDUP_MIN_BYTES

        let first = guard.bound("call-1", &payload);
        assert!(!first.truncated);
        assert_eq!(first.content, payload); // first occurrence kept verbatim
        assert!(first.duplicate_of.is_none());

        let second = guard.bound("call-2", &payload);
        assert_eq!(
            second.content,
            "[duplicate of tool result call-1: 5120 bytes identical, omitted]"
        );
        assert_eq!(second.duplicate_of.as_deref(), Some("call-1"));
        assert_eq!(second.original_bytes, payload.len());
    }

    #[test]
    fn tool_result_guard_leaves_small_outputs_verbatim() {
        let guard = ToolResultGuard::default();
        let small = "{\"ok\": true}";
        let result = guard.bound("call-1", small);
        assert_eq!(result.content, small);
        assert!(!result.truncated);
        assert_eq!(result.original_bytes, small.len());

        // Below the dedup floor even exact repeats stay verbatim.
        let repeat = guard.bound("call-2", small);
        assert_eq!(repeat.content, small);
        assert!(repeat.duplicate_of.is_none());
    }

    #[test]
    fn tool_result_guard_never_dedups_distinct_results_with_same_length() {
        let guard = ToolResultGuard::default();
        let a = format!("{}A", "x".repeat(4 * 1024));
        let b = format!("{}B", "x".repeat(4 * 1024));

        let first = guard.bound("call-1", &a);
        let second = guard.bound("call-2", &b);
        assert!(first.duplicate_of.is_none());
        assert!(second.duplicate_of.is_none(), "same length must not dedup");
        assert_eq!(second.content, b);
    }
}
