//! [`ContextInstructionHook`] — the `slab_agent::AgentHook` that injects
//! rendered context on agent start.
//!
//! On `OnAgentStart` it resolves the model's instruction template, scans
//! skills + `AGENTS.md`, and injects `[system, developer, …agents_md]`
//! messages in that order. `system` is emitted first so the existing
//! thread-level insertion logic (which places injected messages after the
//! leading `system` block) lands `developer` and the `AGENTS.md` user messages
//! in the right positions. Skill-body expansion is NOT done here — hooks can
//! only inject new messages, not mutate user turns — so the server expands
//! invoked skills before the turn starts (see `slab-server` harness).

use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::{AgentHook, HookEvent, HookOutcome, HookToolAction};
use slab_types::ConversationMessage;

use crate::agent_md_manager::{AgentMdRecord, scan_agents_md};
use crate::developer_instruction::DeveloperInstructionFragment;
use crate::error::Result;
use crate::fragment::ContextFragment;
use crate::helper::{build_environment, build_skill_roots};
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

    async fn render_startup_messages(&self, model: &str) -> Result<Vec<ConversationMessage>> {
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

        let mut messages = Vec::with_capacity(2 + agents_md.len());
        messages.push(SystemInstructionFragment.render(&env)?);
        let mut developer = DeveloperInstructionFragment::new(skills, skill_roots);
        if let Some(template_source) = developer_template {
            developer.template_source = template_source;
        }
        messages.push(developer.render(&env)?);
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
        let HookEvent::OnAgentStart { config, .. } = event else {
            return HookOutcome::Continue;
        };
        if config.transient {
            return HookOutcome::Continue;
        }
        match self.render_startup_messages(&config.model).await {
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

    struct MockSources {
        workspace: Option<PathBuf>,
        skills_dir: PathBuf,
        agents_md: PathBuf,
        template: Option<String>,
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
    }

    fn write_skill(root: &Path, name: &str, body: &str) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    #[tokio::test]
    async fn emits_system_then_developer_on_agent_start() {
        let ws = tempfile::TempDir::new().unwrap();
        let global = tempfile::TempDir::new().unwrap();
        write_skill(
            &ws.path().join(".agents").join("skills"),
            "rust-code-style",
            "---\nname: rust-code-style\ndescription: Use when working on Rust code.\n---\nbody\n",
        );
        std::fs::write(ws.path().join("AGENTS.md"), "# workspace rules\n").unwrap();

        let hook = ContextInstructionHook::new(Arc::new(MockSources {
            workspace: Some(ws.path().to_path_buf()),
            skills_dir: global.path().join("skills"),
            agents_md: global.path().join("AGENTS.md"),
            template: None,
        }));

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
        // system, developer, agents_md (workspace) = 3 messages.
        assert_eq!(injected_messages.len(), 3);
        assert_eq!(injected_messages[0].role, "system");
        assert_eq!(injected_messages[1].role, "developer");
        assert!(injected_messages[1].content.rendered_text().contains("skill://rust-code-style"));
        assert_eq!(injected_messages[2].role, "user");
        assert!(injected_messages[2].content.rendered_text().contains("# workspace rules"));
    }

    #[tokio::test]
    async fn skips_transient_threads() {
        let global = tempfile::TempDir::new().unwrap();
        let hook = ContextInstructionHook::new(Arc::new(MockSources {
            workspace: None,
            skills_dir: global.path().join("skills"),
            agents_md: global.path().join("AGENTS.md"),
            template: None,
        }));
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
        let global = tempfile::TempDir::new().unwrap();
        let hook = ContextInstructionHook::new(Arc::new(MockSources {
            workspace: None,
            skills_dir: global.path().join("skills"),
            agents_md: global.path().join("AGENTS.md"),
            template: None,
        }));
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
}
