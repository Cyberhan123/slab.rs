//! [`ContextInstructionHook`] — the `slab_agent::AgentHook` that injects
//! rendered context on agent start.
//!
//! On `OnAgentStart` it resolves the model's instruction template, scans
//! skills + `AGENTS.md`, snapshots the environment + permission state, and
//! injects a single ordered batch:
//! `[system, environment, permissions, …reasoning-effort(added later),
//! developer(skills — only when a skill or a model template exists), memory?,
//! …agents_md]`. `system` is emitted first so the
//! existing thread-level insertion logic (which places injected messages after
//! the leading `system` block) lands the rest in the right positions. Skill-body
//! expansion is NOT done here — hooks can only inject new messages, not mutate
//! user turns — so the server expands invoked skills before the turn starts
//! (see `slab-server` harness).

use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::{AgentHook, HookEvent, HookOutcome, HookToolAction};
use slab_types::{
    ChatReasoningEffort, ChatVerbosity, ConversationMessage, ConversationMessageContent,
};

use crate::agent_md_manager::{AgentMdRecord, scan_agents_md};
use crate::developer_instruction::DeveloperInstructionFragment;
use crate::environment_instruction::EnvironmentContextFragment;
use crate::error::Result;
use crate::fragment::ContextFragment;
use crate::helper::{build_environment, build_skill_roots};
use crate::permissions_instruction::PermissionsInstructionFragment;
use crate::reasoning_effort::ReasoningEffortFragment;
use crate::skill_manager::scan_skills;
use crate::sources::AgentContextSources;
use crate::system_instruction::SystemInstructionFragment;
use crate::user_instruction::AgentMdFragment;

/// Hook that injects system/developer/`AGENTS.md` context at agent start.
pub struct ContextInstructionHook {
    sources: Arc<dyn AgentContextSources>,
}

impl ContextInstructionHook {
    pub fn new(sources: Arc<dyn AgentContextSources>) -> Self {
        Self { sources }
    }

    async fn render_startup_messages(
        &self,
        thread_id: &str,
        model: &str,
        reasoning_effort: Option<ChatReasoningEffort>,
        verbosity: Option<ChatVerbosity>,
        input_message: Option<&str>,
    ) -> Result<Vec<ConversationMessage>> {
        let env = build_environment();

        let skills = scan_skills(
            self.sources.workspace_root().as_deref(),
            &self.sources.app_home_skills_dir(),
        );
        let agents_md = scan_agents_md(
            self.sources.workspace_root().as_deref(),
            &self.sources.app_home_agents_md(),
        );
        let skill_roots = build_skill_roots(&skills, &agents_md);

        let developer_template = self.sources.instruction_template_for(model).await;

        let mut messages = Vec::new();
        // 1. Identity / persona. `workspace_bound` keeps the tool-use guidance
        //    in the system prompt aligned with the registered tool set (the
        //    workspace-bound tools, e.g. `apply_patch`, exist only when a
        //    workspace root is configured).
        messages.push(tagged(
            SystemInstructionFragment { workspace_bound: self.sources.workspace_root().is_some() }
                .render(&env)?,
            "slab_system",
        ));
        // 2. Environment facts (cwd / shell / os / time).
        messages.push(tagged(
            EnvironmentContextFragment { snapshot: self.sources.environment_snapshot() }
                .render(&env)?,
            "slab_environment",
        ));
        // 3. Permission instructions + tool-use policy.
        messages.push(tagged(
            PermissionsInstructionFragment {
                snapshot: self.sources.permission_snapshot(thread_id),
            }
            .render(&env)?,
            "slab_permissions",
        ));
        // 4. Reasoning-effort steer (only when an effort/verbosity is requested).
        if reasoning_effort.is_some() || verbosity.is_some() {
            messages.push(tagged(
                ReasoningEffortFragment { effort: reasoning_effort, verbosity }.render(&env)?,
                "slab_reasoning_effort",
            ));
        }
        // 5. Skills (developer) — only when there is something to render.
        //    An empty skills catalogue would inject a dead instruction block
        //    describing a skill system with zero entries; a model-provided
        //    instruction template still injects (it is an explicit override).
        if !skills.is_empty() || developer_template.is_some() {
            let mut developer = DeveloperInstructionFragment::new(skills, skill_roots);
            if let Some(template_source) = developer_template {
                developer.template_source = template_source;
            }
            messages.push(tagged(developer.render(&env)?, "slab_skills"));
        }
        // 6. Folded read-side memory (developer, preserves the `slab_memory` name).
        if let Some(memory) = self.sources.memory_context(thread_id, model, input_message).await {
            messages.push(ConversationMessage {
                role: "developer".to_owned(),
                content: ConversationMessageContent::Text(memory.body),
                name: Some("slab_memory".to_owned()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            });
            // 6b. Recall-selected rollout summaries (developer): a separate
            // tag so the summary fragment and the per-task recall refresh
            // independently between runs.
            if let Some(relevant) = memory.relevant_body {
                messages.push(ConversationMessage {
                    role: "developer".to_owned(),
                    content: ConversationMessageContent::Text(relevant),
                    name: Some("slab_memory_relevant".to_owned()),
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                });
            }
        }
        // 7. Discovered `AGENTS.md` bodies (user).
        for AgentMdRecord { path, body } in agents_md {
            messages.push(tagged(
                AgentMdFragment { path: path.to_string_lossy().into_owned(), body }.render(&env)?,
                "slab_agents_md",
            ));
        }
        Ok(messages)
    }
}

/// Stamp a stable fragment tag on an injected message.
///
/// The thread-level merge (`slab_agent::thread::merge_injected_messages`)
/// REPLACES same-tagged messages on every run instead of inserting duplicates
/// — without the tag there is no identity to replace against, and the batch
/// re-appended once per user turn (the context-inflation bug).
fn tagged(mut message: ConversationMessage, name: &str) -> ConversationMessage {
    message.name = Some(name.to_owned());
    message
}

#[async_trait]
impl AgentHook for ContextInstructionHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        match event {
            HookEvent::OnAgentStart { thread_id, config, input_message, .. } => {
                if config.transient {
                    return HookOutcome::Continue;
                }
                match self
                    .render_startup_messages(
                        thread_id,
                        &config.model,
                        config.reasoning_effort,
                        config.verbosity,
                        input_message.as_deref(),
                    )
                    .await
                {
                    Ok(messages) if !messages.is_empty() => HookOutcome::Effects {
                        tool_action: HookToolAction::Continue,
                        injected_messages: messages,
                        observations: Vec::new(),
                    },
                    Ok(_) => HookOutcome::Continue,
                    Err(error) => HookOutcome::AppendObservation {
                        observation: format!("context instruction injection skipped: {error}"),
                    },
                }
            }
            // Release per-thread recall state when the thread finishes; the
            // surfaced-memory cache must not outlive its thread.
            HookEvent::OnAgentEnd { thread_id, .. } => {
                self.sources.evict_thread(thread_id);
                HookOutcome::Continue
            }
            _ => HookOutcome::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use async_trait::async_trait;
    use slab_agent::AgentConfig;

    use super::*;
    use crate::snapshots::{
        EnvironmentSnapshot, OsKind, PermissionBaselineLabel, PermissionModeLabel,
        PermissionSnapshot, ShellKind,
    };

    struct MockSources {
        workspace: Option<PathBuf>,
        skills_dir: PathBuf,
        agents_md: PathBuf,
        template: Option<String>,
        permission: PermissionSnapshot,
        memory: Option<String>,
        relevant: Option<String>,
        evicted: std::sync::Mutex<Vec<String>>,
    }

    fn permissive_snapshot() -> PermissionSnapshot {
        PermissionSnapshot {
            mode: PermissionModeLabel::FullControl,
            baseline: PermissionBaselineLabel::FullAccess,
            read_only: false,
            shell_allowed: true,
            file_write_allowed: true,
            network_allowed: true,
        }
    }

    #[async_trait]
    impl AgentContextSources for MockSources {
        fn workspace_root(&self) -> Option<PathBuf> {
            self.workspace.clone()
        }
        fn app_home_skills_dir(&self) -> PathBuf {
            self.skills_dir.clone()
        }
        fn app_home_agents_md(&self) -> PathBuf {
            self.agents_md.clone()
        }
        async fn instruction_template_for(&self, _model_id: &str) -> Option<String> {
            self.template.clone()
        }
        fn environment_snapshot(&self) -> EnvironmentSnapshot {
            EnvironmentSnapshot {
                cwd: self.workspace.as_ref().map(|p| p.to_string_lossy().into_owned()),
                shell: ShellKind::Bash,
                os: OsKind::Linux,
                timestamp: "2026-07-24T00:00:00Z".to_owned(),
            }
        }
        fn permission_snapshot(&self, _thread_id: &str) -> PermissionSnapshot {
            self.permission.clone()
        }
        async fn memory_context(
            &self,
            _thread_id: &str,
            _model_id: &str,
            _input_message: Option<&str>,
        ) -> Option<crate::snapshots::MemoryContext> {
            self.memory.as_ref().map(|body| crate::snapshots::MemoryContext {
                body: body.clone(),
                relevant_body: self.relevant.clone(),
            })
        }
        fn evict_thread(&self, thread_id: &str) {
            self.evicted.lock().expect("evicted lock").push(thread_id.to_owned());
        }
    }

    fn write_skill(root: &Path, name: &str, body: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    fn mock_sources(workspace: Option<PathBuf>) -> MockSources {
        let global = tempfile::TempDir::new().unwrap();
        MockSources {
            workspace,
            skills_dir: global.path().join("skills"),
            agents_md: global.path().join("AGENTS.md"),
            template: None,
            permission: permissive_snapshot(),
            memory: None,
            relevant: None,
            evicted: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn start_event(config: AgentConfig) -> HookEvent {
        HookEvent::OnAgentStart {
            thread_id: "t".into(),
            session_id: "s".into(),
            parent_id: None,
            depth: 0,
            config: Box::new(config),
            input_message: Some("help me with the parser bug".to_owned()),
        }
    }

    #[tokio::test]
    async fn emits_ordered_context_then_agents_md_on_agent_start() {
        let ws = tempfile::TempDir::new().unwrap();
        write_skill(
            &ws.path().join(".agents").join("skills"),
            "rust-code-style",
            "---\nname: rust-code-style\ndescription: Use when working on Rust code.\n---\nbody\n",
        );
        std::fs::write(ws.path().join("AGENTS.md"), "# workspace rules\n").unwrap();

        let hook =
            ContextInstructionHook::new(Arc::new(mock_sources(Some(ws.path().to_path_buf()))));

        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        // system, environment, permissions, developer(skills), agents_md = 5.
        assert_eq!(injected_messages.len(), 5);
        assert_eq!(injected_messages[0].role, "system");
        assert_eq!(injected_messages[1].role, "developer");
        assert!(injected_messages[1].content.rendered_text().contains("<environment_context>"));
        assert_eq!(injected_messages[2].role, "developer");
        assert!(
            injected_messages[2].content.rendered_text().contains("<permissions_instructions>")
        );
        let skills_msg = injected_messages
            .iter()
            .find(|m| m.content.rendered_text().contains("skill://rust-code-style"))
            .expect("skills developer message should be injected");
        assert_eq!(skills_msg.role, "developer");
        let agents_msg = injected_messages
            .iter()
            .find(|m| m.content.rendered_text().contains("# workspace rules"))
            .expect("agents_md user message should be injected");
        assert_eq!(agents_msg.role, "user");
    }

    #[tokio::test]
    async fn skips_skills_fragment_when_no_skills_exist() {
        let ws = tempfile::TempDir::new().unwrap();
        // No SKILL.md written anywhere: workspace + global roots are empty.
        let hook =
            ContextInstructionHook::new(Arc::new(mock_sources(Some(ws.path().to_path_buf()))));

        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        // system, environment, permissions, agents_md(root AGENTS.md absent) = 3;
        // the skills developer message must NOT be injected for an empty catalogue.
        assert!(
            injected_messages.iter().all(|m| m.name.as_deref() != Some("slab_skills")),
            "no slab_skills message should be injected when no skills exist"
        );
    }

    #[tokio::test]
    async fn injects_skills_fragment_for_model_template_even_without_skills() {
        let ws = tempfile::TempDir::new().unwrap();
        let mut sources = mock_sources(Some(ws.path().to_path_buf()));
        sources.template = Some("<skills_instructions>custom</skills_instructions>".to_owned());
        let hook = ContextInstructionHook::new(Arc::new(sources));

        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        let skills_msg = injected_messages
            .iter()
            .find(|m| m.name.as_deref() == Some("slab_skills"))
            .expect("model-provided template must still be injected");
        assert!(skills_msg.content.rendered_text().contains("custom"));
    }

    #[tokio::test]
    async fn system_prompt_apply_patch_guidance_follows_workspace_binding() {
        // Without a workspace root the workspace-bound tools (apply_patch …)
        // are not registered, so the system prompt must not reference them.
        let unbound_event = start_event(AgentConfig::default());
        let unbound_hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let HookOutcome::Effects { injected_messages, .. } =
            unbound_hook.on_event(&unbound_event).await
        else {
            panic!("expected effects");
        };
        let system = injected_messages[0].content.rendered_text();
        assert!(!system.contains("apply_patch"), "unbound system prompt: {system}");
        assert!(system.contains("no workspace root is configured"));

        // With a workspace root the guidance is present.
        let ws = tempfile::TempDir::new().unwrap();
        let bound_event = start_event(AgentConfig::default());
        let bound_hook =
            ContextInstructionHook::new(Arc::new(mock_sources(Some(ws.path().to_path_buf()))));
        let HookOutcome::Effects { injected_messages, .. } =
            bound_hook.on_event(&bound_event).await
        else {
            panic!("expected effects");
        };
        let system = injected_messages[0].content.rendered_text();
        assert!(system.contains("apply_patch"));
        assert!(system.contains("in the workspace root"));
    }

    #[tokio::test]
    async fn environment_fragment_explains_missing_workspace_instead_of_bare_unset() {
        let hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;
        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects");
        };
        let env = injected_messages[1].content.rendered_text();
        assert!(env.contains("no workspace configured"), "env fragment: {env}");
        assert!(!env.contains("(unset)"));
    }

    #[tokio::test]
    async fn folds_memory_read_context_as_slab_memory_developer_message() {
        let ws = tempfile::TempDir::new().unwrap();
        let mut sources = mock_sources(Some(ws.path().to_path_buf()));
        sources.memory = Some("memory summary body".to_owned());
        let hook = ContextInstructionHook::new(Arc::new(sources));

        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        let memory_msg = injected_messages
            .iter()
            .find(|m| m.name.as_deref() == Some("slab_memory"))
            .expect("folded memory message should be injected");
        assert_eq!(memory_msg.role, "developer");
        assert!(memory_msg.content.rendered_text().contains("memory summary body"));
    }

    #[tokio::test]
    async fn folds_relevant_memory_as_slab_memory_relevant_fragment() {
        let ws = tempfile::TempDir::new().unwrap();
        let mut sources = mock_sources(Some(ws.path().to_path_buf()));
        sources.memory = Some("memory summary body".to_owned());
        sources.relevant = Some("relevant rollout summaries".to_owned());
        let hook = ContextInstructionHook::new(Arc::new(sources));

        let outcome = hook.on_event(&start_event(AgentConfig::default())).await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        let memory_position = injected_messages
            .iter()
            .position(|m| m.name.as_deref() == Some("slab_memory"))
            .expect("summary fragment present");
        let relevant_position = injected_messages
            .iter()
            .position(|m| m.name.as_deref() == Some("slab_memory_relevant"))
            .expect("relevant fragment present");
        let relevant_msg = &injected_messages[relevant_position];
        assert_eq!(relevant_msg.role, "developer");
        assert!(relevant_msg.content.rendered_text().contains("relevant rollout summaries"));
        // 6b sits immediately after slot 6.
        assert_eq!(relevant_position, memory_position + 1);
    }

    #[tokio::test]
    async fn evicts_thread_on_agent_end() {
        let sources = Arc::new(mock_sources(None));
        let hook =
            ContextInstructionHook::new(Arc::clone(&sources) as Arc<dyn AgentContextSources>);

        let outcome = hook
            .on_event(&HookEvent::OnAgentEnd {
                thread_id: "t".into(),
                session_id: "s".into(),
                status: slab_agent::ThreadStatus::Completed,
                error: None,
            })
            .await;

        assert!(matches!(outcome, HookOutcome::Continue));
        assert_eq!(
            sources.evicted.lock().expect("evicted lock").as_slice(),
            ["t".to_owned()].as_slice()
        );
    }

    #[tokio::test]
    async fn skips_transient_threads() {
        let hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let outcome = hook
            .on_event(&start_event(AgentConfig { transient: true, ..AgentConfig::default() }))
            .await;
        assert!(matches!(outcome, HookOutcome::Continue));
    }

    #[tokio::test]
    async fn non_start_events_continue() {
        let hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let outcome = hook
            .on_event(&HookEvent::OnLlmStart {
                thread_id: "t".into(),
                session_id: "s".into(),
                turn_index: 0,
                messages: Vec::new(),
                tools: Vec::new(),
            })
            .await;
        assert!(matches!(outcome, HookOutcome::Continue));
    }

    #[tokio::test]
    async fn emits_reasoning_effort_fragment_when_effort_is_set() {
        let hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let outcome = hook
            .on_event(&start_event(AgentConfig {
                reasoning_effort: Some(slab_types::ChatReasoningEffort::High),
                ..AgentConfig::default()
            }))
            .await;

        let HookOutcome::Effects { injected_messages, .. } = outcome else {
            panic!("expected effects, got {outcome:?}");
        };
        let reasoning_msg = injected_messages
            .iter()
            .find(|m| m.content.rendered_text().contains("<reasoning_effort>"))
            .expect("reasoning-effort fragment should be injected when effort is set");
        assert_eq!(reasoning_msg.role, "developer");
        let body = reasoning_msg.content.rendered_text();
        assert!(body.contains("Reason carefully and thoroughly"));
        // No fake <think> compat text.
        assert!(!body.contains("<think>"));
    }
}
