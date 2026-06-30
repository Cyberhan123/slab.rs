use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::log_redaction::redact_log_text;

pub(crate) const DEFAULT_MAX_LOG_BYTES: u64 = 50 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_LOG_FILES: usize = 5;

#[derive(Clone)]
pub(crate) struct RedactingSizeRotatingWriter {
    inner: Arc<Mutex<SizeRotatingLogFile>>,
}

impl RedactingSizeRotatingWriter {
    pub(crate) fn new(
        path: impl Into<PathBuf>,
        max_bytes: u64,
        max_files: usize,
    ) -> io::Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(SizeRotatingLogFile::new(
                path.into(),
                max_bytes,
                max_files,
            )?)),
        })
    }
}

impl Write for RedactingSizeRotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let redacted = redact_log_text(&text);
        self.inner.lock().unwrap().write_redacted(redacted.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.lock().unwrap().flush()
    }
}

pub(crate) struct SizeRotatingLogFile {
    path: PathBuf,
    max_bytes: u64,
    max_files: usize,
    file: File,
    bytes_written: u64,
}

impl SizeRotatingLogFile {
    pub(crate) fn new(path: PathBuf, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes_written = file.metadata()?.len();

        Ok(Self {
            path,
            max_bytes: max_bytes.max(1),
            max_files: max_files.max(1),
            file,
            bytes_written,
        })
    }

    pub(crate) fn write_redacted(&mut self, mut buf: &[u8]) -> io::Result<()> {
        while !buf.is_empty() {
            if self.bytes_written >= self.max_bytes {
                self.rotate()?;
            }

            let remaining = self.max_bytes.saturating_sub(self.bytes_written);
            let chunk_len = buf.len().min(remaining as usize);
            if chunk_len == 0 {
                self.rotate()?;
                continue;
            }

            let (chunk, rest) = buf.split_at(chunk_len);
            self.file.write_all(chunk)?;
            self.bytes_written += chunk.len() as u64;
            buf = rest;
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let max_rotated_files = self.max_files.saturating_sub(1);
        for index in (1..=max_rotated_files).rev() {
            let source = rotated_path(&self.path, index);
            if !source.exists() {
                continue;
            }
            if index == max_rotated_files {
                remove_file_if_exists(&source)?;
                continue;
            }
            let target = rotated_path(&self.path, index + 1);
            rename_replace(&source, &target)?;
        }

        if max_rotated_files == 0 {
            remove_file_if_exists(&self.path)?;
        } else {
            let first_rotated = rotated_path(&self.path, 1);
            remove_file_if_exists(&first_rotated)?;
            if self.path.exists() {
                rename_replace(&self.path, &first_rotated)?;
            }
        }

        self.file = OpenOptions::new().create(true).write(true).truncate(true).open(&self.path)?;
        self.bytes_written = 0;
        Ok(())
    }
}

fn rename_replace(source: &Path, target: &Path) -> io::Result<()> {
    remove_file_if_exists(target)?;
    fs::rename(source, target)
}

fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let extension = format!("log.{index}");
    path.with_extension(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_by_size_and_retains_bounded_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let log = temp.path().join("slab-server.log");
        let mut file = SizeRotatingLogFile::new(log.clone(), 10, 3).expect("log file");

        file.write_redacted(b"1234567890\n").expect("write first");
        file.write_redacted(b"abcdefghij\n").expect("write second");
        file.write_redacted(b"klmnopqrst\n").expect("write third");
        file.write_redacted(b"uvwxyz1234\n").expect("write fourth");
        file.flush().expect("flush");

        assert!(log.exists());
        assert!(log.with_extension("log.1").exists());
        assert!(log.with_extension("log.2").exists());
        assert!(!log.with_extension("log.3").exists());
    }

    #[test]
    fn splits_large_writes_across_rotated_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let log = temp.path().join("slab-server.log");
        let mut file = SizeRotatingLogFile::new(log.clone(), 5, 3).expect("log file");

        file.write_redacted(b"abcdefghijkl").expect("write large record");
        file.flush().expect("flush");

        assert!(fs::metadata(&log).expect("current").len() <= 5);
        assert!(fs::metadata(log.with_extension("log.1")).expect("first rotated").len() <= 5);
        assert!(fs::metadata(log.with_extension("log.2")).expect("second rotated").len() <= 5);
    }

    #[test]
    fn redacting_writer_reports_original_bytes_and_masks_file_content() {
        let temp = tempfile::tempdir().expect("temp dir");
        let log = temp.path().join("slab-server.log");
        let mut writer = RedactingSizeRotatingWriter::new(log.clone(), 1024, 5).expect("writer");
        let input = b"token=secret-value\n";

        let written = writer.write(input).expect("write");
        writer.flush().expect("flush");

        let output = fs::read_to_string(log).expect("read log");
        assert_eq!(written, input.len());
        assert!(output.contains("token=<redacted>"));
        assert!(!output.contains("secret-value"));
    }
}
