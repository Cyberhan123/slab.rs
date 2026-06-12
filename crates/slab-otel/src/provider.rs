use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use anyhow::Context;
use tracing::Subscriber;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::registry::LookupSpan;

use crate::config::{OtelExporter, OtelSettings};

#[derive(Debug, Clone, Copy)]
struct NoopLayer;

impl<S> Layer<S> for NoopLayer where S: Subscriber {}

/// Holds telemetry exporter state and non-blocking writer guards.
#[derive(Debug)]
pub struct OtelProvider {
    settings: OtelSettings,
    logger_writer: Option<NonBlocking>,
    tracing_writer: Option<NonBlocking>,
    _guards: Vec<WorkerGuard>,
}

impl OtelProvider {
    pub fn from(settings: &OtelSettings) -> anyhow::Result<Option<Self>> {
        if !settings.enabled {
            crate::metrics::set_enabled(false);
            return Ok(None);
        }

        let mut guards = Vec::new();
        let logger_writer =
            local_writer(&settings.exporter, &settings.service_name, "log", &mut guards)?;
        let tracing_writer = local_writer(
            &settings.trace_exporter,
            &settings.service_name,
            "traces.log",
            &mut guards,
        )?;

        crate::metrics::set_enabled(settings.metrics_exporter.is_enabled());

        Ok(Some(Self {
            settings: settings.clone(),
            logger_writer,
            tracing_writer,
            _guards: guards,
        }))
    }

    pub fn settings(&self) -> &OtelSettings {
        &self.settings
    }

    pub fn logger_layer<S>(&self) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
    {
        let Some(writer) = self.logger_writer.clone() else {
            return Box::new(NoopLayer);
        };

        Box::new(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_writer(writer),
        )
    }

    pub fn tracing_layer<S>(&self) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
    {
        let Some(writer) = self.tracing_writer.clone() else {
            return Box::new(NoopLayer);
        };

        Box::new(
            tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_current_span(true)
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(writer),
        )
    }
}

pub fn from_settings(settings: &OtelSettings) -> anyhow::Result<Option<OtelProvider>> {
    OtelProvider::from(settings)
}

pub fn install_log_bridge() {
    let _ = tracing_log::LogTracer::init();
}

fn local_writer(
    exporter: &OtelExporter,
    service_name: &str,
    suffix: &str,
    guards: &mut Vec<WorkerGuard>,
) -> anyhow::Result<Option<NonBlocking>> {
    let Some(directory) = exporter.local_directory() else {
        return Ok(None);
    };
    std::fs::create_dir_all(directory).with_context(|| {
        format!("failed to create OpenTelemetry local export directory '{}'", directory.display())
    })?;
    let path = local_file_path(directory, service_name, suffix);
    let file = OpenOptions::new().create(true).append(true).open(&path).with_context(|| {
        format!("failed to open OpenTelemetry local export '{}'", path.display())
    })?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    guards.push(guard);
    Ok(Some(writer))
}

fn local_file_path(directory: &Path, service_name: &str, suffix: &str) -> PathBuf {
    directory.join(format!("{}.{}", sanitize_file_stem(service_name), suffix))
}

fn sanitize_file_stem(raw: &str) -> String {
    let mut value = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            value.push(ch);
        } else {
            value.push('-');
        }
    }
    let value = value.trim_matches('-');
    if value.is_empty() { "slab".to_owned() } else { value.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OtelExporter, OtelSettings};

    #[test]
    fn disabled_provider_returns_none() {
        let settings = OtelSettings { enabled: false, ..OtelSettings::default() };

        assert!(OtelProvider::from(&settings).expect("provider").is_none());
    }

    #[test]
    fn local_exporter_creates_service_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let settings = OtelSettings {
            service_name: "slab/test".to_owned(),
            exporter: OtelExporter::LocalFile { directory: temp.path().to_path_buf() },
            trace_exporter: OtelExporter::LocalFile { directory: temp.path().to_path_buf() },
            ..OtelSettings::default()
        };

        let provider = OtelProvider::from(&settings).expect("provider").expect("enabled");
        drop(provider);

        assert!(temp.path().join("slab-test.log").exists());
        assert!(temp.path().join("slab-test.traces.log").exists());
    }
}
