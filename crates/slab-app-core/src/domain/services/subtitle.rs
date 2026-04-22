use std::path::{Path, PathBuf};

use slab_subtitle::timetypes::{TimePoint, TimeSpan};
use slab_subtitle::{SrtFile, SubtitleFileInterface};

use crate::domain::models::{RenderSubtitleCommand, RenderSubtitleResult};
use crate::error::AppCoreError;

#[derive(Clone, Default)]
pub struct SubtitleService;

impl SubtitleService {
    pub fn new() -> Self {
        Self
    }

    pub async fn render(
        &self,
        command: RenderSubtitleCommand,
    ) -> Result<RenderSubtitleResult, AppCoreError> {
        let output_path = resolve_output_path(&command)?;
        if tokio::fs::try_exists(&output_path).await.unwrap_or(false) && !command.overwrite {
            return Err(AppCoreError::BadRequest(format!(
                "subtitle output already exists: {}",
                output_path.display()
            )));
        }

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create subtitle output directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        let entries = command
            .entries
            .iter()
            .map(|entry| {
                (
                    TimeSpan::new(
                        TimePoint::from_msecs(i64::try_from(entry.start_ms).unwrap_or(i64::MAX)),
                        TimePoint::from_msecs(i64::try_from(entry.end_ms).unwrap_or(i64::MAX)),
                    ),
                    entry.text.clone(),
                )
            })
            .collect::<Vec<_>>();
        let entry_count = entries.len();
        let srt = SrtFile::create(entries)
            .map_err(|error| AppCoreError::BadRequest(format!("failed to create SRT: {error}")))?;
        let bytes = srt
            .to_data()
            .map_err(|error| AppCoreError::BadRequest(format!("failed to encode SRT: {error}")))?;

        tokio::fs::write(&output_path, bytes).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to write subtitle file {}: {error}",
                output_path.display()
            ))
        })?;

        Ok(RenderSubtitleResult {
            output_path: output_path.to_string_lossy().into_owned(),
            format: "srt".to_owned(),
            entry_count,
        })
    }
}

fn resolve_output_path(command: &RenderSubtitleCommand) -> Result<PathBuf, AppCoreError> {
    if let Some(output_path) =
        command.output_path.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        return Ok(PathBuf::from(output_path));
    }

    let source_path = Path::new(&command.source_path);
    let parent = source_path.parent().ok_or_else(|| {
        AppCoreError::BadRequest(format!("source_path has no parent: {}", source_path.display()))
    })?;
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "source_path has no usable file stem: {}",
                source_path.display()
            ))
        })?;

    Ok(parent.join(format!("{}.{}.srt", stem, command.variant.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::{RenderSubtitleEntry, SubtitleVariant};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_output_path_uses_source_parent_stem_and_variant() {
        let command = RenderSubtitleCommand {
            source_path: "/tmp/movie.mp4".to_owned(),
            variant: SubtitleVariant::Translated,
            entries: vec![RenderSubtitleEntry {
                start_ms: 0,
                end_ms: 1000,
                text: "Hello".to_owned(),
            }],
            output_path: None,
            overwrite: true,
        };

        let output = resolve_output_path(&command).expect("default path");
        assert_eq!(output, PathBuf::from("/tmp/movie.translated.srt"));
    }

    #[tokio::test]
    async fn render_writes_srt_file_contents() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("slab-subtitle-render-{suffix}"));
        tokio::fs::create_dir_all(&root).await.unwrap();

        let source_path = root.join("clip.mp4");
        let output_path = root.join("clip.source.srt");
        let service = SubtitleService::new();

        let result = service
            .render(RenderSubtitleCommand {
                source_path: source_path.to_string_lossy().into_owned(),
                variant: SubtitleVariant::Source,
                entries: vec![RenderSubtitleEntry {
                    start_ms: 0,
                    end_ms: 1500,
                    text: "Hello world".to_owned(),
                }],
                output_path: None,
                overwrite: true,
            })
            .await
            .expect("render succeeds");

        let written = tokio::fs::read_to_string(&output_path).await.expect("srt file exists");
        assert_eq!(result.output_path, output_path.to_string_lossy());
        assert!(written.contains("00:00:00,000 --> 00:00:01,500"));
        assert!(written.contains("Hello world"));

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn render_rejects_existing_file_when_overwrite_is_false() {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("slab-subtitle-overwrite-{suffix}"));
        tokio::fs::create_dir_all(&root).await.unwrap();

        let output_path = root.join("clip.source.srt");
        tokio::fs::write(&output_path, "existing").await.unwrap();
        let service = SubtitleService::new();

        let error = service
            .render(RenderSubtitleCommand {
                source_path: root.join("clip.mp4").to_string_lossy().into_owned(),
                variant: SubtitleVariant::Source,
                entries: vec![RenderSubtitleEntry {
                    start_ms: 0,
                    end_ms: 1000,
                    text: "Hello".to_owned(),
                }],
                output_path: Some(output_path.to_string_lossy().into_owned()),
                overwrite: false,
            })
            .await
            .expect_err("existing file should be rejected");

        assert!(
            matches!(error, AppCoreError::BadRequest(message) if message.contains("already exists"))
        );

        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
