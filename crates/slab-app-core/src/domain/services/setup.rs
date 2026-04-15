use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use futures::StreamExt;
use slab_types::RuntimeBackendId;
use slab_utils::cab::{
    PackagedPayloadManifest, RuntimeVariant, apply_payload_manifest, detect_best_variant,
    expand_cab_with_progress, remove_dir_if_exists, selected_packages,
};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};
use uuid::Uuid;

use crate::context::worker_state::OperationContext;
use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, CompleteSetupCommand, ComponentStatus, EnvironmentStatus, TaskProgress,
    TaskStatus,
};
use crate::error::AppCoreError;
use crate::infra::db::repository::config::ConfigStore;
use crate::runtime_supervisor::{RuntimeBackendRuntimeStatus, RuntimeSupervisorControlHandle};

const SETUP_INITIALIZED_CONFIG_KEY: &str = "setup_initialized";
const SETUP_INITIALIZED_CONFIG_NAME: &str = "Setup Initialized";
const SETUP_PROVISION_TASK_TYPE: &str = "setup_provision";
const GITHUB_RELEASE_OWNER: &str = "Cyberhan123";
const GITHUB_RELEASE_REPO: &str = "slab.rs";
const PAYLOAD_CACHE_DIR_NAME: &str = "payload-cache";
const RUNTIME_READY_TIMEOUT: Duration = Duration::from_secs(60);
const RUNTIME_READY_POLL_INTERVAL: Duration = Duration::from_millis(500);
const PROVISION_STEP_COUNT: u32 = 6;
const DOWNLOAD_PROGRESS_DELTA_BYTES: u64 = 1024 * 1024 * 4;
const EMBEDDED_PAYLOAD_MANIFEST_JSON: &str =
    include_str!(concat!(env!("OUT_DIR"), "/payload-manifest.json"));

#[repr(u32)]
#[derive(Clone, Copy)]
enum ProvisionStep {
    SelectPayload = 1,
    DownloadPayload = 2,
    ExpandPayload = 3,
    InstallPayload = 4,
    EnsureFfmpeg = 5,
    RestartRuntime = 6,
}

impl ProvisionStep {
    const fn index(self) -> u32 {
        self as u32
    }
}

#[derive(Clone)]
pub struct SetupService {
    model_state: ModelState,
    worker_state: WorkerState,
    runtime_control: Option<RuntimeSupervisorControlHandle>,
}

impl SetupService {
    pub fn new(
        model_state: ModelState,
        worker_state: WorkerState,
        runtime_control: Option<RuntimeSupervisorControlHandle>,
    ) -> Self {
        Self { model_state, worker_state, runtime_control }
    }

    pub async fn environment_status(&self) -> Result<EnvironmentStatus, AppCoreError> {
        let initialized = self.load_setup_initialized().await?;
        let ffmpeg_installed =
            tokio::task::spawn_blocking(ffmpeg_sidecar::command::ffmpeg_is_installed)
                .await
                .unwrap_or(false);

        let ffmpeg_version = if ffmpeg_installed {
            tokio::task::spawn_blocking(|| ffmpeg_sidecar::version::ffmpeg_version().ok())
                .await
                .unwrap_or(None)
        } else {
            None
        };

        let backends: Vec<ComponentStatus> = RuntimeBackendId::ALL
            .into_iter()
            .map(|backend| {
                let installed = self.model_state.runtime_status().status(backend)
                    == RuntimeBackendRuntimeStatus::Ready;
                ComponentStatus { name: backend.to_string(), installed, version: None }
            })
            .collect();

        Ok(EnvironmentStatus {
            initialized,
            ffmpeg: ComponentStatus {
                name: "ffmpeg".to_owned(),
                installed: ffmpeg_installed,
                version: ffmpeg_version,
            },
            backends,
        })
    }

    pub async fn provision(&self) -> Result<AcceptedOperation, AppCoreError> {
        let service = self.clone();
        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending(SETUP_PROVISION_TASK_TYPE, None, None),
                move |operation| async move {
                    service.run_provision(operation).await;
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }

    pub async fn complete_setup(
        &self,
        cmd: CompleteSetupCommand,
    ) -> Result<EnvironmentStatus, AppCoreError> {
        self.persist_setup_initialized(cmd.initialized).await?;

        info!(initialized = cmd.initialized, "setup state persisted");
        self.environment_status().await
    }

    async fn run_provision(self, operation: OperationContext) {
        let operation_id = operation.id().to_owned();

        if let Err(error) = operation.mark_running().await {
            warn!(task_id = %operation_id, error = %error, "failed to mark setup_provision running");
            return;
        }

        let result = self.provision_inner(&operation).await;
        match result {
            Ok(payload) => {
                if let Err(error) = operation.mark_succeeded(&payload).await {
                    warn!(task_id = %operation_id, error = %error, "failed to persist setup_provision success");
                }
            }
            Err(error) => {
                let message = error.to_string();
                warn!(task_id = %operation_id, error = %message, "setup provision failed");
                if let Err(db_error) = operation.mark_failed(&message).await {
                    warn!(task_id = %operation_id, error = %db_error, "failed to persist setup_provision failure");
                }
            }
        }
    }

    async fn provision_inner(&self, operation: &OperationContext) -> Result<String, AppCoreError> {
        let manifest = embedded_payload_manifest()?;
        ensure_packaged_payload_manifest_available(&manifest)?;
        let variant = tokio::task::spawn_blocking(detect_best_variant)
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("failed to join GPU detection task: {error}"))
            })?
            .map_err(AppCoreError::from)?;
        let selected_variants = selected_packages(variant);
        let selected_manifest =
            manifest.selected_for(&selected_variants).map_err(AppCoreError::from)?;
        let version = manifest.version.clone();
        let target_dir = self.runtime_lib_dir()?;
        let download_cache_dir = self.payload_cache_dir(&version);
        let expand_root = std::env::temp_dir()
            .join("Slab")
            .join("setup")
            .join(format!("payload-{}", Uuid::new_v4()));

        publish_stage_progress(
            operation,
            ProvisionStep::SelectPayload,
            "Selecting runtime payload",
        )
        .await?;

        tokio::fs::create_dir_all(&download_cache_dir).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create payload cache directory '{}': {error}",
                download_cache_dir.display()
            ))
        })?;
        tokio::fs::create_dir_all(&expand_root).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create payload staging directory '{}': {error}",
                expand_root.display()
            ))
        })?;

        let payload_result = async {
            self.download_required_cabs(
                operation,
                &version,
                &selected_variants,
                &download_cache_dir,
            )
            .await?;

            publish_progress(
                operation,
                "Expanding runtime payload",
                0,
                Some(100),
                ProvisionStep::ExpandPayload.index(),
                PROVISION_STEP_COUNT,
            )
            .await?;
            self.expand_selected_cabs(&selected_variants, &download_cache_dir, &expand_root)
                .await?;

            publish_progress(
                operation,
                "Installing runtime libraries",
                0,
                Some(100),
                ProvisionStep::InstallPayload.index(),
                PROVISION_STEP_COUNT,
            )
            .await?;
            self.apply_selected_payload(&expand_root, &target_dir, &selected_manifest).await?;

            publish_progress(
                operation,
                "Checking FFmpeg",
                0,
                Some(100),
                ProvisionStep::EnsureFfmpeg.index(),
                PROVISION_STEP_COUNT,
            )
            .await?;
            let ffmpeg_path = self.ensure_ffmpeg_installed().await?;

            publish_progress(
                operation,
                "Restarting runtime workers",
                0,
                Some(100),
                ProvisionStep::RestartRuntime.index(),
                PROVISION_STEP_COUNT,
            )
            .await?;
            let restarted_backends = self.restart_runtime_backends().await?;
            self.wait_for_backends_ready(&restarted_backends).await?;

            self.persist_setup_initialized(true).await?;
            Ok::<PathBuf, AppCoreError>(ffmpeg_path)
        }
        .await;

        let _ = remove_dir_if_exists(&expand_root);

        let ffmpeg_path = payload_result?;
        let backends = self
            .runtime_control
            .as_ref()
            .map(|control| {
                control
                    .managed_backends()
                    .into_iter()
                    .filter(|backend| backend.is_runtime_worker_backend())
                    .map(|backend| backend.canonical_id().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "initialized": true,
            "variant": variant.as_str(),
            "version": version,
            "ffmpeg_path": ffmpeg_path,
            "target_dir": target_dir,
            "backends": backends,
        })
        .to_string())
    }

    async fn download_required_cabs(
        &self,
        operation: &OperationContext,
        version: &str,
        variants: &[RuntimeVariant],
        download_cache_dir: &Path,
    ) -> Result<(), AppCoreError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("slab-setup/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| {
                AppCoreError::Internal(format!("failed to build HTTP client: {error}"))
            })?;

        let download_progress_total = ((variants.len() as u64).max(1)).saturating_mul(100);

        for (index, variant) in variants.iter().enumerate() {
            let progress_base = (index as u64).saturating_mul(100);
            let cab_name = variant.cab_name();
            let cab_path = download_cache_dir.join(cab_name);
            if cab_path.is_file() {
                publish_progress(
                    operation,
                    format!("Using cached {cab_name}"),
                    progress_base.saturating_add(100),
                    Some(download_progress_total),
                    ProvisionStep::DownloadPayload.index(),
                    PROVISION_STEP_COUNT,
                )
                .await?;
                continue;
            }

            let url = github_release_asset_url(version, cab_name);
            download_cab_with_progress(
                &client,
                &url,
                &cab_path,
                operation,
                progress_base,
                download_progress_total,
            )
            .await?;
        }

        Ok(())
    }

    async fn expand_selected_cabs(
        &self,
        variants: &[RuntimeVariant],
        download_cache_dir: &Path,
        expand_root: &Path,
    ) -> Result<(), AppCoreError> {
        for variant in variants {
            let cab_path = download_cache_dir.join(variant.cab_name());
            let expand_root = expand_root.to_path_buf();
            tokio::task::spawn_blocking(move || {
                expand_cab_with_progress(&cab_path, &expand_root, |_bytes| Ok(()))
            })
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("failed to join CAB extraction task: {error}"))
            })?
            .map_err(AppCoreError::from)?;
        }

        Ok(())
    }

    async fn apply_selected_payload(
        &self,
        source_root: &Path,
        target_dir: &Path,
        manifest: &slab_utils::cab::SelectedPayloadManifest,
    ) -> Result<(), AppCoreError> {
        let source_root = source_root.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        let manifest = manifest.clone();
        tokio::task::spawn_blocking(move || {
            apply_payload_manifest(&source_root, &target_dir, &manifest)
        })
        .await
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to join payload apply task: {error}"))
        })?
        .map_err(AppCoreError::from)
    }

    async fn ensure_ffmpeg_installed(&self) -> Result<PathBuf, AppCoreError> {
        let configured_dir = self.model_state.pmid().config().setup.ffmpeg.dir;

        tokio::task::spawn_blocking(move || -> anyhow::Result<PathBuf> {
            if ffmpeg_sidecar::command::ffmpeg_is_installed() {
                return Ok(ffmpeg_sidecar::paths::ffmpeg_path());
            }

            let download_url = ffmpeg_sidecar::download::ffmpeg_download_url()?;
            let destination = match configured_dir.as_deref().filter(|dir| !dir.trim().is_empty()) {
                Some(dir) => {
                    let dir = PathBuf::from(dir);
                    std::fs::create_dir_all(&dir)?;
                    dir
                }
                None => ffmpeg_sidecar::paths::sidecar_dir()?,
            };

            let archive =
                ffmpeg_sidecar::download::download_ffmpeg_package(download_url, &destination)?;
            ffmpeg_sidecar::download::unpack_ffmpeg(&archive, &destination)?;
            let _ = std::fs::remove_file(&archive);

            Ok(ffmpeg_sidecar::paths::ffmpeg_path())
        })
        .await
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to join FFmpeg install task: {error}"))
        })?
        .map_err(AppCoreError::from)
    }

    async fn restart_runtime_backends(&self) -> Result<Vec<RuntimeBackendId>, AppCoreError> {
        let Some(control) = self.runtime_control.as_ref() else {
            return Err(AppCoreError::Internal(
                "setup provision requires runtime supervisor control".to_owned(),
            ));
        };

        let managed_ggml_backends: Vec<_> = control
            .managed_backends()
            .into_iter()
            .filter(|backend| backend.is_runtime_worker_backend())
            .collect();
        control.restart_backends(&managed_ggml_backends)
    }

    async fn wait_for_backends_ready(
        &self,
        backends: &[RuntimeBackendId],
    ) -> Result<(), AppCoreError> {
        if backends.is_empty() {
            return Ok(());
        }

        let deadline = tokio::time::Instant::now() + RUNTIME_READY_TIMEOUT;
        loop {
            let all_ready = backends.iter().all(|backend| {
                self.model_state.runtime_status().status(*backend)
                    == RuntimeBackendRuntimeStatus::Ready
            });
            if all_ready {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                let statuses = backends
                    .iter()
                    .map(|backend| {
                        format!(
                            "{}={}",
                            backend.canonical_id(),
                            self.model_state.runtime_status().status(*backend).as_str()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(AppCoreError::Internal(format!(
                    "timed out waiting for runtime backends to become ready: {statuses}"
                )));
            }

            tokio::time::sleep(RUNTIME_READY_POLL_INTERVAL).await;
        }
    }

    async fn load_setup_initialized(&self) -> Result<bool, AppCoreError> {
        let raw = self.model_state.store().get_config_value(SETUP_INITIALIZED_CONFIG_KEY).await?;

        match raw.as_deref().map(str::trim) {
            None | Some("") => Ok(false),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(other) => Err(AppCoreError::Internal(format!(
                "config_store key '{}' contains invalid boolean value '{}'",
                SETUP_INITIALIZED_CONFIG_KEY, other
            ))),
        }
    }

    async fn persist_setup_initialized(&self, initialized: bool) -> Result<(), AppCoreError> {
        let value = if initialized { "true" } else { "false" };
        self.model_state
            .store()
            .set_config_entry(
                SETUP_INITIALIZED_CONFIG_KEY,
                Some(SETUP_INITIALIZED_CONFIG_NAME),
                value,
            )
            .await?;
        Ok(())
    }

    fn runtime_lib_dir(&self) -> Result<PathBuf, AppCoreError> {
        self.model_state.config().lib_dir.clone().ok_or_else(|| {
            AppCoreError::Internal(
                "desktop setup provisioning requires a resolved resources/libs target directory"
                    .to_owned(),
            )
        })
    }

    fn payload_cache_dir(&self, version: &str) -> PathBuf {
        self.model_state
            .config()
            .settings_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir().join("Slab"))
            .join(PAYLOAD_CACHE_DIR_NAME)
            .join(version)
    }
}

fn embedded_payload_manifest() -> Result<PackagedPayloadManifest, AppCoreError> {
    static MANIFEST: OnceLock<Result<PackagedPayloadManifest, String>> = OnceLock::new();

    MANIFEST
        .get_or_init(|| {
            serde_json::from_str(EMBEDDED_PAYLOAD_MANIFEST_JSON).map_err(|error| error.to_string())
        })
        .clone()
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to load embedded payload manifest: {error}"))
        })
}

fn ensure_packaged_payload_manifest_available(
    manifest: &PackagedPayloadManifest,
) -> Result<(), AppCoreError> {
    if manifest.is_empty() {
        return Err(AppCoreError::NotImplemented(format!(
            "setup provisioning is only available in builds that embed packaged runtime payloads; the current '{}' build does not include them",
            std::env::consts::OS
        )));
    }

    Ok(())
}

async fn publish_progress(
    operation: &OperationContext,
    label: impl Into<String>,
    current: u64,
    total: Option<u64>,
    step: u32,
    step_count: u32,
) -> Result<(), AppCoreError> {
    let payload = serde_json::json!({
        "progress": TaskProgress {
            label: Some(label.into()),
            current,
            total,
            unit: None,
            step: Some(step),
            step_count: Some(step_count),
        }
    })
    .to_string();

    operation.update_status(TaskStatus::Running, Some(&payload), None).await
}

async fn publish_stage_progress(
    operation: &OperationContext,
    step: ProvisionStep,
    label: impl Into<String>,
) -> Result<(), AppCoreError> {
    publish_progress(operation, label, 0, Some(100), step.index(), PROVISION_STEP_COUNT).await
}

async fn download_cab_with_progress(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    operation: &OperationContext,
    progress_base: u64,
    progress_total: u64,
) -> Result<(), AppCoreError> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create CAB download directory '{}': {error}",
                parent.display()
            ))
        })?;
    }

    let response = client.get(url).send().await.map_err(|error| {
        AppCoreError::Internal(format!("failed to download CAB from '{url}': {error}"))
    })?;
    let response = response.error_for_status().map_err(|error| {
        AppCoreError::Internal(format!("failed to download CAB from '{url}': {error}"))
    })?;
    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let tmp_path = destination.with_extension(format!(
        "{}.part",
        destination.extension().and_then(|ext| ext.to_str()).unwrap_or("cab")
    ));
    let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create staged CAB '{}': {error}",
            tmp_path.display()
        ))
    })?;
    let mut downloaded = 0_u64;
    let mut last_reported = 0_u64;
    let file_name =
        destination.file_name().and_then(|name| name.to_str()).unwrap_or("payload.cab").to_owned();

    publish_progress(
        operation,
        format!("Downloading {file_name}"),
        progress_base,
        Some(progress_total),
        ProvisionStep::DownloadPayload.index(),
        PROVISION_STEP_COUNT,
    )
    .await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            AppCoreError::Internal(format!("failed while streaming CAB '{url}': {error}"))
        })?;
        file.write_all(&chunk).await.map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to write staged CAB '{}': {error}",
                tmp_path.display()
            ))
        })?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded.saturating_sub(last_reported) >= DOWNLOAD_PROGRESS_DELTA_BYTES
            || total.is_some_and(|expected| downloaded >= expected)
        {
            let current = total
                .map(|expected| {
                    progress_base.saturating_add(
                        ((downloaded.saturating_mul(100)) / expected.max(1)).min(99),
                    )
                })
                .unwrap_or(progress_base);
            publish_progress(
                operation,
                format!("Downloading {file_name}"),
                current,
                Some(progress_total),
                ProvisionStep::DownloadPayload.index(),
                PROVISION_STEP_COUNT,
            )
            .await?;
            last_reported = downloaded;
        }
    }

    file.flush().await.map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to flush staged CAB '{}': {error}",
            tmp_path.display()
        ))
    })?;
    drop(file);

    tokio::fs::rename(&tmp_path, destination).await.map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to move staged CAB '{}' into '{}': {error}",
            tmp_path.display(),
            destination.display()
        ))
    })?;

    publish_progress(
        operation,
        format!("Downloaded {file_name}"),
        progress_base.saturating_add(100),
        Some(progress_total),
        ProvisionStep::DownloadPayload.index(),
        PROVISION_STEP_COUNT,
    )
    .await?;

    Ok(())
}

fn github_release_asset_url(version: &str, asset_name: &str) -> String {
    format!(
        "https://github.com/{GITHUB_RELEASE_OWNER}/{GITHUB_RELEASE_REPO}/releases/download/v{version}/{asset_name}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_provisioning_rejects_empty_embedded_manifest() {
        let error =
            ensure_packaged_payload_manifest_available(&PackagedPayloadManifest::empty("0.1.0"))
                .expect_err("empty payload manifests should not allow setup provisioning");

        assert!(matches!(error, AppCoreError::NotImplemented(_)));
        assert!(error.to_string().contains("setup provisioning"));
    }

    #[test]
    fn setup_provisioning_accepts_non_empty_manifest() {
        let mut manifest = PackagedPayloadManifest::empty("0.1.0");
        manifest.packages.push(slab_utils::cab::PackagedPayloadPackage {
            variant: RuntimeVariant::Base,
            cab_name: RuntimeVariant::Base.cab_name().to_owned(),
            files: Vec::new(),
        });

        assert!(ensure_packaged_payload_manifest_available(&manifest).is_ok());
    }
}
