//! Read side of the memory workspace: load the injectable summary and parse
//! the `<oai-mem-citation>` blocks the model appends to final replies.
//!
//! The developer-message rendering itself lives in `slab-agent-context` (the
//! `memory` fragment template); this module only loads the structured inputs.

use std::path::Path;

use regex::Regex;

use crate::{Result, error::fs_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCitation {
    pub source: String,
    pub source_kind: MemoryCitationSourceKind,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCitationSourceKind {
    MemorySummary,
    MemoryRegistry,
    RawMemory,
    RolloutSummary,
    Unknown,
}

impl MemoryCitationSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemorySummary => "memory_summary",
            Self::MemoryRegistry => "memory_registry",
            Self::RawMemory => "raw_memory",
            Self::RolloutSummary => "rollout_summary",
            Self::Unknown => "unknown",
        }
    }
}

/// Load the project's `memory_summary.md` for injection, if the project has a
/// v1 summary to offer. `None` (missing/empty file, or a summary that does
/// not start with `v1`) skips the memory fragment entirely.
///
/// The summary is bounded to the read token budget (the cut backs off to a
/// line boundary, so the leading `v1` marker survives) — an over-grown
/// `memory_summary.md` must not silently inflate every agent start.
pub fn load_memory_summary(memory_root: &Path) -> Result<Option<String>> {
    let Some(memory_summary) = read_optional(&memory_root.join("memory_summary.md"))? else {
        return Ok(None);
    };
    if !memory_summary.starts_with("v1") {
        return Ok(None);
    }
    Ok(Some(crate::recall::truncate_to_token_budget(
        &memory_summary,
        crate::recall::SUMMARY_TOKEN_BUDGET,
    )))
}

pub fn parse_memory_citations(text: &str) -> Vec<MemoryCitation> {
    let block_re = Regex::new(
        r"(?s)<oai-mem-citation>\s*<citation_entries>\s*(?P<body>.*?)\s*</citation_entries>",
    )
    .expect("valid citation block regex");
    block_re
        .captures_iter(text)
        .flat_map(|captures| {
            captures["body"]
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter_map(parse_citation_line)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Parse the `<rollout_ids>` block of an `<oai-mem-citation>` into valid
/// rollout (session) UUIDs. The ids are the session ids the memory pipeline
/// originally extracted from, so they let a citation bump usage even when the
/// cited file's slug no longer maps back to a phase1 row.
pub fn parse_memory_rollout_ids(text: &str) -> Vec<String> {
    let block_re = Regex::new(r"(?s)<rollout_ids>\s*(?P<body>.*?)\s*</rollout_ids>")
        .expect("valid rollout ids block regex");
    block_re
        .captures_iter(text)
        .flat_map(|captures| {
            captures["body"]
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter_map(|line| uuid::Uuid::parse_str(line).ok().map(|_| line.to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn parse_citation_line(line: &str) -> Option<MemoryCitation> {
    let (source, note) = match line.split_once("|note=[") {
        Some((source, rest)) => (source.trim(), rest.strip_suffix(']').map(str::to_owned)),
        None => (line.trim(), None),
    };
    (!source.is_empty()).then(|| MemoryCitation {
        source: source.to_owned(),
        source_kind: classify_citation_source(source),
        note,
    })
}

pub fn classify_citation_source(source: &str) -> MemoryCitationSourceKind {
    let path = source.split_once(':').map_or(source, |(path, _)| path).replace('\\', "/");
    if path == "memory_summary.md" {
        return MemoryCitationSourceKind::MemorySummary;
    }
    if path == "MEMORY.md" {
        return MemoryCitationSourceKind::MemoryRegistry;
    }
    if path == "raw_memories.md" {
        return MemoryCitationSourceKind::RawMemory;
    }
    if path.starts_with("rollout_summaries/") && path.ends_with(".md") {
        return MemoryCitationSourceKind::RolloutSummary;
    }
    MemoryCitationSourceKind::Unknown
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(fs_error(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rollout_ids() {
        let ids = parse_memory_rollout_ids(
            "<oai-mem-citation>\n<citation_entries>\nMEMORY.md:1-2\n</citation_entries>\n<rollout_ids>\n0b6e64b2-90d2-4d3a-9f7c-2a1d3c4e5f60\nnot-a-uuid\n7f9242ce-1b3a-4c5d-8e6f-708192a3b4c5\n</rollout_ids>\n</oai-mem-citation>",
        );

        assert_eq!(
            ids,
            vec![
                "0b6e64b2-90d2-4d3a-9f7c-2a1d3c4e5f60".to_owned(),
                "7f9242ce-1b3a-4c5d-8e6f-708192a3b4c5".to_owned(),
            ],
            "invalid lines dropped, order kept"
        );
        assert!(parse_memory_rollout_ids("no block here").is_empty());
    }

    #[test]
    fn parses_memory_citations() {
        let citations = parse_memory_citations(
            "<oai-mem-citation>\n<citation_entries>\nMEMORY.md:1-2|note=[used]\n</citation_entries>\n<rollout_ids>\n</rollout_ids>\n</oai-mem-citation>",
        );

        assert_eq!(
            citations,
            vec![MemoryCitation {
                source: "MEMORY.md:1-2".to_owned(),
                source_kind: MemoryCitationSourceKind::MemoryRegistry,
                note: Some("used".to_owned())
            }]
        );
    }

    #[test]
    fn classifies_memory_citation_sources() {
        assert_eq!(
            classify_citation_source("memory_summary.md:1-2"),
            MemoryCitationSourceKind::MemorySummary
        );
        assert_eq!(
            classify_citation_source("raw_memories.md:1-2"),
            MemoryCitationSourceKind::RawMemory
        );
        assert_eq!(
            classify_citation_source("rollout_summaries/thread.md:1-2"),
            MemoryCitationSourceKind::RolloutSummary
        );
        assert_eq!(classify_citation_source("other.md"), MemoryCitationSourceKind::Unknown);
    }

    #[test]
    fn skips_missing_summary() {
        let root = tempfile::tempdir().expect("tempdir");

        assert!(load_memory_summary(root.path()).expect("read").is_none());
    }

    #[test]
    fn loads_v1_summary_and_skips_other_markers() {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::write(root.path().join("memory_summary.md"), "v1\n# Summary\nbody\n")
            .expect("write");

        let loaded = load_memory_summary(root.path()).expect("read");
        assert_eq!(loaded.expect("v1 summary"), "v1\n# Summary\nbody\n");

        std::fs::write(root.path().join("memory_summary.md"), "v2\nschema change\n")
            .expect("write");
        assert!(load_memory_summary(root.path()).expect("read").is_none());
    }
}
