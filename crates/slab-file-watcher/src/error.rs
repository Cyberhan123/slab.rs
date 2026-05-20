/// Errors produced by the file watcher.
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
}
