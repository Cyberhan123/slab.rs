//! `slab-agent-context` host adapter.
//!
//! [`AppContextSources`] is the app-core implementation of the
//! [`slab_agent_context::AgentContextSources`] port used by
//! [`slab_agent_context::ContextInstructionHook`]. It reads the workspace root
//! from the settings config (mirroring the other agent hooks/tools), resolves a
//! model's instruction template through the model service, snapshots the
//! environment + permission state, and bridges the folded read-side memory
//! context — keeping `slab-agent-context` free of `slab-exec-policy` and
//! `slab-agent-memories`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent_context::{
    AgentContextSources, EnvironmentSnapshot, MemoryContext, OsKind, PermissionBaselineLabel,
    PermissionModeLabel, PermissionSnapshot, ShellKind,
};
use slab_agent_memories::{fs as memory_fs, recall, templates as memory_templates};

use crate::context::ModelState;
use crate::domain::services::{resolve_local_chat_prompt_profile, workspace_root_from_config};

use super::memory::memory_chat_json;
use super::memory_project::resolve_project_key;

/// Side-query wall-clock cap: recall must never stall (or crash) agent start —
/// on timeout/error the fragment is simply skipped and the next run retries.
const RECALL_QUERY_TIMEOUT_SECS: u64 = 20;
/// A thread's recall selection is reused while the memory manifest is
/// unchanged and the entry is fresh — the surfaced-set dedup that keeps
/// repeat turns from re-querying (and re-inflating context).
const RECALL_CACHE_TTL_SECS: u64 = 600;
const RECALL_CACHE_MAX_ENTRIES: usize = 256;

#[derive(Clone)]
struct RecallCacheEntry {
    project_key: String,
    manifest_watermark: SystemTime,
    body: String,
    created_at: Instant,
}

/// Map the configured shell launcher to the context crate's shell family. `Auto`
/// follows the same fallback `slab-shell-command` uses (PowerShell on Windows,
/// bash elsewhere) — the model does not need the resolved bash path.
pub(crate) fn shell_kind(launcher: slab_config::ShellLauncherKind) -> ShellKind {
    match launcher {
        slab_config::ShellLauncherKind::Bash => ShellKind::Bash,
        slab_config::ShellLauncherKind::PowerShell => ShellKind::PowerShell,
        slab_config::ShellLauncherKind::Cmd => ShellKind::Cmd,
        slab_config::ShellLauncherKind::Auto => {
            if cfg!(windows) {
                ShellKind::PowerShell
            } else {
                ShellKind::Bash
            }
        }
    }
}

/// Backs the context hook with app-core model state + the shared exec policy.
#[derive(Clone)]
pub(crate) struct AppContextSources {
    model_state: ModelState,
    shell: ShellKind,
    exec_policy: Arc<dyn slab_exec_policy::ExecPolicyPort>,
    memory_config: slab_config::AgentMemoriesConfig,
    /// PARENT memory root; the read side routes to the current project's
    /// store under `<memory_root>/projects/<key>/`.
    memory_root: PathBuf,
    /// Per-thread recall selections (surfaced-memory dedup). Bounded by
    /// TTL + capacity; entries are dropped on thread end (`evict_thread`).
    recall_cache: Arc<DashMap<String, RecallCacheEntry>>,
}

impl AppContextSources {
    pub(crate) fn new(
        model_state: ModelState,
        shell: ShellKind,
        exec_policy: Arc<dyn slab_exec_policy::ExecPolicyPort>,
        memory_config: slab_config::AgentMemoriesConfig,
        memory_root: PathBuf,
    ) -> Self {
        Self {
            model_state,
            shell,
            exec_policy,
            memory_config,
            memory_root,
            recall_cache: Arc::new(DashMap::new()),
        }
    }

    /// The recall-selected rollout summaries for a thread, or `None`.
    ///
    /// Cached per thread while the project's memory manifest (max mtime of
    /// `raw_memories.md` + `rollout_summaries/`) is unchanged and the entry
    /// is inside the TTL — one side query per thread per manifest
    /// generation, not one per turn.
    async fn recall_body(
        &self,
        thread_id: &str,
        model_id: &str,
        input_message: Option<&str>,
        project_key: &str,
        project_root: &std::path::Path,
    ) -> Option<String> {
        if !self.memory_config.recall_enabled {
            return None;
        }
        let input = input_message?;
        let manifest = match recall::build_manifest(project_root) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(%error, "memory recall manifest build skipped");
                return None;
            }
        };
        if manifest.is_empty() {
            return None;
        }
        let watermark = manifest_watermark(project_root);
        if let Some(entry) = self.recall_cache.get(thread_id)
            && entry.project_key == project_key
            && entry.manifest_watermark == watermark
            && entry.created_at.elapsed() < Duration::from_secs(RECALL_CACHE_TTL_SECS)
        {
            return Some(entry.body.clone());
        }

        let model = self
            .memory_config
            .model
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| model_id.to_owned());
        let system = memory_templates::render_recall_select(recall::RECALL_TOP_K)
            .map_err(|error| tracing::warn!(%error, "memory recall prompt render failed"))
            .ok()?;
        let user = recall::render_manifest_prompt(
            &manifest,
            input,
            &self
                .workspace_root()
                .map(|root| root.to_string_lossy().into_owned())
                .unwrap_or_default(),
            chrono::Utc::now(),
        );
        let query = memory_chat_json(&self.model_state, &model, &system, &user);
        let output =
            match tokio::time::timeout(Duration::from_secs(RECALL_QUERY_TIMEOUT_SECS), query).await
            {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "memory recall side query failed");
                    return None;
                }
                Err(_) => {
                    tracing::warn!("memory recall side query timed out; skipping fragment");
                    return None;
                }
            };
        let selection = recall::parse_recall_selection(&output, &manifest);
        if selection.is_empty() {
            return None;
        }
        let entries = recall::render_selected_entries(project_root, &selection, chrono::Utc::now())
            .map_err(|error| tracing::warn!(%error, "memory recall rendering failed"))
            .ok()?;
        let body = memory_templates::render_memory_relevant(
            &project_root.to_string_lossy(),
            entries.trim_end(),
        )
        .map_err(|error| tracing::warn!(%error, "memory recall wrapper render failed"))
        .ok()?;

        self.recall_cache.insert(
            thread_id.to_owned(),
            RecallCacheEntry {
                project_key: project_key.to_owned(),
                manifest_watermark: watermark,
                body: body.clone(),
                created_at: Instant::now(),
            },
        );
        if self.recall_cache.len() > RECALL_CACHE_MAX_ENTRIES {
            evict_oldest_cache_entry(&self.recall_cache);
        }
        Some(body)
    }

    fn os_kind() -> OsKind {
        if cfg!(windows) {
            OsKind::Windows
        } else if cfg!(target_os = "macos") {
            OsKind::MacOS
        } else if cfg!(target_os = "linux") {
            OsKind::Linux
        } else {
            OsKind::Unknown
        }
    }
}

fn map_mode(mode: slab_exec_policy::PermissionMode) -> PermissionModeLabel {
    match mode {
        slab_exec_policy::PermissionMode::RequestApproval => PermissionModeLabel::RequestApproval,
        slab_exec_policy::PermissionMode::ApproveForMe => PermissionModeLabel::ApproveForMe,
        slab_exec_policy::PermissionMode::FullControl => PermissionModeLabel::FullControl,
        slab_exec_policy::PermissionMode::Custom => PermissionModeLabel::Custom,
    }
}

fn map_baseline(baseline: slab_exec_policy::PermissionBaseline) -> PermissionBaselineLabel {
    match baseline {
        slab_exec_policy::PermissionBaseline::ReadOnly => PermissionBaselineLabel::ReadOnly,
        slab_exec_policy::PermissionBaseline::WorkspaceWrite => {
            PermissionBaselineLabel::WorkspaceWrite
        }
        slab_exec_policy::PermissionBaseline::FullAccess => PermissionBaselineLabel::FullAccess,
    }
}

#[async_trait]
impl AgentContextSources for AppContextSources {
    fn workspace_root(&self) -> Option<PathBuf> {
        workspace_root_from_config(self.model_state.config())
    }

    fn app_home_skills_dir(&self) -> PathBuf {
        slab_utils::app_home::skills_dir()
    }

    fn app_home_agents_md(&self) -> PathBuf {
        slab_utils::app_home::agents_md_path()
    }

    async fn instruction_template_for(&self, model_id: &str) -> Option<String> {
        resolve_local_chat_prompt_profile(&self.model_state, model_id)
            .await
            .ok()
            .and_then(|profile| profile.instruction_template_source)
    }

    fn environment_snapshot(&self) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            cwd: self.workspace_root().map(|root| root.to_string_lossy().into_owned()),
            shell: self.shell,
            os: Self::os_kind(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn permission_snapshot(&self, thread_id: &str) -> PermissionSnapshot {
        let state = self.exec_policy.permission_state_for(thread_id);
        let shell_allowed = state.exposure.contains(slab_exec_policy::OperationCategory::Shell);
        let file_write_allowed =
            state.exposure.contains(slab_exec_policy::OperationCategory::FileEdit);
        let network_allowed = state.exposure.contains(slab_exec_policy::OperationCategory::Network);
        PermissionSnapshot {
            mode: map_mode(state.mode),
            baseline: map_baseline(state.baseline),
            read_only: !shell_allowed && !file_write_allowed && !network_allowed,
            shell_allowed,
            file_write_allowed,
            network_allowed,
        }
    }

    async fn memory_context(
        &self,
        thread_id: &str,
        model_id: &str,
        input_message: Option<&str>,
    ) -> Option<MemoryContext> {
        if !self.memory_config.enabled {
            return None;
        }
        let project_key = resolve_project_key(self.workspace_root().as_deref());
        // Marker-guarded legacy adoption on the read path too: after the
        // first call this is a single stat. A failure races the write-side
        // pipeline's adoption and only costs this run its memory fragment.
        if let Err(error) = memory_fs::adopt_legacy_layout(&self.memory_root, &project_key) {
            tracing::warn!(%error, "memory legacy layout adoption skipped on read side");
        }
        let project_root = memory_fs::project_memory_root(&self.memory_root, &project_key);
        let config = slab_agent_memories::read::MemoryReadConfig {
            memory_root: project_root.clone(),
            inject_hook_instructions: true,
        };
        let body = match slab_agent_memories::read::render_read_developer_message(&config) {
            Ok(Some(body)) => body,
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(%error, "memory context render skipped");
                return None;
            }
        };
        let relevant_body =
            self.recall_body(thread_id, model_id, input_message, &project_key, &project_root).await;
        Some(MemoryContext { body, relevant_body })
    }

    fn evict_thread(&self, thread_id: &str) {
        self.recall_cache.remove(thread_id);
    }
}

/// Max mtime across the recall inputs — the cache invalidation watermark.
fn manifest_watermark(project_root: &std::path::Path) -> SystemTime {
    let mut watermark = SystemTime::UNIX_EPOCH;
    let mut consider = |path: &std::path::Path| {
        if let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified())
            && modified > watermark
        {
            watermark = modified;
        }
    };
    consider(&project_root.join(memory_fs::RAW_MEMORIES_FILE));
    if let Ok(entries) = std::fs::read_dir(project_root.join("rollout_summaries")) {
        for entry in entries.flatten() {
            consider(&entry.path());
        }
    }
    watermark
}

fn evict_oldest_cache_entry(cache: &DashMap<String, RecallCacheEntry>) {
    let oldest = cache
        .iter()
        .min_by_key(|entry| entry.value().created_at)
        .map(|entry| entry.key().to_owned());
    if let Some(key) = oldest {
        cache.remove(&key);
    }
}
