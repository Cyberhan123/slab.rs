use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::error::WatcherError;

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

struct SubscriberEntry {
    tx: mpsc::UnboundedSender<FileEvent>,
    watched_paths: Vec<WatchPath>,
}

struct Inner {
    subscribers: Mutex<HashMap<u64, SubscriberEntry>>,
    next_id: Mutex<u64>,
    /// Keeps the underlying OS watcher alive. `None` for the noop watcher.
    _watcher: Mutex<Option<RecommendedWatcher>>,
}

/// File-system watcher with subscriber-based event fan-out.
///
/// Create one instance per application and share it via [`Arc`].
/// Call [`FileWatcher::add_subscriber`] to obtain a per-use subscription.
pub struct FileWatcher {
    inner: Arc<Inner>,
}

/// A subscription handle.  Drop to unsubscribe.
///
/// Call [`Subscriber::register_paths`] after creation to specify which paths
/// should produce events on the associated receiver.
pub struct Subscriber {
    id: u64,
    inner: Arc<Inner>,
}

impl Subscriber {
    /// Register the paths this subscriber wants to receive events for.
    ///
    /// Paths are watched on the underlying OS watcher immediately.
    /// Any previously registered paths are unwatched first.
    pub fn register_paths(&self, paths: Vec<WatchPath>) {
        let mut subs = self.inner.subscribers.lock().unwrap();
        if let Some(entry) = subs.get_mut(&self.id) {
            let mut watcher_guard = self.inner._watcher.lock().unwrap();
            if let Some(ref mut w) = *watcher_guard {
                for wp in &entry.watched_paths {
                    if let Err(e) = w.unwatch(&wp.path) {
                        warn!(path = %wp.path.display(), error = %e, "failed to unwatch path");
                    }
                }
                for wp in &paths {
                    let mode = if wp.recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    };
                    if let Err(e) = w.watch(&wp.path, mode) {
                        warn!(path = %wp.path.display(), error = %e, "failed to watch path");
                    }
                }
            }
            entry.watched_paths = paths;
            debug!(subscriber_id = self.id, "registered watch paths");
        }
    }
}

impl Drop for Subscriber {
    fn drop(&mut self) {
        let mut subs = self.inner.subscribers.lock().unwrap();
        if let Some(entry) = subs.remove(&self.id) {
            let mut watcher_guard = self.inner._watcher.lock().unwrap();
            if let Some(ref mut w) = *watcher_guard {
                for wp in &entry.watched_paths {
                    if let Err(e) = w.unwatch(&wp.path) {
                        warn!(path = %wp.path.display(), error = %e, "failed to unwatch path on drop");
                    }
                }
            }
        }
        debug!(subscriber_id = self.id, "removed subscriber");
    }
}

impl FileWatcher {
    /// Create a live watcher backed by the OS inotify / FSEvents / kqueue API.
    pub fn new() -> Result<Self, WatcherError> {
        let inner = Arc::new(Inner {
            subscribers: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
            _watcher: Mutex::new(None),
        });

        let (event_tx, event_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

        let watcher = notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        })?;

        *inner._watcher.lock().unwrap() = Some(watcher);

        let inner_for_thread = Arc::clone(&inner);
        std::thread::spawn(move || {
            while let Ok(event_result) = event_rx.recv() {
                match event_result {
                    Ok(event) => {
                        let kind = FileEventKind::from(&event.kind);
                        let file_event = FileEvent { paths: event.paths.clone(), kind };
                        let subs = inner_for_thread.subscribers.lock().unwrap();
                        for entry in subs.values() {
                            let relevant = entry.watched_paths.iter().any(|wp| {
                                event.paths.iter().any(|ep| ep.starts_with(&wp.path))
                            });
                            if relevant {
                                let _ = entry.tx.send(file_event.clone());
                            }
                        }
                    }
                    Err(e) => warn!(error = %e, "file watcher error"),
                }
            }
        });

        Ok(Self { inner })
    }

    /// Create an inert watcher that never fires events (useful in tests).
    pub fn noop() -> Self {
        Self {
            inner: Arc::new(Inner {
                subscribers: Mutex::new(HashMap::new()),
                next_id: Mutex::new(1),
                _watcher: Mutex::new(None),
            }),
        }
    }

    /// Create a new subscriber.
    ///
    /// Returns `(Subscriber, Receiver)`.  Call [`Subscriber::register_paths`]
    /// to specify which paths trigger events, then poll the receiver.
    pub fn add_subscriber(&self) -> (Subscriber, mpsc::UnboundedReceiver<FileEvent>) {
        let id = {
            let mut n = self.inner.next_id.lock().unwrap();
            let id = *n;
            *n += 1;
            id
        };
        let (tx, rx) = mpsc::unbounded_channel();
        self.inner
            .subscribers
            .lock()
            .unwrap()
            .insert(id, SubscriberEntry { tx, watched_paths: vec![] });
        (Subscriber { id, inner: Arc::clone(&self.inner) }, rx)
    }
}
