//! Single-source `<think>` marker constants and reasoning-segment parsers.
//!
//! The markers and the parse helpers here were previously duplicated across
//! `slab-app-core` (chat reasoning), `bin/slab-runtime` (llama engine), and
//! `slab-llama` (budget tracker) — byte-identical copies that had to be kept
//! in sync by hand. This module is the one home; those sites re-export or
//! call it.
//!
//! Scope note: `slab-agent` keeps its own, *stricter* matcher
//! (`turn::find_think_open` requires the next char after `<think` to be `>` or
//! whitespace so `<thinking>` is not matched). That is a different semantic,
//! not drift; the loose prefix here matches what model templates emit.

/// Attribute-tolerant open marker (`<think>`, `<think\n>`, …) — matched as a
/// loose prefix, mirroring llama.cpp's `common` reasoning handling.
pub const THINK_OPEN_MARKER: &str = "<think";

/// The reasoning close tag. Also re-exported by `slab-llama::thinking_budget`
/// for the decode-loop budget injection.
pub const THINK_CLOSE_TAG: &str = "</think>";

/// A raw generation split into visible content and reasoning segments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedThinkingOutput {
    pub content: String,
    pub reasoning: String,
}

/// Length of the trailing run of `raw` that is a strict prefix of `marker`
/// (and shorter than it): a marker possibly split across token pieces that
/// must be held back until the rest arrives.
pub fn trailing_partial_marker_len(raw: &str, marker: &str) -> usize {
    let max = raw.len().min(marker.len().saturating_sub(1));
    (1..=max).rev().find(|len| raw.ends_with(&marker[..*len])).unwrap_or(0)
}

/// Collapse a pre-`<think>` prefix to empty when it is whitespace-only (the
/// templates emit a newline before the tag; that must not surface as content).
pub fn normalize_thinking_content_prefix(prefix: &str) -> &str {
    if prefix.trim().is_empty() { "" } else { prefix }
}

/// Parse generation output whose prompt did NOT prefill an open `<think>`
/// block. See [`parse_thinking_output_with_prefill`] for the semantics.
pub fn parse_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    parse_thinking_output_with_prefill(raw, complete, false)
}

/// Whether the prompt ends inside a prefilled open `<think>` block (Qwen-style
/// template prefill): the last `<think` open tag is unclosed and nothing but
/// whitespace follows it.
pub fn prompt_has_prefilled_thinking(prompt: &str) -> bool {
    let Some(open_start) = prompt.rfind(THINK_OPEN_MARKER) else {
        return false;
    };
    if let Some(close_start) = prompt.rfind(THINK_CLOSE_TAG)
        && close_start > open_start
    {
        return false;
    }

    let after_open_marker = &prompt[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return false;
    };
    after_open_marker[open_end_rel + 1..].trim().is_empty()
}

/// Parse generation output into content/reasoning, honoring a prompt-prefilled
/// open `<think>` block (`prefilled_thinking`, see
/// [`prompt_has_prefilled_thinking`]): then the whole raw output is reasoning
/// until the close tag.
///
/// Streaming safety (`complete == false`): trailing partial-marker runs of
/// either tag are held back until the rest arrives.
pub fn parse_thinking_output_with_prefill(
    raw: &str,
    complete: bool,
    prefilled_thinking: bool,
) -> ParsedThinkingOutput {
    if prefilled_thinking {
        return parse_prefilled_thinking_output(raw, complete);
    }

    let Some(open_start) = raw.find(THINK_OPEN_MARKER) else {
        if complete {
            return ParsedThinkingOutput { content: raw.to_owned(), reasoning: String::new() };
        }

        let stable_end =
            raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_OPEN_MARKER));
        let stable_content = &raw[..stable_end];
        return ParsedThinkingOutput {
            content: if stable_content.trim().is_empty() {
                String::new()
            } else {
                stable_content.to_owned()
            },
            reasoning: String::new(),
        };
    };

    let content_prefix = normalize_thinking_content_prefix(&raw[..open_start]).to_owned();
    let after_open_marker = &raw[open_start..];
    let Some(open_end_rel) = after_open_marker.find('>') else {
        return ParsedThinkingOutput {
            content: if complete { raw.to_owned() } else { content_prefix },
            reasoning: String::new(),
        };
    };

    let reasoning_start = open_start + open_end_rel + 1;
    let after_open = &raw[reasoning_start..];
    if let Some(close_rel) = after_open.find(THINK_CLOSE_TAG) {
        let close_start = reasoning_start + close_rel;
        let close_end = close_start + THINK_CLOSE_TAG.len();
        let mut content = content_prefix;
        content.push_str(&raw[close_end..]);
        return ParsedThinkingOutput {
            content,
            reasoning: raw[reasoning_start..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: content_prefix,
        reasoning: raw[reasoning_start..stable_reasoning_end].to_owned(),
    }
}

/// Parse generation output for a prompt-prefilled open `<think>` block: all
/// output is reasoning until the close tag, the rest is content.
fn parse_prefilled_thinking_output(raw: &str, complete: bool) -> ParsedThinkingOutput {
    if let Some(close_start) = raw.find(THINK_CLOSE_TAG) {
        let close_end = close_start + THINK_CLOSE_TAG.len();
        return ParsedThinkingOutput {
            content: raw[close_end..].to_owned(),
            reasoning: raw[..close_start].to_owned(),
        };
    }

    let stable_reasoning_end = if complete {
        raw.len()
    } else {
        raw.len().saturating_sub(trailing_partial_marker_len(raw, THINK_CLOSE_TAG))
    };
    ParsedThinkingOutput {
        content: String::new(),
        reasoning: raw[..stable_reasoning_end].to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_reasoning_block() {
        let parsed = parse_thinking_output("<think>step one</think>\n\nfinal answer", true);
        assert_eq!(
            parsed,
            ParsedThinkingOutput {
                content: "\n\nfinal answer".to_owned(),
                reasoning: "step one".to_owned(),
            }
        );
    }

    #[test]
    fn attribute_tolerant_open_tag() {
        let parsed = parse_thinking_output("<think\n>step</think>answer", true);
        assert_eq!(parsed.reasoning, "step");
        assert_eq!(parsed.content, "answer");
    }

    #[test]
    fn whitespace_prefix_before_think_collapses() {
        let parsed = parse_thinking_output(" \n<think>step</think>answer", true);
        assert_eq!(parsed.content, "answer");
    }

    #[test]
    fn no_marker_complete_passes_through() {
        let parsed = parse_thinking_output("plain answer", true);
        assert_eq!(parsed.content, "plain answer");
        assert_eq!(parsed.reasoning, "");
    }

    #[test]
    fn no_marker_incomplete_holds_partial_open() {
        // A trailing "<th" may be the start of "<think>" — hold it back.
        let parsed = parse_thinking_output("answer<th", false);
        assert_eq!(parsed.content, "answer");
        // No partial run: everything is stable content.
        let parsed = parse_thinking_output("answer", false);
        assert_eq!(parsed.content, "answer");
    }

    #[test]
    fn unterminated_open_tag_incomplete_holds() {
        // "<think>partial reas" without a close: hold the trailing close-prefix
        // runs, emit the rest as reasoning, prefix as content.
        let parsed = parse_thinking_output("<think>partial reas</thin", false);
        assert_eq!(parsed.content, "");
        assert_eq!(parsed.reasoning, "partial reas");
    }

    #[test]
    fn unterminated_open_tag_without_close_relaxed_when_complete() {
        let parsed = parse_thinking_output("prefix<think>still thinking", true);
        assert_eq!(parsed.content, "prefix");
        assert_eq!(parsed.reasoning, "still thinking");
    }

    #[test]
    fn prefilled_prompt_makes_whole_output_reasoning() {
        assert!(prompt_has_prefilled_thinking("user text\n<think>\n"));
        assert!(prompt_has_prefilled_thinking("user text\n<think\n>\n"));
        // Closed think in the prompt (history) is not a prefill.
        assert!(!prompt_has_prefilled_thinking("<think>old</think>user\n"));
        assert!(!prompt_has_prefilled_thinking("no markers at all"));

        let parsed = parse_thinking_output_with_prefill("step</think>answer", true, true);
        assert_eq!(parsed.reasoning, "step");
        assert_eq!(parsed.content, "answer");

        let parsed = parse_thinking_output_with_prefill("never closed", true, true);
        assert_eq!(parsed.reasoning, "never closed");
        assert_eq!(parsed.content, "");

        // Streaming: trailing partial close tag held back.
        let parsed = parse_thinking_output_with_prefill("step</thin", false, true);
        assert_eq!(parsed.reasoning, "step");
    }

    #[test]
    fn partial_marker_helpers() {
        assert_eq!(trailing_partial_marker_len("abc<thi", THINK_OPEN_MARKER), 4);
        assert_eq!(trailing_partial_marker_len("abc<think", THINK_OPEN_MARKER), 0);
        assert_eq!(trailing_partial_marker_len("no run", THINK_OPEN_MARKER), 0);
        assert_eq!(normalize_thinking_content_prefix("  \n "), "");
        assert_eq!(normalize_thinking_content_prefix(" x "), " x ");
    }
}
