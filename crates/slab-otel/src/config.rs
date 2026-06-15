use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_enabled() -> bool {
    true
}

fn default_environment() -> String {
    "dev".to_owned()
}

fn default_service_name() -> String {
    "slab".to_owned()
}

fn default_slab_home() -> PathBuf {
    slab_utils::app_home::app_home_dir()
}

fn default_capture_content() -> bool {
    false
}

fn default_local_file_exporter() -> OtelExporter {
    OtelExporter::LocalFile { directory: default_slab_home().join("logs") }
}

fn is_default_slab_home(value: &PathBuf) -> bool {
    value == &default_slab_home()
}

fn is_default_local_file_exporter(value: &OtelExporter) -> bool {
    value == &default_local_file_exporter()
}

/// HTTP encoding used by OTLP/HTTP exporters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OtelHttpProtocol {
    Binary,
    Json,
}

/// Optional TLS hints for remote OTLP exporters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OtelTlsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_cert_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_name: Option<String>,
}

/// Export target for logs, traces, or metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OtelExporter {
    #[default]
    None,
    LocalFile {
        directory: PathBuf,
    },
    OtlpHttp {
        endpoint: String,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        headers: HashMap<String, String>,
        #[serde(default = "default_http_protocol")]
        protocol: OtelHttpProtocol,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tls: Option<OtelTlsConfig>,
    },
}

impl OtelExporter {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn local_directory(&self) -> Option<&PathBuf> {
        match self {
            Self::LocalFile { directory } => Some(directory),
            Self::None | Self::OtlpHttp { .. } => None,
        }
    }
}

fn default_http_protocol() -> OtelHttpProtocol {
    OtelHttpProtocol::Binary
}

/// Runtime OpenTelemetry settings shared by Slab hosts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OtelSettings {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default = "default_service_name")]
    pub service_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_version: Option<String>,
    #[serde(default = "default_slab_home", skip_serializing_if = "is_default_slab_home")]
    #[schemars(skip)]
    pub slab_home: PathBuf,
    #[serde(
        default = "default_local_file_exporter",
        skip_serializing_if = "is_default_local_file_exporter"
    )]
    #[schemars(skip)]
    pub exporter: OtelExporter,
    #[serde(
        default = "default_local_file_exporter",
        skip_serializing_if = "is_default_local_file_exporter"
    )]
    #[schemars(skip)]
    pub trace_exporter: OtelExporter,
    #[serde(default)]
    pub metrics_exporter: OtelExporter,
    #[serde(default = "default_capture_content")]
    pub capture_content: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub span_attributes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tracestate: BTreeMap<String, String>,
}

impl OtelSettings {
    pub fn default_for_service(service_name: impl Into<String>) -> Self {
        Self { service_name: service_name.into(), ..Self::default() }
    }

    pub fn with_slab_home(mut self, slab_home: impl Into<PathBuf>) -> Self {
        let slab_home = slab_home.into();
        if self.exporter == default_local_file_exporter() {
            self.exporter = OtelExporter::LocalFile { directory: slab_home.join("logs") };
        }
        if self.trace_exporter == default_local_file_exporter() {
            self.trace_exporter = OtelExporter::LocalFile { directory: slab_home.join("logs") };
        }
        self.slab_home = slab_home;
        self
    }
}

impl Default for OtelSettings {
    fn default() -> Self {
        let slab_home = default_slab_home();
        let logs_dir = slab_home.join("logs");
        Self {
            enabled: true,
            environment: default_environment(),
            service_name: default_service_name(),
            service_version: None,
            slab_home,
            exporter: OtelExporter::LocalFile { directory: logs_dir.clone() },
            trace_exporter: OtelExporter::LocalFile { directory: logs_dir },
            metrics_exporter: OtelExporter::None,
            capture_content: false,
            span_attributes: BTreeMap::new(),
            tracestate: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_local_file_export_without_content_capture() {
        let settings = OtelSettings::default();

        assert!(settings.enabled);
        assert!(!settings.capture_content);
        assert!(matches!(settings.exporter, OtelExporter::LocalFile { .. }));
        assert!(matches!(settings.trace_exporter, OtelExporter::LocalFile { .. }));
        assert_eq!(settings.metrics_exporter, OtelExporter::None);
        assert!(settings.slab_home.ends_with(slab_utils::app_home::APP_ID));
    }

    #[test]
    fn default_serialization_omits_runtime_resolved_paths() {
        let value = serde_json::to_value(OtelSettings::default()).expect("settings json");
        let object = value.as_object().expect("settings object");

        assert!(!object.contains_key("slab_home"));
        assert!(!object.contains_key("exporter"));
        assert!(!object.contains_key("trace_exporter"));
    }

    #[test]
    fn exporter_shape_round_trips() {
        let raw = serde_json::json!({
            "type": "otlp_http",
            "endpoint": "https://otlp.example.com",
            "headers": { "x-api-key": "secret" },
            "protocol": "json",
            "tls": { "domain_name": "otlp.example.com" }
        });

        let exporter: OtelExporter = serde_json::from_value(raw.clone()).expect("exporter");
        assert_eq!(serde_json::to_value(exporter).expect("json"), raw);
    }

    #[test]
    fn custom_slab_home_moves_default_local_exporters() {
        let temp = tempfile::tempdir().expect("temp dir");
        let settings = OtelSettings::default().with_slab_home(temp.path());

        assert_eq!(settings.slab_home, temp.path());
        assert_eq!(settings.exporter.local_directory(), Some(&temp.path().join("logs")));
        assert_eq!(settings.trace_exporter.local_directory(), Some(&temp.path().join("logs")));
    }
}
