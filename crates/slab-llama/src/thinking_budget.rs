//! Thinking-budget enforcement for the `<think>` segment.
//!
//! Mirrors llama.cpp upstream `--reasoning-budget N` semantics (the vendored
//! SDK does not ship the `common` C++ library where that lives, so this is the
//! Rust-side equivalent): count generated thinking tokens and, when the budget
//! trips, force-close reasoning by injecting the model's `</think>` tokens so
//! generation continues with the visible answer instead of burning the whole
//! completion budget on thinking.
//!
//! This module is a pure byte-level state machine — no FFI — so the decode
//! loop stays the only place that touches llama.cpp.

/// The reasoning close tag whose tokens get injected when the budget trips.
/// Single-sourced in `slab_utils::thinking_markers`; re-exported so the
/// decode loop's `slab_llama::thinking_budget::THINK_CLOSE_TAG` path is
/// stable.
pub use slab_utils::thinking_markers::THINK_CLOSE_TAG;

/// Attribute-tolerant open marker (`<think>`, `<think\n>`, …), matching the
/// looser `<think` prefix the shared parsers use.
use slab_utils::thinking_markers::THINK_OPEN_MARKER;

/// How many trailing bytes the tracker keeps between ingests. Markers can be
/// split across token pieces (multibyte UTF-8 neighbors make pieces arbitrary
/// byte runs), so detection needs `marker.len() - 1` bytes of history for the
/// close tag plus slack for the `>` that completes an open tag. Bounded so a
/// long generation keeps the tracker O(1) in memory.
const TAIL_LIMIT: usize = 16;

/// Per-generation thinking-budget spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThinkingBudget {
    /// Maximum thinking tokens before the close tag is force-injected.
    pub budget: u32,
    /// The prompt already ended inside an open `<think>` block (Qwen-style
    /// template prefill) — counting starts with the first generated token.
    pub starts_in_thinking: bool,
}

/// What the decode loop should do after ingesting one token's piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingBudgetAction {
    /// Nothing — keep generating (or the tracker is inert for this model).
    None,
    /// The budget tripped on the just-emitted token: inject the model's
    /// `</think>` tokens and continue generation after them.
    InjectClose,
}

/// Byte-level `<think>` segment tracker. One ingest per sampled token piece.
#[derive(Debug)]
pub struct ThinkingBudgetTracker {
    budget_remaining: u32,
    /// A complete `<think …>` open tag has been seen (or was prefilled).
    opened: bool,
    /// `<think` was seen but its closing `>` has not arrived yet.
    open_pending: bool,
    /// A `</think>` close tag has been seen — reasoning ended, stay inert.
    closed: bool,
    /// The budget tripped and `InjectClose` was returned — never fire twice.
    exhausted: bool,
    tail: Vec<u8>,
}

impl ThinkingBudgetTracker {
    pub fn new(spec: ThinkingBudget) -> Self {
        Self {
            budget_remaining: spec.budget,
            // A prefilled prompt already sits inside the think block, so the
            // very first generated token is thinking content.
            opened: spec.starts_in_thinking,
            open_pending: false,
            closed: false,
            exhausted: false,
            tail: Vec::new(),
        }
    }

    /// Ingest one sampled token's detokenized bytes and report the action.
    pub fn ingest_bytes(&mut self, piece: &[u8]) -> ThinkingBudgetAction {
        if self.closed || self.exhausted {
            return ThinkingBudgetAction::None;
        }

        self.tail.extend_from_slice(piece);
        let keep = self.tail.len().saturating_sub(TAIL_LIMIT);
        self.tail.drain(..keep);

        // Close wins over the budget: if this piece completes `</think>` the
        // model ended reasoning on its own — even exactly at the budget
        // boundary — and injecting another close tag would corrupt the stream.
        if find(&self.tail, THINK_CLOSE_TAG).is_some() {
            self.closed = true;
            return ThinkingBudgetAction::None;
        }

        if !self.opened {
            // Open-tag completion: `<think` seen (possibly in an earlier
            // piece) plus the `>` that closes the tag. The pieces making up
            // the open tag itself are not thinking content and are not
            // counted — counting starts with the first piece after the tag.
            // Content before the model opens its think block (or a model that
            // never thinks at all) is likewise not counted: the tracker stays
            // inert until an open tag completes.
            if let Some(open_at) = find(&self.tail, THINK_OPEN_MARKER) {
                if self.tail[open_at + THINK_OPEN_MARKER.len()..].contains(&b'>') {
                    self.opened = true;
                    self.open_pending = false;
                } else {
                    self.open_pending = true;
                }
            } else if self.open_pending && self.tail.contains(&b'>') {
                self.opened = true;
                self.open_pending = false;
            }
            return ThinkingBudgetAction::None;
        }

        // Inside the think block: one counted token per ingest.
        if self.budget_remaining == 0 {
            self.exhausted = true;
            return ThinkingBudgetAction::InjectClose;
        }
        self.budget_remaining -= 1;
        if self.budget_remaining == 0 {
            // The budget-tripping token is already sampled and emitted by the
            // time its piece reaches the tracker — the close tag is injected
            // right after it, so exactly `budget` thinking tokens surface.
            self.exhausted = true;
            return ThinkingBudgetAction::InjectClose;
        }
        ThinkingBudgetAction::None
    }

    /// `&str` convenience wrapper for tests and non-FFI callers.
    pub fn ingest(&mut self, piece: &str) -> ThinkingBudgetAction {
        self.ingest_bytes(piece.as_bytes())
    }
}

fn find(haystack: &[u8], needle: &str) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{ThinkingBudget, ThinkingBudgetAction, ThinkingBudgetTracker};

    fn tracker(budget: u32, starts_in_thinking: bool) -> ThinkingBudgetTracker {
        ThinkingBudgetTracker::new(ThinkingBudget { budget, starts_in_thinking })
    }

    #[test]
    fn prefill_counts_from_first_token() {
        let mut t = tracker(3, true);
        assert_eq!(t.ingest("pl"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("anning"), ThinkingBudgetAction::None);
        // Budget 3 trips on the third thinking token — exactly 3 thinking
        // tokens surface (each piece is emitted before its ingest).
        assert_eq!(t.ingest(" hard"), ThinkingBudgetAction::InjectClose);
        assert_eq!(t.ingest("er"), ThinkingBudgetAction::None);
    }

    #[test]
    fn mid_generation_open_tag_starts_counting() {
        let mut t = tracker(2, false);
        // Content before the think block is not counted.
        assert_eq!(t.ingest("Sure, "), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("<think>"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("step"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest(" two"), ThinkingBudgetAction::InjectClose);
    }

    #[test]
    fn split_open_marker_is_stitched_from_tail() {
        let mut t = tracker(1, false);
        assert_eq!(t.ingest("answer <th"), ThinkingBudgetAction::None);
        // The `>` completing the open tag arrives in a later piece.
        assert_eq!(t.ingest("ink>\n"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("reasoning"), ThinkingBudgetAction::InjectClose);
    }

    #[test]
    fn split_close_tag_ends_reasoning_without_injection() {
        let mut t = tracker(10, true);
        assert_eq!(t.ingest("thinking "), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("</thi"), ThinkingBudgetAction::None);
        // The completed close tag wins over the budget that trips on it.
        assert_eq!(t.ingest("nk>done"), ThinkingBudgetAction::None);
        // Already closed — inert for the rest of the answer.
        assert_eq!(t.ingest(" more"), ThinkingBudgetAction::None);
    }

    #[test]
    fn fires_exactly_once_then_idempotent() {
        let mut t = tracker(1, true);
        assert_eq!(t.ingest("only"), ThinkingBudgetAction::InjectClose);
        assert_eq!(t.ingest(" token"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest(" more"), ThinkingBudgetAction::None);
    }

    #[test]
    fn plain_model_never_opens_is_inert() {
        let mut t = tracker(512, false);
        for piece in ["The answer", " is 42", ". No thinking here."] {
            assert_eq!(t.ingest(piece), ThinkingBudgetAction::None);
        }
    }

    #[test]
    fn zero_budget_trips_on_first_thinking_token() {
        let mut t = tracker(0, true);
        assert_eq!(t.ingest("x"), ThinkingBudgetAction::InjectClose);
    }

    #[test]
    fn multi_attribute_open_tag_completes_on_angle_bracket() {
        // Attribute-tolerant open tag: `<think\n>` — the `>` closes the tag.
        let mut t = tracker(1, false);
        assert_eq!(t.ingest("<think\n"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest(">"), ThinkingBudgetAction::None);
        assert_eq!(t.ingest("reasoning"), ThinkingBudgetAction::InjectClose);
    }
}
