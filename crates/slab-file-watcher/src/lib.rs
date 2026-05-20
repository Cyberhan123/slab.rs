//! File-system change watcher.
//!
//! A lightweight wrapper around the `notify` crate that provides a
//! subscriber-based API compatible with the `codex-file-watcher` interface
//! used by `slab-agent-tools`.

mod error;
mod watcher;

pub use error::WatcherError;
pub use watcher::{FileEvent, FileEventKind, FileWatcher, Subscriber, WatchPath};
