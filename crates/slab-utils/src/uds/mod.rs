//! Cross-platform async Unix domain socket helpers.

mod stdio_to_uds;

use std::io::Result as IoResult;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;

/// Creates `socket_dir` if needed and restricts it to the current user where
/// the platform exposes Unix permissions.
pub async fn prepare_private_socket_directory(socket_dir: impl AsRef<Path>) -> IoResult<()> {
    platform::prepare_private_socket_directory(socket_dir.as_ref()).await
}

/// Returns whether `socket_path` points at a stale Unix socket rendezvous path.
///
/// On Unix this checks the file type. On Windows, `uds_windows` represents the
/// rendezvous as a regular path, so existence is the only useful stale-path
/// signal available.
pub async fn is_stale_socket_path(socket_path: impl AsRef<Path>) -> IoResult<bool> {
    platform::is_stale_socket_path(socket_path.as_ref()).await
}

/// Async Unix domain socket listener.
pub struct UnixListener {
    inner: platform::Listener,
}

impl UnixListener {
    /// Binds a new listener at `socket_path`.
    pub async fn bind(socket_path: impl AsRef<Path>) -> IoResult<Self> {
        platform::bind_listener(socket_path.as_ref()).await.map(|inner| Self { inner })
    }

    /// Accepts the next incoming stream.
    pub async fn accept(&mut self) -> IoResult<UnixStream> {
        self.inner.accept().await.map(|inner| UnixStream { inner })
    }
}

/// Async Unix domain socket stream.
pub struct UnixStream {
    inner: platform::Stream,
}

impl UnixStream {
    /// Connects to `socket_path`.
    pub async fn connect(socket_path: impl AsRef<Path>) -> IoResult<Self> {
        platform::connect_stream(socket_path.as_ref()).await.map(|inner| Self { inner })
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(unix)]
mod platform {
    use std::io;
    use std::io::ErrorKind;
    use std::io::Result as IoResult;
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use tokio::fs;
    use tokio::net::UnixListener;
    use tokio::net::UnixStream;

    /// Owner-only access keeps the control socket directory private while
    /// preserving owner traversal and socket path creation.
    const SOCKET_DIR_MODE: u32 = 0o700;
    const SOCKET_DIR_PERMISSION_BITS: u32 = 0o777;

    pub(super) type Stream = UnixStream;

    pub(super) struct Listener(UnixListener);

    pub(super) async fn prepare_private_socket_directory(socket_dir: &Path) -> IoResult<()> {
        let mut dir_builder = fs::DirBuilder::new();
        dir_builder.mode(SOCKET_DIR_MODE);
        match dir_builder.create(socket_dir).await {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }

        let metadata = fs::symlink_metadata(socket_dir).await?;
        if !metadata.is_dir() {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "socket directory path exists and is not a directory: {}",
                    socket_dir.display()
                ),
            ));
        }

        let permissions = metadata.permissions();
        // The SSH-over-UDS control socket is reachable by path, so the
        // rendezvous directory must be owner-traversable while denying
        // group/other access; exact 0700 fixes insecure modes and unusable
        // owner-only modes like 0600.
        if permissions.mode() & SOCKET_DIR_PERMISSION_BITS != SOCKET_DIR_MODE {
            fs::set_permissions(socket_dir, std::fs::Permissions::from_mode(SOCKET_DIR_MODE))
                .await?;
        }

        Ok(())
    }

    pub(super) async fn bind_listener(socket_path: &Path) -> IoResult<Listener> {
        UnixListener::bind(socket_path).map(Listener)
    }

    impl Listener {
        pub(super) async fn accept(&mut self) -> IoResult<Stream> {
            self.0.accept().await.map(|(stream, _addr)| stream)
        }
    }

    pub(super) async fn connect_stream(socket_path: &Path) -> IoResult<Stream> {
        UnixStream::connect(socket_path).await
    }

    pub(super) async fn is_stale_socket_path(socket_path: &Path) -> IoResult<bool> {
        Ok(fs::symlink_metadata(socket_path).await?.file_type().is_socket())
    }
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::io::Result as IoResult;
    use std::net::Shutdown;
    use std::ops::Deref;
    use std::os::windows::io::AsRawSocket;
    use std::os::windows::io::AsSocket;
    use std::os::windows::io::BorrowedSocket;
    use std::path::Path;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;
    use std::task::ready;

    use async_io::Async;
    use tokio::io::AsyncRead;
    use tokio::io::AsyncWrite;
    use tokio::io::ReadBuf;
    use tokio::task;
    use tokio_util::compat::Compat;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    pub(super) struct Stream(Compat<Async<WindowsUnixStream>>);

    pub(super) async fn prepare_private_socket_directory(socket_dir: &Path) -> IoResult<()> {
        tokio::fs::create_dir_all(socket_dir).await
    }

    pub(super) struct Listener(Async<WindowsUnixListener>);

    pub(super) async fn bind_listener(socket_path: &Path) -> IoResult<Listener> {
        let socket_path = socket_path.to_path_buf();
        let listener =
            spawn_blocking_io(move || uds_windows::UnixListener::bind(socket_path)).await?;
        Async::new(WindowsUnixListener::from(listener)).map(Listener)
    }

    impl Listener {
        pub(super) async fn accept(&mut self) -> IoResult<Stream> {
            let (stream, _addr) = self.0.read_with(|listener| listener.accept()).await?;
            Async::new(WindowsUnixStream::from(stream))
                .map(FuturesAsyncReadCompatExt::compat)
                .map(Stream)
        }
    }

    pub(super) async fn connect_stream(socket_path: &Path) -> IoResult<Stream> {
        let socket_path = socket_path.to_path_buf();
        let stream =
            spawn_blocking_io(move || uds_windows::UnixStream::connect(socket_path)).await?;
        Async::new(WindowsUnixStream::from(stream))
            .map(FuturesAsyncReadCompatExt::compat)
            .map(Stream)
    }

    pub(super) async fn is_stale_socket_path(socket_path: &Path) -> IoResult<bool> {
        tokio::fs::try_exists(socket_path).await
    }

    async fn spawn_blocking_io<T>(
        operation: impl FnOnce() -> IoResult<T> + Send + 'static,
    ) -> IoResult<T>
    where
        T: Send + 'static,
    {
        task::spawn_blocking(operation)
            .await
            .map_err(|err| io::Error::other(format!("blocking socket task failed: {err}")))?
    }

    pub(super) struct WindowsUnixListener(uds_windows::UnixListener);

    impl From<uds_windows::UnixListener> for WindowsUnixListener {
        fn from(listener: uds_windows::UnixListener) -> Self {
            Self(listener)
        }
    }

    impl Deref for WindowsUnixListener {
        type Target = uds_windows::UnixListener;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl AsSocket for WindowsUnixListener {
        fn as_socket(&self) -> BorrowedSocket<'_> {
            unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
        }
    }

    pub(super) struct WindowsUnixStream(uds_windows::UnixStream);

    impl From<uds_windows::UnixStream> for WindowsUnixStream {
        fn from(stream: uds_windows::UnixStream) -> Self {
            Self(stream)
        }
    }

    impl Deref for WindowsUnixStream {
        type Target = uds_windows::UnixStream;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl AsSocket for WindowsUnixStream {
        fn as_socket(&self) -> BorrowedSocket<'_> {
            unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
        }
    }

    impl io::Read for WindowsUnixStream {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            io::Read::read(&mut self.0, buf)
        }
    }

    impl io::Write for WindowsUnixStream {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            io::Write::write(&mut self.0, buf)
        }

        fn flush(&mut self) -> IoResult<()> {
            io::Write::flush(&mut self.0)
        }
    }

    impl AsyncRead for Stream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<IoResult<()>> {
            Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for Stream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<IoResult<usize>> {
            Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
            Pin::new(&mut self.get_mut().0).poll_flush(cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
            let stream = &mut self.get_mut().0;
            ready!(Pin::new(&mut *stream).poll_flush(cx))?;
            // `Compat<Async<_>>` maps shutdown to `poll_close()`, which only
            // flushes for `async_io::Async`; call the socket shutdown directly.
            stream.get_ref().get_ref().shutdown(Shutdown::Write)?;
            Poll::Ready(Ok(()))
        }
    }

    unsafe impl async_io::IoSafe for WindowsUnixListener {}
    unsafe impl async_io::IoSafe for WindowsUnixStream {}
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use pretty_assertions::assert_eq;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    use super::*;

    #[tokio::test]
    async fn prepare_private_socket_directory_creates_directory() {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let socket_dir = temp_dir.path().join("app-server-control");

        prepare_private_socket_directory(&socket_dir).await.expect("socket dir should be created");

        assert!(socket_dir.is_dir());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn prepare_private_socket_directory_sets_existing_permissions_to_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        for mode in [0o755, 0o600] {
            let socket_dir = temp_dir.path().join(format!("app-server-control-{mode:o}"));
            std::fs::create_dir(&socket_dir).expect("socket dir should be created");
            std::fs::set_permissions(&socket_dir, std::fs::Permissions::from_mode(mode))
                .expect("socket dir permissions should be changed");

            prepare_private_socket_directory(&socket_dir)
                .await
                .expect("socket dir permissions should be set exactly");

            let mode =
                std::fs::metadata(&socket_dir).expect("socket dir metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn regular_file_path_is_not_stale_socket_path() {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let regular_file = temp_dir.path().join("not-a-socket");
        std::fs::write(&regular_file, b"not a socket").expect("regular file should be created");

        assert!(
            !is_stale_socket_path(&regular_file).await.expect("stale socket check should succeed")
        );
    }

    #[tokio::test]
    async fn bound_listener_path_is_stale_socket_path() {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let socket_path = temp_dir.path().join("socket");
        let _listener = match UnixListener::bind(&socket_path).await {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping test: failed to bind unix socket: {err}");
                return;
            }
            Err(err) => panic!("failed to bind test socket: {err}"),
        };

        assert!(
            is_stale_socket_path(&socket_path).await.expect("stale socket check should succeed")
        );
    }

    #[tokio::test]
    async fn stream_round_trips_data_between_listener_and_client() {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        let socket_path = temp_dir.path().join("socket");
        let mut listener = match UnixListener::bind(&socket_path).await {
            Ok(listener) => listener,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping test: failed to bind unix socket: {err}");
                return;
            }
            Err(err) => panic!("failed to bind test socket: {err}"),
        };

        let server_task = tokio::spawn(async move {
            let mut server_stream = listener.accept().await.expect("connection should accept");
            let mut request = [0; 7];
            server_stream.read_exact(&mut request).await.expect("server should read request");
            assert_eq!(&request, b"request");
            server_stream.write_all(b"response").await.expect("server should write response");
        });

        let mut client_stream =
            UnixStream::connect(&socket_path).await.expect("client should connect");
        client_stream.write_all(b"request").await.expect("client should write request");
        let mut response = [0; 8];
        client_stream.read_exact(&mut response).await.expect("client should read response");
        assert_eq!(&response, b"response");

        server_task.await.expect("server task should join");
    }
}
