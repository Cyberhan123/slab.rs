use std::path::PathBuf;
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
    read::parse_memory_citations,
    templates,
};
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

#[derive(Clone)]
pub struct AgentMemoryPipeline {
    store: Arc<AnyStore>,
    model_state: Arc<ModelState>,
    config: AgentMemoriesConfig,
    memory_root: PathBuf,
    control: Arc<OnceLock<Arc<AgentControl>>>,
}

impl AgentMemoryPipeline {
    pub fn new(
        store: Arc<AnyStore>,
        model_state: Arc<ModelState>,
        config: AgentMemoriesConfig,
        memory_root: PathBuf,
    ) -> Self {
        Self { store, model_state, config, memory_root, control: Arc::new(OnceLock::new()) }
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
            self.mark_phase2_selection(&selection.inputs, selection.new_watermark).await?;
            self.complete_phase2(&run_id, "succeeded", selection.new_watermark, None).await?;
            return Ok(());
        }

        let result = self.run_consolidation_agent(model, &diff.diff_path).await;
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
        let config = AgentConfig {
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
        };
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
        let snapshot = control
            .wait_for_terminal_snapshot(&thread_id)
            .await
            .map_err(|error| error.to_string())?;
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
        let rows: Vec<AgentThreadMessageRow> = sqlx::query_as(
            "SELECT role, content, created_at FROM agent_thread_messages \
             WHERE thread_id = ?1 ORDER BY turn_index ASC, created_at ASC, id ASC",
        )
        .bind(&candidate.thread_id)
        .fetch_all(&self.store.pool)
        .await
        .map_err(|error| error.to_string())?;
        let items = rows
            .into_iter()
            .map(|row| {
                let content = serde_json::from_str::<ConversationMessage>(&row.content)
                    .map(|message| message.rendered_text())
                    .unwrap_or(row.content);
                phase1::RolloutResponseItem { role: row.role, content, created_at: row.created_at }
            })
            .collect();
        Ok(Phase1RolloutInput { candidate, items })
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
        let citations = parse_memory_citations(content);
        if citations.is_empty() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.store.pool.begin().await.map_err(|error| error.to_string())?;
        for citation in citations {
            sqlx::query(
                "INSERT INTO agent_memory_usage_events (id, thread_id, source, note, used_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(thread_id)
            .bind(citation.source)
            .bind(citation.note)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        }
        tx.commit().await.map_err(|error| error.to_string())?;
        Ok(())
    }
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
struct AgentThreadMessageRow {
    role: String,
    content: String,
    created_at: String,
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
