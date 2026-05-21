mod backend;
mod pipe;
mod process;
mod process_group;
#[cfg(test)]
mod tests;
#[cfg(windows)]
mod win;

/// Report whether ConPTY is available on this platform (Windows only).
pub use backend::conpty_supported;
/// Spawn a process attached to a PTY for interactive use.
pub use backend::spawn_process as spawn_pty_process;
/// Spawn a non-interactive process using regular pipes for stdin/stdout/stderr.
pub use pipe::spawn_process as spawn_pipe_process;
/// Spawn a non-interactive process using regular pipes, but close stdin immediately.
pub use pipe::spawn_process_no_stdin as spawn_pipe_process_no_stdin;
/// Handle for interacting with a spawned process (PTY or pipe).
pub use process::ProcessHandle;
/// Bundle of process handles plus split output and exit receivers returned by spawn helpers.
pub use process::SpawnedProcess;
/// Terminal size in character cells used for PTY spawn and resize operations.
pub use process::TerminalSize;
/// Combine stdout/stderr receivers into a single broadcast receiver.
pub use process::combine_output_receivers;
