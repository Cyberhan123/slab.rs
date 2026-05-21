//! Gitignore-aware fuzzy file search.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use ignore::{DirEntry, WalkBuilder};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: usize = 100;

/// Options used by one-shot and streaming file search.
#[derive(Debug, Clone)]
pub struct FileSearchOptions {
    pub root: PathBuf,
    pub query: String,
    pub limit: usize,
    pub include_dirs: bool,
    pub include_hidden: bool,
    pub extra_ignore_names: Vec<String>,
}

impl FileSearchOptions {
    pub fn new(root: impl Into<PathBuf>, query: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            query: query.into(),
            limit: DEFAULT_LIMIT,
            include_dirs: false,
            include_hidden: false,
            extra_ignore_names: Vec::new(),
        }
    }

    fn normalized_limit(&self) -> usize {
        if self.limit == 0 { DEFAULT_LIMIT } else { self.limit }
    }
}

/// A matched path returned by the fuzzy matcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMatch {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub name: String,
    pub score: u32,
    pub indices: Vec<u32>,
}

/// A point-in-time search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSearchSnapshot {
    pub query: String,
    pub matches: Vec<FileMatch>,
    pub scanned: usize,
    pub truncated: bool,
    pub complete: bool,
}

/// Receives snapshots from a streaming file search session.
pub trait SessionReporter: Send + Sync + 'static {
    fn report(&self, snapshot: FileSearchSnapshot);
}

/// A reporter that publishes snapshots over a channel.
#[derive(Clone)]
pub struct ChannelSessionReporter {
    tx: Sender<FileSearchSnapshot>,
}

impl ChannelSessionReporter {
    pub fn new() -> (Self, Receiver<FileSearchSnapshot>) {
        let (tx, rx) = unbounded();
        (Self { tx }, rx)
    }
}

impl SessionReporter for ChannelSessionReporter {
    fn report(&self, snapshot: FileSearchSnapshot) {
        let _ = self.tx.send(snapshot);
    }
}

/// Streaming fuzzy file search session.
pub struct FileSearchSession {
    command_tx: Sender<MatcherCommand>,
    walker_thread: Option<JoinHandle<()>>,
    matcher_thread: Option<JoinHandle<()>>,
}

impl FileSearchSession {
    pub fn spawn(options: FileSearchOptions, reporter: Arc<dyn SessionReporter>) -> Result<Self> {
        let root = options
            .root
            .canonicalize()
            .with_context(|| format!("failed to resolve search root {}", options.root.display()))?;
        let options = FileSearchOptions { root, ..options };
        let (candidate_tx, candidate_rx) = unbounded();
        let (command_tx, command_rx) = unbounded();
        let walker_options = options.clone();
        let matcher_options = options.clone();

        let walker_thread = thread::spawn(move || {
            walk_files(walker_options, candidate_tx);
        });
        let matcher_thread = thread::spawn(move || {
            run_matcher(matcher_options, candidate_rx, command_rx, reporter);
        });

        Ok(Self {
            command_tx,
            walker_thread: Some(walker_thread),
            matcher_thread: Some(matcher_thread),
        })
    }

    pub fn update_query(&self, query: impl Into<String>) {
        let _ = self.command_tx.send(MatcherCommand::UpdateQuery(query.into()));
    }

    pub fn stop(&self) {
        let _ = self.command_tx.send(MatcherCommand::Stop);
    }
}

impl Drop for FileSearchSession {
    fn drop(&mut self) {
        let _ = self.command_tx.send(MatcherCommand::Stop);
        if let Some(thread) = self.walker_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.matcher_thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn run(options: FileSearchOptions) -> Result<FileSearchSnapshot> {
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve search root {}", options.root.display()))?;
    let options = FileSearchOptions { root, ..options };
    if options.query.trim().is_empty() {
        return Ok(FileSearchSnapshot {
            query: String::new(),
            matches: Vec::new(),
            scanned: 0,
            truncated: false,
            complete: true,
        });
    }

    let mut candidates = Vec::new();
    let (candidate_tx, candidate_rx) = unbounded();
    walk_files(options.clone(), candidate_tx);
    while let Ok(message) = candidate_rx.recv() {
        match message {
            WalkerMessage::Candidate(candidate) => candidates.push(candidate),
            WalkerMessage::Done => break,
        }
    }

    Ok(snapshot_for(&options.query, &candidates, options.normalized_limit(), true))
}

#[derive(Debug, Clone)]
struct FileCandidate {
    relative_path: String,
    absolute_path: PathBuf,
    name: String,
}

enum WalkerMessage {
    Candidate(FileCandidate),
    Done,
}

enum MatcherCommand {
    UpdateQuery(String),
    Stop,
}

fn walk_files(options: FileSearchOptions, tx: Sender<WalkerMessage>) {
    let extra_ignore_names = Arc::new(
        options.extra_ignore_names.iter().map(|name| normalize_name(name)).collect::<HashSet<_>>(),
    );
    let root = options.root.clone();
    let include_dirs = options.include_dirs;
    let mut builder = WalkBuilder::new(&root);
    builder.hidden(!options.include_hidden);
    builder.filter_entry({
        let extra_ignore_names = Arc::clone(&extra_ignore_names);
        move |entry| !entry_has_ignored_name(entry, &extra_ignore_names)
    });

    for result in builder.build() {
        let Ok(entry) = result else {
            continue;
        };
        let path = entry.path();
        if path == root {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() && !include_dirs {
            continue;
        }
        if !file_type.is_dir() && !file_type.is_file() {
            continue;
        }
        let Ok(absolute_path) = path.canonicalize() else {
            continue;
        };
        if !absolute_path.starts_with(&root) {
            continue;
        }
        let Ok(relative_path) = absolute_path.strip_prefix(&root) else {
            continue;
        };
        let relative_path = relative_path.to_string_lossy().replace('\\', "/");
        if relative_path.is_empty() {
            continue;
        }
        let name = absolute_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| relative_path.clone());
        if tx
            .send(WalkerMessage::Candidate(FileCandidate { relative_path, absolute_path, name }))
            .is_err()
        {
            return;
        }
    }

    let _ = tx.send(WalkerMessage::Done);
}

fn run_matcher(
    options: FileSearchOptions,
    candidate_rx: Receiver<WalkerMessage>,
    command_rx: Receiver<MatcherCommand>,
    reporter: Arc<dyn SessionReporter>,
) {
    let mut candidates = Vec::new();
    let mut query = options.query.clone();
    let mut complete = false;
    let limit = options.normalized_limit();
    report_snapshot(&query, &candidates, limit, complete, reporter.as_ref());

    loop {
        match command_rx.try_recv() {
            Ok(MatcherCommand::UpdateQuery(updated)) => {
                query = updated;
                report_snapshot(&query, &candidates, limit, complete, reporter.as_ref());
            }
            Ok(MatcherCommand::Stop) => break,
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        match candidate_rx.recv_timeout(std::time::Duration::from_millis(25)) {
            Ok(WalkerMessage::Candidate(candidate)) => {
                candidates.push(candidate);
                drain_candidates(&candidate_rx, &mut candidates, &mut complete);
                report_snapshot(&query, &candidates, limit, complete, reporter.as_ref());
            }
            Ok(WalkerMessage::Done) => {
                complete = true;
                report_snapshot(&query, &candidates, limit, complete, reporter.as_ref());
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if complete {
                    continue;
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_candidates(
    candidate_rx: &Receiver<WalkerMessage>,
    candidates: &mut Vec<FileCandidate>,
    complete: &mut bool,
) {
    loop {
        match candidate_rx.try_recv() {
            Ok(WalkerMessage::Candidate(candidate)) => candidates.push(candidate),
            Ok(WalkerMessage::Done) => {
                *complete = true;
                break;
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
        }
    }
}

fn report_snapshot(
    query: &str,
    candidates: &[FileCandidate],
    limit: usize,
    complete: bool,
    reporter: &dyn SessionReporter,
) {
    reporter.report(snapshot_for(query, candidates, limit, complete));
}

fn snapshot_for(
    query: &str,
    candidates: &[FileCandidate],
    limit: usize,
    complete: bool,
) -> FileSearchSnapshot {
    let query = query.trim().to_owned();
    let mut matches = if query.is_empty() { Vec::new() } else { fuzzy_match(&query, candidates) };
    let truncated = matches.len() > limit;
    matches.truncate(limit);
    FileSearchSnapshot { query, matches, scanned: candidates.len(), truncated, complete }
}

fn fuzzy_match(query: &str, candidates: &[FileCandidate]) -> Vec<FileMatch> {
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matched = Vec::new();

    for candidate in candidates {
        let mut haystack = candidate.relative_path.clone();
        let mut buf = Vec::new();
        let mut indices = Vec::new();
        let Some(score) =
            pattern.indices(Utf32Str::new(&mut haystack, &mut buf), &mut matcher, &mut indices)
        else {
            continue;
        };
        matched.push(FileMatch {
            relative_path: candidate.relative_path.clone(),
            absolute_path: candidate.absolute_path.clone(),
            name: candidate.name.clone(),
            score,
            indices,
        });
    }

    matched.sort_by(|left, right| {
        right.score.cmp(&left.score).then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    matched
}

fn entry_has_ignored_name(entry: &DirEntry, ignored_names: &HashSet<String>) -> bool {
    if ignored_names.is_empty() {
        return false;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    ignored_names.contains(&normalize_name(name))
}

fn normalize_name(name: &str) -> String {
    if cfg!(windows) { name.to_lowercase() } else { name.to_owned() }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn run_respects_gitignore_and_extra_ignored_names() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("src")).expect("src");
        fs::create_dir_all(root.path().join("target")).expect("target");
        fs::write(root.path().join(".gitignore"), "ignored.rs\n").expect("gitignore");
        fs::write(root.path().join("src").join("workspace_search.rs"), "").expect("source");
        fs::write(root.path().join("src").join("ignored.rs"), "").expect("ignored");
        fs::write(root.path().join("target").join("workspace_search.rs"), "").expect("target");

        let mut options = FileSearchOptions::new(root.path(), "wss");
        options.extra_ignore_names = vec!["target".to_string()];
        let snapshot = run(options).expect("search");

        assert!(snapshot.complete);
        assert_eq!(snapshot.matches.len(), 1);
        assert_eq!(snapshot.matches[0].relative_path, "src/workspace_search.rs");
    }

    #[test]
    fn empty_query_returns_empty_complete_snapshot() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::write(root.path().join("main.rs"), "").expect("file");

        let snapshot = run(FileSearchOptions::new(root.path(), " ")).expect("search");

        assert!(snapshot.complete);
        assert_eq!(snapshot.scanned, 0);
        assert!(snapshot.matches.is_empty());
    }

    #[test]
    fn session_reports_query_updates() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::write(root.path().join("alpha.rs"), "").expect("alpha");
        fs::write(root.path().join("beta.rs"), "").expect("beta");
        let (reporter, rx) = ChannelSessionReporter::new();
        let session = FileSearchSession::spawn(
            FileSearchOptions::new(root.path(), "alp"),
            Arc::new(reporter),
        )
        .expect("session");

        session.update_query("bet");
        let mut latest = None;
        for _ in 0..20 {
            if let Ok(snapshot) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                latest = Some(snapshot);
                if latest.as_ref().is_some_and(|snapshot| {
                    snapshot.complete
                        && snapshot.query == "bet"
                        && snapshot
                            .matches
                            .first()
                            .is_some_and(|matched| matched.relative_path == "beta.rs")
                }) {
                    break;
                }
            }
        }
        session.stop();

        let latest = latest.expect("snapshot");
        assert_eq!(latest.query, "bet");
        assert_eq!(latest.matches[0].relative_path, "beta.rs");
    }
}
