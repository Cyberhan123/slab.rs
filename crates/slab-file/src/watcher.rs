//! File-system change watcher.
//!
//! A lightweight wrapper around the `notify` crate that provides a
//! subscriber-based API compatible with the `codex-file-watcher` interface
//! used by `slab-agent-tools`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Errors produced by the file watcher.
#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
}

/// A path to watch along with its recursion mode.
#[derive(Debug, Clone)]
pub struct WatchPath {
    pub path: PathBuf,
    pub recursive: bool,
}

/// A single file-system change event.
#[derive(Debug, Clone)]
pub struct FileEvent {
    pub paths: Vec<PathBuf>,
    pub kind: FileEventKind,
}

/// The kind of file-system change that occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Create,
    Modify,
    Remove,
    Other,
}

impl From<&EventKind> for FileEventKind {
    fn from(kind: &EventKind) -> Self {
        match kind {
            EventKind::Create(_) => FileEventKind::Create,
            EventKind::Modify(_) => FileEventKind::Modify,
            EventKind::Remove(_) => FileEventKind::Remove,
            _ => FileEventKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WatchKey {
    path: PathBuf,
    recursive: bool,
}

#[derive(Debug, Clone)]
struct WatchTarget {
    requested_path: PathBuf,
    registered_path: PathBuf,
    recursive: bool,
}

struct SubscriberEntry {
    tx: mpsc::UnboundedSender<FileEvent>,
    watched_paths: Vec<WatchTarget>,
}

#[derive(Default)]
struct WatchRegistration {
    count: usize,
}

#[derive(Default)]
struct WatcherState {
    subscribers: HashMap<u64, SubscriberEntry>,
    watch_counts: HashMap<WatchKey, WatchRegistration>,
    next_id: u64,
}

struct Inner {
    state: Mutex<WatcherState>,
    watcher: Mutex<Option<RecommendedWatcher>>,
}

/// File-system watcher with subscriber-based event fan-out.
///
/// Create one instance per application and share it via [`Arc`].
/// Call [`FileWatcher::add_subscriber`] to obtain a per-use subscription.
pub struct FileWatcher {
    inner: Arc<Inner>,
}

/// A subscription handle. Drop to unsubscribe.
///
/// Call [`FileWatcherSubscriber::register_paths`] after creation to specify
/// which paths should produce events on the associated receiver.
pub struct FileWatcherSubscriber {
    id: u64,
    inner: Arc<Inner>,
}

pub type Subscriber = FileWatcherSubscriber;

/// Debounced wrapper around a file watcher receiver.
pub struct ThrottledWatchReceiver {
    rx: mpsc::UnboundedReceiver<FileEvent>,
    debounce: Duration,
}

impl ThrottledWatchReceiver {
    pub fn new(rx: mpsc::UnboundedReceiver<FileEvent>, debounce: Duration) -> Self {
        Self { rx, debounce }
    }

    pub async fn recv(&mut self) -> Option<Vec<FileEvent>> {
        let first = self.rx.recv().await?;
        let mut events = vec![first];
        loop {
            match timeout(self.debounce, self.rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) | Err(_) => break,
            }
        }
        Some(events)
    }
}

impl FileWatcherSubscriber {
    /// Register the paths this subscriber wants to receive events for.
    ///
    /// Paths are watched on the underlying OS watcher immediately. Any
    /// previously registered paths are unwatched first. Missing paths are
    /// watched through their closest existing ancestor and upgraded after they
    /// are created.
    pub fn register_paths(&self, paths: Vec<WatchPath>) {
        let targets = paths
            .into_iter()
            .filter_map(|path| match watch_target(&path.path, path.recursive) {
                Ok(target) => Some(target),
                Err(error) => {
                    warn!(path = %path.path.display(), error = %error, "failed to resolve watch path");
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut state = self.inner.state.lock().unwrap();
        let mut watcher = self.inner.watcher.lock().unwrap();
        let Some(entry) = state.subscribers.get_mut(&self.id) else {
            return;
        };
        let old_targets = std::mem::replace(&mut entry.watched_paths, targets.clone());
        for target in old_targets {
            unregister_target(&mut state, watcher.as_mut(), &target);
        }
        for target in targets {
            register_target(&mut state, watcher.as_mut(), &target);
        }
        debug!(subscriber_id = self.id, "registered watch paths");
    }
}

impl Drop for FileWatcherSubscriber {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        let mut watcher = self.inner.watcher.lock().unwrap();
        if let Some(entry) = state.subscribers.remove(&self.id) {
            for target in entry.watched_paths {
                unregister_target(&mut state, watcher.as_mut(), &target);
            }
        }
        debug!(subscriber_id = self.id, "removed subscriber");
    }
}

impl FileWatcher {
    /// Create a live watcher backed by the OS inotify / FSEvents / kqueue API.
    pub fn new() -> Result<Self, WatcherError> {
        let inner = Arc::new(Inner {
            state: Mutex::new(WatcherState { next_id: 1, ..WatcherState::default() }),
            watcher: Mutex::new(None),
        });

        let (event_tx, event_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })?;

        *inner.watcher.lock().unwrap() = Some(watcher);

        let inner_for_thread = Arc::clone(&inner);
        std::thread::spawn(move || {
            while let Ok(event_result) = event_rx.recv() {
                match event_result {
                    Ok(event) => dispatch_event(&inner_for_thread, event),
                    Err(error) => warn!(%error, "file watcher error"),
                }
            }
        });

        Ok(Self { inner })
    }

    /// Create an inert watcher that never fires events (useful in tests).
    pub fn noop() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(WatcherState { next_id: 1, ..WatcherState::default() }),
                watcher: Mutex::new(None),
            }),
        }
    }

    /// Create a new subscriber.
    ///
    /// Returns `(Subscriber, Receiver)`. Call [`Subscriber::register_paths`]
    /// to specify which paths trigger events, then poll the receiver.
    pub fn add_subscriber(&self) -> (Subscriber, mpsc::UnboundedReceiver<FileEvent>) {
        let mut state = self.inner.state.lock().unwrap();
        let id = state.next_id;
        state.next_id += 1;
        let (tx, rx) = mpsc::unbounded_channel();
        state.subscribers.insert(id, SubscriberEntry { tx, watched_paths: Vec::new() });
        (FileWatcherSubscriber { id, inner: Arc::clone(&self.inner) }, rx)
    }
}

fn dispatch_event(inner: &Inner, event: Event) {
    let kind = FileEventKind::from(&event.kind);
    let file_event = FileEvent { paths: event.paths.clone(), kind };
    let mut sends = Vec::new();
    let mut state = inner.state.lock().unwrap();
    let mut watcher = inner.watcher.lock().unwrap();
    upgrade_created_targets(&mut state, watcher.as_mut());

    for entry in state.subscribers.values() {
        if entry.watched_paths.iter().any(|target| event_matches_target(&event.paths, target)) {
            sends.push(entry.tx.clone());
        }
    }
    drop(watcher);
    drop(state);

    for tx in sends {
        let _ = tx.send(file_event.clone());
    }
}

fn upgrade_created_targets(state: &mut WatcherState, watcher: Option<&mut RecommendedWatcher>) {
    let subscriber_ids = state.subscribers.keys().copied().collect::<Vec<_>>();
    let mut watcher = watcher;
    for subscriber_id in subscriber_ids {
        let Some(entry) = state.subscribers.get(&subscriber_id) else {
            continue;
        };
        let replacements = entry
            .watched_paths
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                if !target.requested_path.exists() {
                    return None;
                }
                let Ok(upgraded) = watch_target(&target.requested_path, target.recursive) else {
                    return None;
                };
                (upgraded.registered_path != target.registered_path).then_some((index, upgraded))
            })
            .collect::<Vec<_>>();

        for (index, upgraded) in replacements {
            let Some(entry) = state.subscribers.get_mut(&subscriber_id) else {
                continue;
            };
            let old = std::mem::replace(&mut entry.watched_paths[index], upgraded.clone());
            unregister_target(state, watcher.as_deref_mut(), &old);
            register_target(state, watcher.as_deref_mut(), &upgraded);
        }
    }
}

fn register_target(
    state: &mut WatcherState,
    watcher: Option<&mut RecommendedWatcher>,
    target: &WatchTarget,
) {
    let key = watch_key(target);
    let registration = state.watch_counts.entry(key.clone()).or_default();
    registration.count += 1;
    if registration.count != 1 {
        return;
    }

    let Some(watcher) = watcher else {
        return;
    };
    let mode =
        if target.recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
    if let Err(error) = watcher.watch(&target.registered_path, mode) {
        warn!(path = %target.registered_path.display(), %error, "failed to watch path");
    }
}

fn unregister_target(
    state: &mut WatcherState,
    watcher: Option<&mut RecommendedWatcher>,
    target: &WatchTarget,
) {
    let key = watch_key(target);
    let Some(registration) = state.watch_counts.get_mut(&key) else {
        return;
    };
    registration.count = registration.count.saturating_sub(1);
    if registration.count > 0 {
        return;
    }
    state.watch_counts.remove(&key);

    let Some(watcher) = watcher else {
        return;
    };
    if let Err(error) = watcher.unwatch(&target.registered_path) {
        warn!(path = %target.registered_path.display(), %error, "failed to unwatch path");
    }
}

fn watch_key(target: &WatchTarget) -> WatchKey {
    WatchKey { path: target.registered_path.clone(), recursive: target.recursive }
}

fn watch_target(path: &Path, recursive: bool) -> std::io::Result<WatchTarget> {
    let requested_path = absolute_path(path)?;
    let registered_path = if requested_path.exists() {
        requested_path.canonicalize()?
    } else {
        existing_ancestor(&requested_path)?
    };
    Ok(WatchTarget { requested_path, registered_path, recursive })
}

fn event_matches_target(paths: &[PathBuf], target: &WatchTarget) -> bool {
    paths.iter().any(|path| {
        let event_path = absolute_path(path).unwrap_or_else(|_| path.clone());
        event_path == target.requested_path
            || event_path.starts_with(&target.requested_path)
            || target.requested_path.starts_with(&event_path)
            || event_path.starts_with(&target.registered_path)
    })
}

fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    let path =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    if path.exists() { path.canonicalize() } else { Ok(path) }
}

fn existing_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    let mut current = path;
    while !current.exists() {
        current = current.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "watch path has no existing ancestor")
        })?;
    }
    current.canonicalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_path_watches_existing_ancestor() {
        let root = tempfile::tempdir().expect("tempdir");
        let requested = root.path().join("missing").join("file.txt");

        let target = watch_target(&requested, true).expect("target");

        assert_eq!(target.requested_path, requested);
        assert_eq!(target.registered_path, root.path().canonicalize().unwrap());
    }

    #[tokio::test]
    async fn throttled_receiver_batches_events() {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut receiver = ThrottledWatchReceiver::new(rx, Duration::from_millis(10));
        tx.send(FileEvent { paths: vec![PathBuf::from("a")], kind: FileEventKind::Modify })
            .unwrap();
        tx.send(FileEvent { paths: vec![PathBuf::from("b")], kind: FileEventKind::Modify })
            .unwrap();

        let batch = receiver.recv().await.expect("batch");

        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn noop_watcher_keeps_subscriber_api() {
        let watcher = FileWatcher::noop();
        let (subscriber, _rx) = watcher.add_subscriber();

        subscriber.register_paths(vec![WatchPath { path: PathBuf::from("."), recursive: true }]);
    }
}
