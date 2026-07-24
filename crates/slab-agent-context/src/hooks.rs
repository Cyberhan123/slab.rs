//! [`ContextInstructionHook`] — the `slab_agent::AgentHook` that injects
//! rendered context on agent start.
//!
//! On `OnAgentStart` it resolves the model's instruction template, scans
//! skills + `AGENTS.md`, snapshots the environment + permission state, and
//! injects a single ordered batch:
//! `[system, environment, permissions, …reasoning-effort(sliced later),
//! developer(skills), memory?, …agents_md]`. `system` is emitted first so the
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

        let developer_template = self
            .sources
            .instruction_template_for(model)
            .await
            .or_else(|| Some(crate::helper::DEFAULT_DEVELOPER_TEMPLATE.to_owned()));

        let mut messages = Vec::new();
        // 1. Identity / persona.
        messages.push(SystemInstructionFragment.render(&env)?);
        // 2. Environment facts (cwd / shell / os / time).
        messages.push(
            EnvironmentContextFragment { snapshot: self.sources.environment_snapshot() }
                .render(&env)?,
        );
        // 3. Permission instructions + tool-use policy.
        messages.push(
            PermissionsInstructionFragment {
                snapshot: self.sources.permission_snapshot(thread_id),
            }
            .render(&env)?,
        );
        // 4. Reasoning-effort steer (only when an effort/verbosity is requested).
        if reasoning_effort.is_some() || verbosity.is_some() {
            messages.push(
                ReasoningEffortFragment { effort: reasoning_effort, verbosity }.render(&env)?,
            );
        }
        // 5. Skills (developer).
        let mut developer = DeveloperInstructionFragment::new(skills, skill_roots);
        if let Some(template_source) = developer_template {
            developer.template_source = template_source;
        }
        messages.push(developer.render(&env)?);
        // 6. Folded read-side memory (developer, preserves the `slab_memory` name).
        if let Some(memory) = self.sources.memory_context() {
            messages.push(ConversationMessage {
                role: "developer".to_owned(),
                content: ConversationMessageContent::Text(memory.body),
                name: Some("slab_memory".to_owned()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            });
        }
        // 7. Discovered `AGENTS.md` bodies (user).
        for AgentMdRecord { path, body } in agents_md {
            messages.push(
                AgentMdFragment { path: path.to_string_lossy().into_owned(), body }.render(&env)?,
            );
        }
        Ok(messages)
    }
}

#[async_trait]
impl AgentHook for ContextInstructionHook {
    async fn on_event(&self, event: &HookEvent) -> HookOutcome {
        let HookEvent::OnAgentStart { thread_id, config, .. } = event else {
            return HookOutcome::Continue;
        };
        if config.transient {
            return HookOutcome::Continue;
        }
        match self
            .render_startup_messages(
                thread_id,
                &config.model,
                config.reasoning_effort,
                config.verbosity,
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
        fn memory_context(&self) -> Option<crate::snapshots::MemoryContext> {
            self.memory.as_ref().map(|body| crate::snapshots::MemoryContext { body: body.clone() })
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

        let outcome = hook
            .on_event(&HookEvent::OnAgentStart {
                thread_id: "t".into(),
                session_id: "s".into(),
                parent_id: None,
                depth: 0,
                config: AgentConfig::default(),
            })
            .await;

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
    async fn folds_memory_read_context_as_slab_memory_developer_message() {
        let ws = tempfile::TempDir::new().unwrap();
        let mut sources = mock_sources(Some(ws.path().to_path_buf()));
        sources.memory = Some("memory summary body".to_owned());
        let hook = ContextInstructionHook::new(Arc::new(sources));

        let outcome = hook
            .on_event(&HookEvent::OnAgentStart {
                thread_id: "t".into(),
                session_id: "s".into(),
                parent_id: None,
                depth: 0,
                config: AgentConfig::default(),
            })
            .await;

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
    async fn skips_transient_threads() {
        let hook = ContextInstructionHook::new(Arc::new(mock_sources(None)));
        let outcome = hook
            .on_event(&HookEvent::OnAgentStart {
                thread_id: "t".into(),
                session_id: "s".into(),
                parent_id: None,
                depth: 0,
                config: AgentConfig { transient: true, ..AgentConfig::default() },
            })
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
            .on_event(&HookEvent::OnAgentStart {
                thread_id: "t".into(),
                session_id: "s".into(),
                parent_id: None,
                depth: 0,
                config: AgentConfig {
                    reasoning_effort: Some(slab_types::ChatReasoningEffort::High),
                    ..AgentConfig::default()
                },
            })
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
