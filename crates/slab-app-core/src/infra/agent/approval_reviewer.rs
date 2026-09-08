//! Model-based approval reviewer for the "approve for me" permission mode.
//!
//! When a thread runs under `PermissionMode::ApproveForMe` and the harness
//! `turn/start` carried an `approval_model`, `RequireApproval` verdicts are
//! first routed here: the configured model reviews the pending operation in
//! ONE bounded chat call (system prompt = the built-in
//! [`slab_agent_context`] approval-review template + the user's injected
//! policy) and answers APPROVE/DENY. Every failure mode — wrong mode, no
//! configuration, LLM error, timeout, unparseable output — reports
//! [`ReviewOutcome::Unavailable`] so the kernel falls back to the human
//! approval card; the human path always remains the safety net.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::config::{AgentConfig, AgentToolChoice};
use slab_agent::port::{ApprovalReviewRequest, ApprovalReviewerPort, ReviewOutcome, ToolRiskLevel};
use slab_agent::{ExecPolicyPort, LlmPort};
use slab_agent_tracing::AgentTraceContext;
use slab_exec_policy::PermissionMode;
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;

/// Review timeout budget. MUST stay well under the human approval timeout
/// (`AgentEventHub::APPROVAL_TIMEOUT_SECS = 300`) so a failed review still
/// leaves most of the human window.
const REVIEW_TIMEOUT_SECS: u64 = 60;

/// Arguments passed to the review prompt are truncated to this many chars —
/// the reviewer needs intent, not the full payload.
const MAX_REVIEW_ARGUMENT_CHARS: usize = 2_000;

/// Reviewer reasons are capped in the output contract; enforce it on parse.
const MAX_REVIEW_REASON_CHARS: usize = 200;

/// Per-thread reviewer configuration (flows from harness `turn/start`).
#[derive(Debug, Clone)]
struct ApprovalReviewConfig {
    model: String,
    prompt: Option<String>,
}

/// [`ApprovalReviewerPort`] implementation delegating approval decisions to a
/// single-shot model review. Shares the `ServerLlmAdapter` Arc the agent
/// control uses (arbitrary model ids, including cloud), and gates on the
/// thread's permission mode via the shared [`ExecPolicyPort`].
pub(crate) struct ModelApprovalReviewer {
    llm: Arc<dyn LlmPort>,
    exec_policy: Arc<dyn ExecPolicyPort>,
    configs: DashMap<String, ApprovalReviewConfig>,
}

impl ModelApprovalReviewer {
    pub(crate) fn new(llm: Arc<dyn LlmPort>, exec_policy: Arc<dyn ExecPolicyPort>) -> Self {
        Self { llm, exec_policy, configs: DashMap::new() }
    }

    fn config_for(&self, thread_id: &str) -> Option<ApprovalReviewConfig> {
        self.configs.get(thread_id).map(|entry| entry.clone())
    }

    fn build_messages(
        &self,
        request: &ApprovalReviewRequest,
        custom_policy: &str,
    ) -> Result<Vec<ConversationMessage>, slab_agent_context::ContextError> {
        let descriptor = &request.descriptor;
        let system = slab_agent_context::render_approval_review_prompt(
            &slab_agent_context::ApprovalReviewContext {
                tool_name: request.tool_name.clone(),
                category: descriptor.category.as_str().to_owned(),
                display: request.display.clone(),
                arguments: slab_agent_context::helper::truncate(
                    &request.arguments,
                    MAX_REVIEW_ARGUMENT_CHARS,
                ),
                risk_level: risk_level_label(request.risk.as_ref().map(|risk| risk.level)),
                risk_labels: request
                    .risk
                    .as_ref()
                    .map(|risk| risk.labels.clone())
                    .unwrap_or_default(),
                workspace_root: descriptor
                    .workspace_root
                    .as_ref()
                    .map(|root| root.to_string_lossy().into_owned()),
                custom_policy: custom_policy.to_owned(),
            },
        )?;
        // One compact operation summary line as the user turn — the system
        // prompt carries the full context and the output contract.
        let user =
            format!("Pending operation for review: {} — {}", request.tool_name, request.display);
        Ok(vec![
            ConversationMessage {
                role: "system".to_owned(),
                content: ConversationMessageContent::Text(system),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            ConversationMessage {
                role: "user".to_owned(),
                content: ConversationMessageContent::Text(user),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
        ])
    }

    async fn review_with_config(
        &self,
        thread_id: &str,
        request: &ApprovalReviewRequest,
        config: &ApprovalReviewConfig,
    ) -> ReviewOutcome {
        let messages = match self.build_messages(request, config.prompt.as_deref().unwrap_or("")) {
            Ok(messages) => messages,
            Err(error) => {
                warn!(%error, thread_id, "approval review prompt render failed");
                return ReviewOutcome::Unavailable;
            }
        };
        // Minimal no-tools single-shot config: deterministic temperature,
        // transient (no root-start background work), no tool use possible.
        let review_config = AgentConfig {
            model: config.model.clone(),
            temperature: Some(0.0),
            transient: true,
            tool_choice: AgentToolChoice::None,
            ..AgentConfig::default()
        };
        let trace_context = AgentTraceContext::new("approval-review").with_thread(thread_id);
        let call =
            self.llm.chat_completion(&config.model, &messages, &[], &review_config, &trace_context);
        let response = match tokio::time::timeout(Duration::from_secs(REVIEW_TIMEOUT_SECS), call)
            .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                warn!(%error, thread_id, model = %config.model, "approval review LLM call failed");
                return ReviewOutcome::Unavailable;
            }
            Err(_) => {
                warn!(
                    thread_id,
                    model = %config.model,
                    timeout_secs = REVIEW_TIMEOUT_SECS,
                    "approval review LLM call timed out"
                );
                return ReviewOutcome::Unavailable;
            }
        };
        match parse_review_verdict(response.content.as_deref().unwrap_or("")) {
            Some((true, reason)) => ReviewOutcome::Approved { reason },
            Some((false, reason)) => ReviewOutcome::Rejected {
                reason: reason.unwrap_or_else(|| "denied by the approval model".to_owned()),
            },
            None => {
                warn!(
                    thread_id,
                    model = %config.model,
                    "approval review output unparseable; falling back to human approval"
                );
                ReviewOutcome::Unavailable
            }
        }
    }
}

#[async_trait]
impl ApprovalReviewerPort for ModelApprovalReviewer {
    async fn review(&self, thread_id: &str, request: &ApprovalReviewRequest) -> ReviewOutcome {
        // Mode gate: reviewer delegation ONLY applies to the "approve for me"
        // mode. Checked before anything else so misconfigured threads never
        // spend an LLM call.
        if !matches!(
            self.exec_policy.permission_state_for(thread_id).mode,
            PermissionMode::ApproveForMe
        ) {
            return ReviewOutcome::Unavailable;
        }
        let Some(config) = self.config_for(thread_id) else {
            return ReviewOutcome::Unavailable;
        };
        self.review_with_config(thread_id, request, &config).await
    }

    async fn set_thread_config(&self, thread_id: &str, model: Option<&str>, prompt: Option<&str>) {
        let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) else {
            self.configs.remove(thread_id);
            return;
        };
        let prompt = prompt.map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned);
        self.configs
            .insert(thread_id.to_owned(), ApprovalReviewConfig { model: model.to_owned(), prompt });
    }
}

fn risk_level_label(level: Option<ToolRiskLevel>) -> String {
    match level {
        Some(ToolRiskLevel::Low) => "low".to_owned(),
        Some(ToolRiskLevel::Medium) => "medium".to_owned(),
        Some(ToolRiskLevel::High) => "high".to_owned(),
        None => "not assessed".to_owned(),
    }
}

/// Parse the reviewer model's answer against the output contract: the first
/// non-empty line must start with `APPROVE` or `DENY` (case-insensitive,
/// optionally followed by a same-line reason); otherwise the next non-empty
/// line is the reason. Returns `None` for anything unparseable (the caller
/// falls back to the human approval path).
fn parse_review_verdict(text: &str) -> Option<(bool, Option<String>)> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next()?;
    let mut tokens = first.split_whitespace();
    let keyword = tokens.next()?.trim_end_matches([':', '：']).to_ascii_uppercase();
    let same_line_reason: String = tokens.collect::<Vec<_>>().join(" ");
    let verdict = match keyword.as_str() {
        "APPROVE" => true,
        "DENY" => false,
        _ => return None,
    };
    let reason = if same_line_reason.is_empty() {
        lines.next().map(str::to_owned)
    } else {
        Some(same_line_reason)
    };
    let reason = reason
        .map(|reason| slab_agent_context::helper::truncate(reason.trim(), MAX_REVIEW_REASON_CHARS));
    Some((verdict, reason.filter(|value| !value.is_empty())))
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait as _;
    use slab_agent::error::AgentError;
    use slab_agent::port::LlmResponse;
    use slab_exec_policy::{
        ExecDecision, OperationDescriptor, PermissionMode, PermissionStateSnapshot, ToolExposure,
    };

    struct StubExecPolicy {
        mode: PermissionMode,
    }

    #[async_trait]
    impl ExecPolicyPort for StubExecPolicy {
        async fn evaluate(
            &self,
            _thread_id: &str,
            _descriptor: &OperationDescriptor,
        ) -> ExecDecision {
            ExecDecision::Allow
        }
        async fn remember(
            &self,
            _thread_id: &str,
            _descriptor: &OperationDescriptor,
            _scope: slab_exec_policy::ApprovalScope,
        ) {
        }
        async fn set_thread_mode(&self, _thread_id: &str, _mode: PermissionMode) {}
        async fn clear_thread(&self, _thread_id: &str) {}
        fn permission_state_for(&self, _thread_id: &str) -> PermissionStateSnapshot {
            PermissionStateSnapshot {
                mode: self.mode,
                baseline: slab_exec_policy::PermissionBaseline::WorkspaceWrite,
                exposure: ToolExposure::all(),
            }
        }
    }

    /// Scripted LLM stub: returns canned content (or errors when `fail` is
    /// set) and counts calls.
    struct SharedStubLlm {
        content: Option<String>,
        fail: bool,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl SharedStubLlm {
        fn ok(content: &str) -> Self {
            Self {
                content: Some(content.to_owned()),
                fail: false,
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn failing() -> Self {
            Self { content: None, fail: true, calls: std::sync::atomic::AtomicUsize::new(0) }
        }

        fn call_count(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LlmPort for SharedStubLlm {
        async fn chat_completion(
            &self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[slab_agent::port::ToolSpec],
            _config: &AgentConfig,
            _trace_context: &AgentTraceContext,
        ) -> Result<LlmResponse, AgentError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail {
                return Err(AgentError::Llm("boom".to_owned()));
            }
            Ok(LlmResponse {
                content: self.content.clone(),
                content_already_streamed: false,
                tool_calls: Vec::new(),
                finish_reason: Some("stop".to_owned()),
                usage: None,
            })
        }
    }

    fn request() -> ApprovalReviewRequest {
        ApprovalReviewRequest {
            tool_name: "run_shell".to_owned(),
            display: "cargo test -p slab-agent".to_owned(),
            descriptor: OperationDescriptor {
                category: slab_exec_policy::OperationCategory::Shell,
                subject: "cargo test -p slab-agent".to_owned(),
                detail: None,
                workspace_root: None,
                tool_name: Some("run_shell".to_owned()),
            },
            arguments: r#"{ "command": "cargo test -p slab-agent" }"#.to_owned(),
            risk: None,
        }
    }

    #[tokio::test]
    async fn wrong_mode_returns_unavailable_without_llm_call() {
        let llm = Arc::new(SharedStubLlm::ok("APPROVE\nfine"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::RequestApproval }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), None).await;
        assert_eq!(reviewer.review("t1", &request()).await, ReviewOutcome::Unavailable);
        assert_eq!(llm.call_count(), 0);
    }

    #[tokio::test]
    async fn unconfigured_thread_returns_unavailable_without_llm_call() {
        let llm = Arc::new(SharedStubLlm::ok("APPROVE\nfine"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        assert_eq!(reviewer.review("t1", &request()).await, ReviewOutcome::Unavailable);
        assert_eq!(llm.call_count(), 0);
    }

    #[tokio::test]
    async fn approve_verdict_maps_to_approved_with_reason() {
        let llm = Arc::new(SharedStubLlm::ok("APPROVE\nsafe test command"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), Some("no git push")).await;
        assert_eq!(
            reviewer.review("t1", &request()).await,
            ReviewOutcome::Approved { reason: Some("safe test command".to_owned()) }
        );
        assert_eq!(llm.call_count(), 1);
    }

    #[tokio::test]
    async fn deny_verdict_maps_to_rejected_with_reason() {
        let llm = Arc::new(SharedStubLlm::ok("DENY: touches credentials"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), None).await;
        assert_eq!(
            reviewer.review("t1", &request()).await,
            ReviewOutcome::Rejected { reason: "touches credentials".to_owned() }
        );
    }

    #[tokio::test]
    async fn llm_error_returns_unavailable() {
        let llm = Arc::new(SharedStubLlm::failing());
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), None).await;
        assert_eq!(reviewer.review("t1", &request()).await, ReviewOutcome::Unavailable);
    }

    #[tokio::test]
    async fn unparseable_output_returns_unavailable() {
        let llm = Arc::new(SharedStubLlm::ok("I think this is probably fine, let me explain…"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), None).await;
        assert_eq!(reviewer.review("t1", &request()).await, ReviewOutcome::Unavailable);
    }

    #[tokio::test]
    async fn clearing_config_disables_review() {
        let llm = Arc::new(SharedStubLlm::ok("APPROVE\nfine"));
        let port: Arc<dyn LlmPort> = llm.clone();
        let reviewer = ModelApprovalReviewer::new(
            port,
            Arc::new(StubExecPolicy { mode: PermissionMode::ApproveForMe }),
        );
        reviewer.set_thread_config("t1", Some("reviewer-model"), None).await;
        reviewer.set_thread_config("t1", None, None).await;
        assert_eq!(reviewer.review("t1", &request()).await, ReviewOutcome::Unavailable);
        assert_eq!(llm.call_count(), 0);
    }

    #[test]
    fn parse_review_verdict_contract() {
        // Exact keyword, reason on the next line.
        assert_eq!(
            parse_review_verdict("\n\nAPPROVE\nsafe command\n"),
            Some((true, Some("safe command".to_owned())))
        );
        // Case-insensitive, reason on the same line with a colon.
        assert_eq!(
            parse_review_verdict("deny: destructive"),
            Some((false, Some("destructive".to_owned())))
        );
        // Keyword without a reason.
        assert_eq!(parse_review_verdict("APPROVE"), Some((true, None)));
        // Garbage first line → unparseable.
        assert_eq!(parse_review_verdict("sounds good to me"), None);
        // Empty → unparseable.
        assert_eq!(parse_review_verdict(""), None);
        assert_eq!(parse_review_verdict("   \n  \n"), None);
        // Reason capped at 200 chars.
        let (verdict, reason) =
            parse_review_verdict(&format!("DENY\n{}", "x".repeat(500))).unwrap();
        assert!(!verdict);
        assert_eq!(reason.unwrap().chars().count(), MAX_REVIEW_REASON_CHARS);
    }
}
