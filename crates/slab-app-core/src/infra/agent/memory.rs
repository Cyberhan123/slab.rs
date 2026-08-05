use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use futures::{StreamExt, stream};
use slab_agent::{AgentConfig, AgentControl, AgentHook, HookEvent, HookOutcome};
use slab_agent_memories::{
    fs as memory_fs, git as memory_git, phase1,
    phase1::{Phase1MemoryOutput, Phase1ModelOutput, Phase1RolloutInput, RolloutCandidate},
    phase2,
    phase2::{Phase2Input, Phase2SelectionConfig},
    read::{MemoryCitationSourceKind, parse_memory_citations},
    templates,
};
use slab_agent_rollout::RolloutFileStore;
use slab_config::AgentMemoriesConfig;
use slab_types::{ConversationMessage, ConversationMessageContent, StructuredOutput};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatStreamOptions, CloudChatParams,
    CommonChatParams, LocalChatParams,
};
use crate::domain::services::ChatService;
use crate::infra::db::AnyStore;

use super::rollout_store::RolloutBackedAgentStore;

const MEMORY_PHASE2_SESSION_PREFIX: &str = "memory-phase2-";

#[derive(Clone)]
pub struct AgentMemoryPipeline {
    store: Arc<AnyStore>,
    /// The rollout JSONL true source (Slice F). Retained so `build_phase1_input`
    /// can stamp the real on-disk `rollout_path` for the prompt. The CONVERSATION
    /// itself is read through `rollout_store` below — the SAME production read
    /// path the agent runtime uses — so the memory model can never diverge from
    /// what the runtime replayed.
    rollout: Arc<RolloutFileStore>,
    /// The rollout-backed store that owns the production read path
    /// (`list_thread_messages` / `read_thread_messages`): rollout flush +
    /// `replay_messages`. Slice E made the rollout JSONL the sole source, so
    /// routing the phase1 read through here keeps the memory model on the SAME
    /// replay the runtime observes — the two cannot diverge.
    rollout_store: Arc<RolloutBackedAgentStore>,
    /// App-wide workspace root, surfaced as `rollout_cwd` in the phase1 prompt
    /// so the memory model knows where the session ran. `None` when no
    /// workspace is bound (the prompt then renders "unknown").
    workspace_root: Option<PathBuf>,
    model_state: Arc<ModelState>,
    config: AgentMemoriesConfig,
    memory_root: PathBuf,
    control: Arc<OnceLock<Arc<AgentControl>>>,
}

impl AgentMemoryPipeline {
    pub fn new(
        store: Arc<AnyStore>,
        rollout: Arc<RolloutFileStore>,
        rollout_store: Arc<RolloutBackedAgentStore>,
        workspace_root: Option<PathBuf>,
        model_state: Arc<ModelState>,
        config: AgentMemoriesConfig,
        memory_root: PathBuf,
    ) -> Self {
        Self {
            store,
            rollout,
            rollout_store,
            workspace_root,
            model_state,
            config,
            memory_root,
            control: Arc::new(OnceLock::new()),
        }
    }

    pub fn set_control(&self, control: Arc<AgentControl>) {
        let _ = self.control.set(control);
    }

    pub fn start_background(&self, fallback_model: String) {
        if !self.config.enabled {
            return;
        }
        let pipeline = self.clone();
        tokio::spawn(async move {
            if let Err(error) = pipeline.run_startup(fallback_model).await {
                warn!(%error, "agent memory startup pipeline failed");
            }
        });
    }

    async fn run_startup(&self, fallback_model: String) -> Result<(), String> {
        let model = self
            .config
            .model
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(fallback_model);
        self.run_phase1(&model).await?;
        self.run_phase2(&model).await?;
        Ok(())
    }

    async fn run_phase1(&self, model: &str) -> Result<(), String> {
        let owner = Uuid::new_v4().to_string();
        let now = Utc::now();
        let candidates = self.claim_phase1_candidates(&owner, now).await?;
        if candidates.is_empty() {
            return Ok(());
        }
        let concurrency = self.config.phase1_concurrency.max(1) as usize;
        let model = model.to_owned();
        stream::iter(candidates)
            .for_each_concurrent(concurrency, |candidate| {
                let pipeline = self.clone();
                let model = model.clone();
                async move {
                    let thread_id = candidate.thread_id.clone();
                    if let Err(error) = pipeline.process_phase1_candidate(&model, candidate).await {
                        warn!(%error, "memory phase1 candidate failed");
                        pipeline.fail_phase1(&thread_id, &error).await.ok();
                    }
                }
            })
            .await;
        Ok(())
    }

    async fn process_phase1_candidate(
        &self,
        model: &str,
        candidate: RolloutCandidate,
    ) -> Result<(), String> {
        let input = self.load_phase1_rollout_input(candidate.clone()).await?;
        let user_prompt = input.render_user_prompt().map_err(|error| error.to_string())?;
        let content = self
            .chat_json(
                model,
                vec![
                    ConversationMessage {
                        role: "system".to_owned(),
                        content: ConversationMessageContent::Text(
                            templates::PHASE1_SYSTEM_TEMPLATE.to_owned(),
                        ),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    },
                    ConversationMessage {
                        role: "user".to_owned(),
                        content: ConversationMessageContent::Text(user_prompt),
                        name: None,
                        tool_call_id: None,
                        tool_calls: Vec::new(),
                    },
                ],
            )
            .await?;
        let parsed =
            Phase1ModelOutput::from_model_json(&content).map_err(|error| error.to_string())?;
        match parsed.into_memory_output(&candidate, Utc::now()) {
            Some(output) => self.complete_phase1_success(output).await,
            None => self.complete_phase1_no_output(&candidate.thread_id).await,
        }
    }

    async fn run_phase2(&self, model: &str) -> Result<(), String> {
        let owner = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let Some(claimed_watermark) = self.claim_phase2(&run_id, &owner, now).await? else {
            return Ok(());
        };
        let inputs = self.load_phase2_inputs().await?;
        let selection = phase2::select_phase2_inputs(
            inputs,
            Phase2SelectionConfig {
                limit: self.config.phase2_limit as usize,
                max_unused_days: self.config.max_unused_days,
            },
            now,
            claimed_watermark,
        );
        if let Err(error) = memory_fs::sync_phase2_workspace(
            &self.memory_root,
            &selection.inputs,
            self.config.extension_retention_days,
            now,
        ) {
            self.complete_phase2(
                &run_id,
                "failed",
                selection.new_watermark,
                Some(&error.to_string()),
            )
            .await
            .ok();
            return Err(error.to_string());
        }

        let diff = memory_git::write_workspace_diff(&self.memory_root)
            .map_err(|error| error.to_string())?;
        if diff.diff.trim().is_empty() {
            memory_git::remove_workspace_diff_file(&self.memory_root)
                .map_err(|error| error.to_string())?;
            self.mark_phase2_selection(&selection.inputs, selection.new_watermark).await?;
            self.complete_phase2(&run_id, "succeeded", selection.new_watermark, None).await?;
            return Ok(());
        }

        let result = self.run_consolidation_agent(model, &diff.diff_path, &owner).await;
        match result {
            Ok(()) => {
                let _ = std::fs::remove_file(&diff.diff_path);
                memory_git::reset_memory_git_baseline(&self.memory_root)
                    .map_err(|error| error.to_string())?;
                self.mark_phase2_selection(&selection.inputs, selection.new_watermark).await?;
                self.complete_phase2(&run_id, "succeeded", selection.new_watermark, None).await?;
                Ok(())
            }
            Err(error) => {
                self.complete_phase2(&run_id, "failed", selection.new_watermark, Some(&error))
                    .await
                    .ok();
                Err(error)
            }
        }
    }

    async fn run_consolidation_agent(
        &self,
        model: &str,
        diff_path: &std::path::Path,
        owner: &str,
    ) -> Result<(), String> {
        let Some(control) = self.control.get().cloned() else {
            return Err("agent control is not available for memory phase2".to_owned());
        };
        let prompt = templates::render_phase2_consolidation(
            &self.memory_root.to_string_lossy(),
            &diff_path.to_string_lossy(),
            "",
            "",
        )
        .map_err(|error| error.to_string())?;
        let config = phase2_consolidation_agent_config(model, prompt);
        let thread_id = control
            .spawn(
                format!("memory-phase2-{}", Uuid::new_v4()),
                config,
                vec![ConversationMessage {
                    role: "user".to_owned(),
                    content: ConversationMessageContent::Text(format!(
                        "Consolidate the memory workspace. Read {} first for the git-style diff context.",
                        diff_path.display()
                    )),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                }],
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut status_rx =
            control.subscribe(&thread_id).await.map_err(|error| error.to_string())?;
        let heartbeat_seconds = (self.config.phase2_lease_seconds / 3).clamp(1, 60);
        let mut heartbeat =
            tokio::time::interval(std::time::Duration::from_secs(heartbeat_seconds));
        loop {
            let status = *status_rx.borrow();
            if matches!(
                status,
                slab_agent::ThreadStatus::Completed
                    | slab_agent::ThreadStatus::Errored
                    | slab_agent::ThreadStatus::Interrupted
                    | slab_agent::ThreadStatus::Shutdown
            ) {
                break;
            }
            tokio::select! {
                changed = status_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    if !self.refresh_phase2_lease(owner, Utc::now()).await? {
                        return Err("memory phase2 lease was lost during consolidation".to_owned());
                    }
                }
            }
        }
        let snapshot = control
            .thread_snapshot(&thread_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("memory consolidation thread {thread_id} disappeared"))?;
        if snapshot.status == slab_agent::ThreadStatus::Completed {
            Ok(())
        } else {
            Err(format!("memory consolidation agent ended with status {}", snapshot.status))
        }
    }

    async fn chat_json(
        &self,
        model: &str,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, String> {
        let command = ChatCompletionCommand {
            id: None,
            model: model.to_owned(),
            messages,
            tools: Vec::new(),
            agent_trace: None,
            continue_generation: false,
            common: CommonChatParams {
                max_tokens: None,
                temperature: Some(0.1),
                top_p: None,
                top_k: None,
                min_p: None,
                presence_penalty: None,
                repetition_penalty: None,
                n: 1,
                stream: false,
                stop: Vec::new(),
                stream_options: ChatStreamOptions::default(),
            },
            local: LocalChatParams {
                gbnf: None,
                structured_output: Some(StructuredOutput::JsonObject),
                session_key: None,
                reasoning_guidance_in_context: false,
            },
            cloud: CloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output: Some(StructuredOutput::JsonObject),
            },
        };
        let service = ChatService::new((*self.model_state).clone());
        match service.create_chat_completion(command).await.map_err(|error| error.to_string())? {
            ChatCompletionOutput::Json(result) => result
                .choices
                .into_iter()
                .next()
                .map(|choice| choice.message.content.rendered_text())
                .ok_or_else(|| "memory model returned no choices".to_owned()),
            ChatCompletionOutput::Stream(_) => {
                Err("memory model returned an unexpected stream".to_owned())
            }
        }
    }

    async fn claim_phase1_candidates(
        &self,
        owner: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<RolloutCandidate>, String> {
        let idle_before = now - Duration::seconds(self.config.phase1_idle_seconds as i64);
        let min_updated = now - Duration::days(self.config.phase1_max_age_days as i64);
        let rows: Vec<AgentThreadCandidateRow> = sqlx::query_as(
            "SELECT id, session_id, config_json, updated_at \
             FROM agent_threads \
             WHERE parent_id IS NULL \
               AND updated_at >= ?1 \
               AND updated_at <= ?2 \
               AND status IN ('completed', 'errored', 'interrupted', 'shutdown') \
             ORDER BY updated_at DESC, id ASC \
             LIMIT ?3",
        )
        .bind(min_updated.to_rfc3339())
        .bind(idle_before.to_rfc3339())
        .bind(self.config.phase1_scan_limit as i64)
        .fetch_all(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;

        let mut claimed = Vec::new();
        let lease_until = now + Duration::seconds(self.config.phase1_lease_seconds as i64);
        for row in rows {
            if row.session_id.starts_with(MEMORY_PHASE2_SESSION_PREFIX) {
                continue;
            }
            let Ok(config) = serde_json::from_str::<AgentConfig>(&row.config_json) else {
                continue;
            };
            if config.transient {
                continue;
            }
            sqlx::query(
                "INSERT OR IGNORE INTO agent_memory_phase1_outputs \
                 (thread_id, session_id, status) VALUES (?1, ?2, 'pending')",
            )
            .bind(&row.id)
            .bind(&row.session_id)
            .execute(&self.store.pool)
            .await
            .map_err(|error| error.to_string())?;
            let updated = sqlx::query(
                "UPDATE agent_memory_phase1_outputs \
                 SET status='running', lease_owner=?1, lease_until=?2, attempts=attempts + 1, \
                     updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE thread_id=?3 \
                   AND (lease_until IS NULL OR lease_until < ?4) \
                   AND (next_retry_at IS NULL OR next_retry_at <= ?4) \
                   AND status IN ('pending', 'failed')",
            )
            .bind(owner)
            .bind(lease_until.to_rfc3339())
            .bind(&row.id)
            .bind(now.to_rfc3339())
            .execute(&self.store.pool)
            .await
            .map_err(|error| error.to_string())?;
            if updated.rows_affected() == 0 {
                continue;
            }
            claimed.push(RolloutCandidate {
                thread_id: row.id,
                session_id: row.session_id,
                rollout_path: None,
                rollout_cwd: None,
                source_updated_at: parse_rfc3339(&row.updated_at),
            });
        }
        Ok(claimed)
    }

    async fn load_phase1_rollout_input(
        &self,
        candidate: RolloutCandidate,
    ) -> Result<Phase1RolloutInput, String> {
        build_phase1_input(
            &self.rollout_store,
            &self.rollout,
            self.workspace_root.as_deref(),
            candidate,
        )
        .await
    }

    async fn complete_phase1_success(&self, output: Phase1MemoryOutput) -> Result<(), String> {
        sqlx::query(
            "UPDATE agent_memory_phase1_outputs \
             SET status='succeeded', raw_memory=?1, rollout_summary=?2, rollout_slug=?3, \
                 source_updated_at=?4, generated_at=?5, lease_owner=NULL, lease_until=NULL, \
                 error=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE thread_id=?6",
        )
        .bind(output.raw_memory)
        .bind(output.rollout_summary)
        .bind(output.rollout_slug)
        .bind(output.source_updated_at.to_rfc3339())
        .bind(output.generated_at.to_rfc3339())
        .bind(output.thread_id)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn complete_phase1_no_output(&self, thread_id: &str) -> Result<(), String> {
        sqlx::query(
            "UPDATE agent_memory_phase1_outputs \
             SET status='succeeded_no_output', lease_owner=NULL, lease_until=NULL, error=NULL, \
                 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE thread_id=?1",
        )
        .bind(thread_id)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn fail_phase1(&self, thread_id: &str, error: &str) -> Result<(), String> {
        let retry_at = Utc::now() + Duration::seconds(self.config.phase1_retry_seconds as i64);
        sqlx::query(
            "UPDATE agent_memory_phase1_outputs \
             SET status='failed', lease_owner=NULL, lease_until=NULL, next_retry_at=?1, error=?2, \
                 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE thread_id=?3",
        )
        .bind(retry_at.to_rfc3339())
        .bind(error)
        .bind(thread_id)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn claim_phase2(
        &self,
        run_id: &str,
        owner: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<Option<DateTime<Utc>>>, String> {
        let lease_until = now + Duration::seconds(self.config.phase2_lease_seconds as i64);
        sqlx::query(
            "INSERT OR IGNORE INTO agent_memory_phase2_lock (id, status) VALUES (1, 'idle')",
        )
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        let updated = sqlx::query(
            "UPDATE agent_memory_phase2_lock \
             SET status='running', lease_owner=?1, lease_until=?2, \
                 claimed_watermark=(SELECT MAX(source_updated_at) FROM agent_memory_phase1_outputs WHERE status='succeeded'), \
                 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id=1 AND (lease_until IS NULL OR lease_until < ?3 OR status != 'running')",
        )
        .bind(owner)
        .bind(lease_until.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        let watermark: Option<String> =
            sqlx::query_scalar("SELECT claimed_watermark FROM agent_memory_phase2_lock WHERE id=1")
                .fetch_one(&self.store.pool)
                .await
                .map_err(|error| error.to_string())?;
        sqlx::query(
            "INSERT INTO agent_memory_phase2_runs \
             (id, status, lease_owner, claimed_watermark, started_at) \
             VALUES (?1, 'running', ?2, ?3, ?4)",
        )
        .bind(run_id)
        .bind(owner)
        .bind(&watermark)
        .bind(now.to_rfc3339())
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(Some(watermark.as_deref().map(parse_rfc3339)))
    }

    async fn refresh_phase2_lease(&self, owner: &str, now: DateTime<Utc>) -> Result<bool, String> {
        let lease_until = now + Duration::seconds(self.config.phase2_lease_seconds as i64);
        let updated = sqlx::query(
            "UPDATE agent_memory_phase2_lock \
             SET lease_until=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id=1 AND status='running' AND lease_owner=?2",
        )
        .bind(lease_until.to_rfc3339())
        .bind(owner)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(updated.rows_affected() == 1)
    }

    async fn load_phase2_inputs(&self) -> Result<Vec<Phase2Input>, String> {
        let rows: Vec<Phase2InputRow> = sqlx::query_as(
            "SELECT thread_id, session_id, raw_memory, rollout_summary, rollout_slug, \
                    generated_at, source_updated_at, last_usage, usage_count \
             FROM agent_memory_phase1_outputs \
             WHERE status='succeeded' AND raw_memory IS NOT NULL AND rollout_summary IS NOT NULL",
        )
        .fetch_all(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(Phase2InputRow::into_input).collect())
    }

    async fn mark_phase2_selection(
        &self,
        inputs: &[Phase2Input],
        _watermark: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        let mut tx = self.store.pool.begin().await.map_err(|error| error.to_string())?;
        sqlx::query("UPDATE agent_memory_phase1_outputs SET selected_for_phase2=0")
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        for input in inputs {
            sqlx::query(
                "UPDATE agent_memory_phase1_outputs \
                 SET selected_for_phase2=1, selected_for_phase2_source_updated_at=?1 \
                 WHERE thread_id=?2",
            )
            .bind(input.source_updated_at.to_rfc3339())
            .bind(&input.thread_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        }
        tx.commit().await.map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn complete_phase2(
        &self,
        run_id: &str,
        status: &str,
        watermark: Option<DateTime<Utc>>,
        error: Option<&str>,
    ) -> Result<(), String> {
        let completed_at = Utc::now().to_rfc3339();
        let watermark = watermark.map(|value| value.to_rfc3339());
        sqlx::query(
            "UPDATE agent_memory_phase2_runs \
             SET status=?1, completed_watermark=?2, completed_at=?3, error=?4 \
             WHERE id=?5",
        )
        .bind(status)
        .bind(&watermark)
        .bind(&completed_at)
        .bind(error)
        .bind(run_id)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        sqlx::query(
            "UPDATE agent_memory_phase2_lock \
             SET status=?1, lease_owner=NULL, lease_until=NULL, completed_watermark=?2, \
                 updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id=1",
        )
        .bind(status)
        .bind(&watermark)
        .execute(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn record_memory_usage(
        &self,
        thread_id: &str,
        response: &slab_agent::LlmResponse,
    ) -> Result<(), String> {
        let Some(content) = response.content.as_deref() else {
            return Ok(());
        };
        record_memory_usage_in_pool(&self.store.pool, thread_id, content).await
    }
}

async fn record_memory_usage_in_pool(
    pool: &sqlx::SqlitePool,
    thread_id: &str,
    content: &str,
) -> Result<(), String> {
    let citations = parse_memory_citations(content);
    if citations.is_empty() {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    for citation in citations {
        let source_kind = citation.source_kind.as_str();
        sqlx::query(
            "INSERT INTO agent_memory_usage_events \
             (id, thread_id, source, source_kind, note, used_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(thread_id)
        .bind(&citation.source)
        .bind(source_kind)
        .bind(&citation.note)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
        if citation.source_kind == MemoryCitationSourceKind::RolloutSummary
            && let Some(filename) = rollout_summary_filename_from_citation(&citation.source)
        {
            sqlx::query(
                "UPDATE agent_memory_phase1_outputs \
                 SET last_usage=?1, usage_count=usage_count + 1, \
                     updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE selected_for_phase2=1 \
                   AND ((rollout_slug IS NOT NULL AND rollout_slug || '.md' = ?2) \
                        OR ((rollout_slug IS NULL OR rollout_slug = '') \
                            AND thread_id || '.md' = ?2))",
            )
            .bind(&now)
            .bind(filename)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        }
    }
    tx.commit().await.map_err(|error| error.to_string())?;
    Ok(())
}

/// Build the phase1 input for a candidate thread, reading the conversation
/// through the SAME production read path the agent runtime uses
/// ([`RolloutBackedAgentStore::read_thread_messages`], which the app-core
/// `RolloutConversationStore::list_thread_messages` trait method also delegates
/// to). The memory model and the runtime therefore share ONE code path and can
/// never diverge on what the conversation was.
///
/// Slice E: rollout is the ONLY source, so this always replays the rollout file
/// and stamps the real on-disk `rollout_path`. A missing rollout file (a
/// brand-new thread before its first append) replays to an empty conversation.
/// `rollout_cwd` is always the app-wide workspace root when one is bound.
///
/// Extracted as a free function so the shared read path is unit-testable
/// without constructing the full [`ModelState`]-backed pipeline.
async fn build_phase1_input(
    rollout_store: &RolloutBackedAgentStore,
    rollout: &RolloutFileStore,
    workspace_root: Option<&Path>,
    mut candidate: RolloutCandidate,
) -> Result<Phase1RolloutInput, String> {
    // The workspace cwd is meaningful regardless of storage source — the session
    // ran in this workspace whether its conversation now lives in rollout or SQL.
    candidate.rollout_cwd = workspace_root.map(|root| root.to_string_lossy().into_owned());

    let records = rollout_store
        .read_thread_messages(&candidate.thread_id)
        .await
        .map_err(|error| error.to_string())?;
    candidate.rollout_path =
        Some(rollout.resolve_path(&candidate.thread_id).to_string_lossy().into_owned());
    let items = records
        .into_iter()
        .map(|record| {
            // rendered_text borrows; call it before moving `role` out of the message.
            let content = record.message.rendered_text();
            phase1::RolloutResponseItem {
                role: record.message.role,
                content,
                created_at: record.created_at,
            }
        })
        .collect();
    let items = phase1::filter_memory_relevant_items(items);
    Ok(Phase1RolloutInput { candidate, items })
}

pub struct AgentMemoryStartupHook {
    pipeline: AgentMemoryPipeline,
}

impl AgentMemoryStartupHook {
    pub fn new(pipeline: AgentMemoryPipeline) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl AgentHook for AgentMemoryStartupHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnAgentStart { parent_id, depth, config, .. } => {
                if !self.pipeline.config.enabled
                    || config.transient
                    || parent_id.is_some()
                    || *depth != 0
                {
                    return HookOutcome::Continue;
                }
                self.pipeline.start_background(config.model.clone());
            }
            HookEvent::OnLlmEnd { thread_id, response, .. } => {
                if self.pipeline.config.enabled {
                    let pipeline = self.pipeline.clone();
                    let thread_id = thread_id.clone();
                    let response = response.clone();
                    tokio::spawn(async move {
                        if let Err(error) =
                            pipeline.record_memory_usage(&thread_id, &response).await
                        {
                            warn!(%error, "failed to record memory usage");
                        }
                    });
                }
            }
            HookEvent::OnLlmStart { .. }
            | HookEvent::OnToolStart { .. }
            | HookEvent::OnToolEnd { .. }
            | HookEvent::OnAgentEnd { .. } => {}
        }
        HookOutcome::Continue
    }
}

#[derive(sqlx::FromRow)]
struct AgentThreadCandidateRow {
    id: String,
    session_id: String,
    config_json: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct Phase2InputRow {
    thread_id: String,
    session_id: String,
    raw_memory: String,
    rollout_summary: String,
    rollout_slug: Option<String>,
    generated_at: String,
    source_updated_at: String,
    last_usage: Option<String>,
    usage_count: i64,
}

impl Phase2InputRow {
    fn into_input(self) -> Phase2Input {
        Phase2Input {
            thread_id: self.thread_id,
            session_id: self.session_id,
            raw_memory: self.raw_memory,
            rollout_summary: self.rollout_summary,
            rollout_slug: self.rollout_slug,
            generated_at: parse_rfc3339(&self.generated_at),
            source_updated_at: parse_rfc3339(&self.source_updated_at),
            last_usage: self.last_usage.as_deref().map(parse_rfc3339),
            usage_count: self.usage_count.max(0) as u64,
        }
    }
}

fn parse_rfc3339(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value).map(|value| value.with_timezone(&Utc)).unwrap_or_else(
        |_| {
            debug!(value, "invalid timestamp in agent memory state; using now");
            Utc::now()
        },
    )
}

fn rollout_summary_filename_from_citation(source: &str) -> Option<String> {
    let path = source.split_once(':').map_or(source, |(path, _)| path).replace('\\', "/");
    let filename = path.strip_prefix("rollout_summaries/")?;
    if filename.ends_with(".md") && !filename.contains('/') {
        return Some(filename.to_owned());
    }
    None
}

fn phase2_consolidation_agent_config(model: &str, prompt: String) -> AgentConfig {
    AgentConfig {
        model: model.to_owned(),
        system_prompt: Some(prompt),
        max_turns: 12,
        max_depth: 0,
        max_threads: 1,
        allowed_tools: vec![
            "read_file".to_owned(),
            "write_file".to_owned(),
            "list_dir".to_owned(),
            "grep".to_owned(),
        ],
        transient: true,
        ..AgentConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::AgentConfig;
    use slab_agent_rollout::{
        RolloutFileStore, RolloutItem, RolloutStore, SessionMeta, TurnContextPayload,
    };

    use super::*;
    use crate::infra::db::AnyStore;

    #[tokio::test]
    async fn records_usage_source_kind_and_updates_matching_selected_rollout() {
        let store = AnyStore::connect("sqlite::memory:").await.expect("store");
        insert_thread(&store, "thread-a").await;
        insert_thread(&store, "thread-b").await;
        sqlx::query(
            "INSERT INTO agent_memory_phase1_outputs \
             (thread_id, session_id, status, raw_memory, rollout_summary, rollout_slug, \
              selected_for_phase2, usage_count) \
             VALUES \
             ('thread-a', 'session-1', 'succeeded', 'raw', 'summary', 'slug-a', 1, 0), \
             ('thread-b', 'session-1', 'succeeded', 'raw', 'summary', NULL, 1, 0)",
        )
        .execute(&store.pool)
        .await
        .expect("phase1 rows");

        record_memory_usage_in_pool(
            &store.pool,
            "reader-thread",
            "<oai-mem-citation>\n<citation_entries>\nrollout_summaries/slug-a.md:1-2|note=[used]\nraw_memories.md:3-4|note=[read]\n</citation_entries>\n<rollout_ids>\n</rollout_ids>\n</oai-mem-citation>",
        )
        .await
        .expect("record usage");

        let events: Vec<(String,)> =
            sqlx::query_as("SELECT source_kind FROM agent_memory_usage_events ORDER BY source")
                .fetch_all(&store.pool)
                .await
                .expect("events");
        assert_eq!(events, vec![("raw_memory".to_owned(),), ("rollout_summary".to_owned(),)]);

        let usage_a: (i64, Option<String>) = sqlx::query_as(
            "SELECT usage_count, last_usage FROM agent_memory_phase1_outputs WHERE thread_id='thread-a'",
        )
        .fetch_one(&store.pool)
        .await
        .expect("usage a");
        let usage_b: (i64, Option<String>) = sqlx::query_as(
            "SELECT usage_count, last_usage FROM agent_memory_phase1_outputs WHERE thread_id='thread-b'",
        )
        .fetch_one(&store.pool)
        .await
        .expect("usage b");
        assert_eq!(usage_a.0, 1);
        assert!(usage_a.1.is_some());
        assert_eq!(usage_b, (0, None));
    }

    #[test]
    fn extracts_rollout_summary_filename_from_citation() {
        assert_eq!(
            rollout_summary_filename_from_citation("rollout_summaries/slug.md:1-2"),
            Some("slug.md".to_owned())
        );
        assert_eq!(
            rollout_summary_filename_from_citation("rollout_summaries/nested/slug.md:1-2"),
            None
        );
        assert_eq!(rollout_summary_filename_from_citation("raw_memories.md:1-2"), None);
    }

    #[test]
    fn phase2_consolidation_agent_config_is_transient_and_local_only() {
        let config = phase2_consolidation_agent_config("model", "prompt".to_owned());

        assert!(config.transient);
        assert_eq!(config.max_depth, 0);
        assert_eq!(config.max_threads, 1);
        assert_eq!(
            config.allowed_tools,
            vec![
                "read_file".to_owned(),
                "write_file".to_owned(),
                "list_dir".to_owned(),
                "grep".to_owned()
            ]
        );
        assert!(!config.allowed_tools.contains(&"delegate_subagent".to_owned()));
        assert!(!config.allowed_tools.contains(&"web_search".to_owned()));
        assert!(!config.allowed_tools.contains(&"shell".to_owned()));
    }

    fn text_msg(role: &str, text: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    fn append_message(
        turn_index: u32,
        message: ConversationMessage,
        id: &str,
        created_at: &str,
    ) -> RolloutItem {
        RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
            turn_index,
            message,
            id: Some(id.to_owned()),
            created_at: Some(created_at.to_owned()),
        })
    }

    // Build a rollout-backed store backed by a REAL in-memory SqlxStore (which
    // impls both AgentStorePort and RolloutIndex) + a temp-dir RolloutFileStore.
    // This makes the memory tests drive the production read path with NO mock —
    // the rollout_session_index gate, replay_messages, and the SQL fallback all
    // run for real, so a gate regression cannot hide behind a stub.
    fn rollout_backed_store(
        store: Arc<AnyStore>,
        rollout: Arc<RolloutFileStore>,
    ) -> Arc<RolloutBackedAgentStore> {
        Arc::new(RolloutBackedAgentStore::new(
            Arc::clone(&store) as Arc<dyn slab_agent::port::AgentStorePort>,
            Arc::clone(&store)
                as Arc<dyn crate::infra::db::repository::rollout_index::RolloutIndex>,
            rollout,
            None,
        ))
    }

    async fn mark_rollout_session(
        store: &AnyStore,
        thread_id: &str,
        session_id: &str,
        file_path: &str,
        backfill_status: &str,
    ) {
        sqlx::query(
            "INSERT INTO rollout_session_index \
             (thread_id, session_id, file_path, last_updated_at, created_at, backfill_status) \
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?4)",
        )
        .bind(thread_id)
        .bind(session_id)
        .bind(file_path)
        .bind(backfill_status)
        .execute(&store.pool)
        .await
        .expect("mark rollout session index");
    }

    fn compacted_marker(
        thread_id: &str,
        turn_index: u32,
        summary: Vec<ConversationMessage>,
        status: &str,
    ) -> RolloutItem {
        RolloutItem::Compacted(slab_agent_rollout::CompactedPayload {
            thread_id: thread_id.to_owned(),
            compacted_messages: summary,
            removed_messages: 0,
            output_tokens: 0,
            status: status.to_owned(),
            turn_index,
        })
    }

    // Slice F: the phase1 input is read from the rollout JSONL true source when
    // the rollout gate (`backfill_status == "completed"`) admits it. The replay
    // MUST preserve role / rendered_text / created_at fidelity (created_at from
    // the F3-carried timestamp, NOT the line write-time), stamp
    // candidate.rollout_path with the REAL on-disk file path, and surface
    // rollout_cwd as the workspace root — so the rendered prompt no longer falls
    // back to "state-db" / "unknown".
    #[tokio::test]
    async fn build_phase1_input_reads_rollout_true_source_with_fidelity() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = Arc::new(AnyStore::connect("sqlite::memory:").await.expect("store"));
        let backed = rollout_backed_store(Arc::clone(&store), Arc::clone(&rollout));
        let thread_id = "t-rollout";
        insert_thread(&store, thread_id).await;

        rollout.create_session(SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            parent_id: None,
            started_at: "2026-08-03T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        rollout
            .append(
                thread_id,
                append_message(0, text_msg("user", "hello world"), "m-u", "2026-08-03T00:00:01Z"),
            )
            .await
            .unwrap();
        rollout
            .append(
                thread_id,
                append_message(0, text_msg("assistant", "hi there"), "m-a", "2026-08-03T00:00:02Z"),
            )
            .await
            .unwrap();
        rollout
            .append(
                thread_id,
                append_message(1, text_msg("tool", "tool output"), "m-t", "2026-08-03T00:00:03Z"),
            )
            .await
            .unwrap();
        // Flip the production read gate to rollout-ready (backfill completed).
        let file_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();
        mark_rollout_session(&store, thread_id, "session-1", &file_path, "completed").await;

        let candidate = RolloutCandidate {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            rollout_path: None,
            rollout_cwd: None,
            source_updated_at: Utc::now(),
        };
        let workspace = PathBuf::from("C:/repo");
        let input =
            build_phase1_input(&backed, rollout.as_ref(), Some(workspace.as_path()), candidate)
                .await
                .expect("phase1 input");

        // rollout_path is the real on-disk rollout file path (NOT "state-db").
        let expected_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();
        assert_eq!(input.candidate.rollout_path.as_deref(), Some(expected_path.as_str()));
        assert_eq!(input.candidate.rollout_cwd.as_deref(), Some("C:/repo"));

        // role / content / created_at fidelity, in conversation order.
        let roles: Vec<&str> = input.items.iter().map(|i| i.role.as_str()).collect();
        assert_eq!(roles, vec!["user", "assistant", "tool"]);
        assert_eq!(input.items[0].content, "hello world");
        assert_eq!(input.items[1].content, "hi there");
        assert_eq!(input.items[2].content, "tool output");
        assert_eq!(input.items[0].created_at, "2026-08-03T00:00:01Z");
        assert_eq!(input.items[1].created_at, "2026-08-03T00:00:02Z");
        assert_eq!(input.items[2].created_at, "2026-08-03T00:00:03Z");

        // The rendered prompt carries the real rollout_path + rollout_cwd.
        let prompt = input.render_user_prompt().expect("prompt");
        assert!(prompt.contains(&format!("rollout_path: {expected_path}")));
        assert!(prompt.contains("rollout_cwd: C:/repo"));
        assert!(!prompt.contains("state-db"));
    }

    // M2: a Compacted marker (auto/manual) resets the replay baseline to the
    // summary, dropping the pre-compaction MessageAppends; post-compaction
    // appends are then layered on top. Memory must observe baseline(summary) +
    // post-compaction, with the pre-compaction turns dropped.
    #[tokio::test]
    async fn compacted_marker_resets_baseline_for_memory() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = Arc::new(AnyStore::connect("sqlite::memory:").await.expect("store"));
        let backed = rollout_backed_store(Arc::clone(&store), Arc::clone(&rollout));
        let thread_id = "t-compacted";
        insert_thread(&store, thread_id).await;

        rollout.create_session(SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            parent_id: None,
            started_at: "2026-08-03T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        // Two pre-compaction appends — these MUST be dropped by the Compacted
        // marker (baseline.clear).
        rollout
            .append(
                thread_id,
                append_message(
                    0,
                    text_msg("user", "pre-1 dropped"),
                    "pre-1",
                    "2026-08-03T00:00:01Z",
                ),
            )
            .await
            .unwrap();
        rollout
            .append(
                thread_id,
                append_message(
                    0,
                    text_msg("assistant", "pre-2 dropped"),
                    "pre-2",
                    "2026-08-03T00:00:02Z",
                ),
            )
            .await
            .unwrap();
        // Compacted: a 2-message summary baseline (auto compaction).
        rollout
            .append(
                thread_id,
                compacted_marker(
                    thread_id,
                    0,
                    vec![
                        text_msg("user", "summary-user"),
                        text_msg("assistant", "summary-assistant"),
                    ],
                    "auto",
                ),
            )
            .await
            .unwrap();
        // Two post-compaction appends — these survive on top of the baseline.
        rollout
            .append(
                thread_id,
                append_message(1, text_msg("user", "post-1"), "post-1", "2026-08-03T00:00:10Z"),
            )
            .await
            .unwrap();
        rollout
            .append(
                thread_id,
                append_message(
                    1,
                    text_msg("assistant", "post-2"),
                    "post-2",
                    "2026-08-03T00:00:11Z",
                ),
            )
            .await
            .unwrap();
        let file_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();
        mark_rollout_session(&store, thread_id, "session-1", &file_path, "completed").await;

        let candidate = RolloutCandidate {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            rollout_path: None,
            rollout_cwd: None,
            source_updated_at: Utc::now(),
        };
        let input = build_phase1_input(&backed, rollout.as_ref(), None, candidate)
            .await
            .expect("phase1 input");

        // baseline(summary=2) + post-compaction(2) = 4. Pre-compaction dropped.
        let contents: Vec<&str> = input.items.iter().map(|i| i.content.as_str()).collect();
        assert_eq!(
            contents,
            vec!["summary-user", "summary-assistant", "post-1", "post-2"],
            "pre-compaction appends must be dropped; summary baseline + post-compaction kept"
        );

        // M1 pin (known pre-existing distortion): replay_messages stamps every
        // compacted baseline message with the SAME timestamp — the Compacted
        // line write-time — instead of preserving any per-message created_at.
        // The two summary messages therefore share one created_at (the compaction
        // moment), distinct from the post-compaction appends' carried timestamps.
        // See notes (M1) for why this is deferred, not fixed here.
        assert_eq!(
            input.items[0].created_at, input.items[1].created_at,
            "M1 pin: compacted-baseline messages share the compaction line timestamp"
        );
        assert_ne!(
            input.items[0].created_at, input.items[2].created_at,
            "post-compaction appends keep their own carried created_at"
        );
    }

    // L4: an empty rollout session (SessionMeta only, no MessageAppends) with a
    // completed gate yields a real rollout_path but an EMPTY item list, and the
    // prompt renders without panicking.
    #[tokio::test]
    async fn empty_rollout_session_yields_real_path_and_empty_items() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = Arc::new(AnyStore::connect("sqlite::memory:").await.expect("store"));
        let backed = rollout_backed_store(Arc::clone(&store), Arc::clone(&rollout));
        let thread_id = "t-empty";
        insert_thread(&store, thread_id).await;

        rollout.create_session(SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            parent_id: None,
            started_at: "2026-08-03T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        // No appends -> empty conversation. Mark rollout-ready so the read comes
        // from rollout (not the empty SQL fallback).
        let file_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();
        mark_rollout_session(&store, thread_id, "session-1", &file_path, "completed").await;

        let candidate = RolloutCandidate {
            thread_id: thread_id.to_owned(),
            session_id: "session-1".to_owned(),
            rollout_path: None,
            rollout_cwd: None,
            source_updated_at: Utc::now(),
        };
        let input = build_phase1_input(&backed, rollout.as_ref(), None, candidate)
            .await
            .expect("phase1 input");

        assert!(input.items.is_empty(), "no appends -> empty item list");
        // rollout-ready -> the real on-disk rollout path is stamped.
        let expected_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();
        assert_eq!(input.candidate.rollout_path.as_deref(), Some(expected_path.as_str()));

        // render_user_prompt must not panic on an empty conversation.
        let prompt = input.render_user_prompt().expect("prompt renders on empty items");
        assert!(prompt.contains(&format!("rollout_path: {expected_path}")));
    }

    async fn insert_thread(store: &AnyStore, thread_id: &str) {
        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&AgentConfig::default()).expect("config");
        sqlx::query(
            "INSERT OR IGNORE INTO chat_sessions (id, name, created_at, updated_at) \
             VALUES ('session-1', '', ?1, ?1)",
        )
        .bind(&now)
        .execute(&store.pool)
        .await
        .expect("session");
        sqlx::query(
            "INSERT INTO agent_threads \
             (id, session_id, parent_id, depth, status, config_json, created_at, updated_at) \
             VALUES (?1, 'session-1', NULL, 0, 'completed', ?2, ?3, ?3)",
        )
        .bind(thread_id)
        .bind(config_json)
        .bind(now)
        .execute(&store.pool)
        .await
        .expect("thread");
    }
}
