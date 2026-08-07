use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_agent::{
    AgentControl, AgentRuntime, AgentThreadContext, PlanStorePort, ToolRouter, WorkspaceRef,
};
use slab_agent_tracing::{AgentTraceSink, BundleAgentTraceSink, NoopAgentTraceSink};
use slab_sandboxing::{SandboxEnvironment, SandboxPolicy, create_platform_driver};

use crate::context::AppContext;
use crate::domain::services::agent::AgentCore;
use crate::domain::services::{
    HarnessService, PluginService, ResponseService, WorkspaceLspService,
};
use crate::infra::db::AnyStore;

use super::event_hub::AgentEventHub;
use super::runtime::AgentRuntimeReloader;

pub(crate) struct AgentBootstrap {
    pub(crate) harness: HarnessService,
    pub(crate) response: ResponseService,
    pub(crate) runtime: AgentRuntimeReloader,
}

pub(crate) fn build_agent_bootstrap(ctx: &AppContext, store: Arc<AnyStore>) -> AgentBootstrap {
    let settings = ctx.pmid.config();
    // Compute the trace directory ONCE from `agent.debug`. The
    // trace sink + (future) trace bundle depend on `agent.debug` ALONE (no
    // longer on `telemetry.enabled`); OTel provider assembly is a separate
    // switch still gated by `telemetry.enabled` in the binary init. This dir is
    // also threaded into the rollout store so a root thread's SessionMeta carries
    // it as `trace_path` (rollout ↔ trace coordination).
    //
    // The decision goes through the pure `agent_trace_enabled` gate so the
    // independence contract (agent.debug alone, telemetry.enabled ignored) is
    // unit-tested — see `agent_trace_gate_is_independent_of_telemetry_enabled`.
    let trace_dir: Option<PathBuf> =
        if agent_trace_enabled(settings.agent.debug, settings.telemetry.enabled) {
            Some(agent_trace_log_dir(ctx))
        } else {
            None
        };
    // Rollout JSONL true source. One shared file store for the whole
    // process; one recorder per thread, files under <app_home>/sessions in the
    // date-partitioned layout `YYYY/MM/DD/rollout-<ts>-<thread_id>.jsonl`.
    let rollout =
        Arc::new(slab_agent_rollout::RolloutFileStore::new(slab_utils::app_home::sessions_dir()));
    // One-shot startup migration of pre-migration FLAT rollout
    // files (`<thread_id>.rollout.jsonl` at the sessions root) into the new
    // date-partitioned layout. Runs synchronously BEFORE any recorder is spawned
    // (the adapter below spawns recorders lazily on first write), so there is no
    // race between a live writer and the rename. Idempotent + crash-safe: a
    // second boot finds no flat files; a crash mid-rename leaves the file at one
    // of the two paths and the next boot picks it up.
    let migrated = rollout.migrate_flat_rollouts();
    if migrated > 0 {
        tracing::info!(migrated, "rollout flat files migrated to date-partitioned layout");
    }
    // The ONLY AgentStorePort wired into the runtime: rollout-backed, with
    // metadata delegated to the SQL store. The same SQL store also backs the
    // rollout-session index (list ghost-gate + new-thread mark). The legacy
    // conversation + audit tables and the startup backfill were dropped:
    // rollout is the sole conversation/turn-state/item source, so there is no
    // backfill to schedule.
    let rollout_store = Arc::new(super::rollout_store::RolloutBackedAgentStore::new(
        Arc::clone(&store) as Arc<dyn slab_agent::port::AgentStorePort>,
        Arc::clone(&store) as Arc<dyn crate::infra::db::repository::rollout_index::RolloutIndex>,
        Arc::clone(&rollout),
        trace_dir.clone(),
    ));
    let store_for_agent: Arc<dyn slab_agent::port::AgentStorePort> =
        rollout_store.clone() as Arc<dyn slab_agent::port::AgentStorePort>;
    let event_hub = Arc::new(AgentEventHub::new());
    // Only the event hub is wired as a notify port now. response_json
    // persistence was removed — the rollout true source plus SQL metadata
    // remain the store of record (see AgentCore / AgentStorePort).
    let composite_notify: Arc<dyn slab_agent::AgentNotifyPort> =
        Arc::new(super::event_hub::CompositeNotifyPort::new(vec![
            Arc::clone(&event_hub) as Arc<dyn slab_agent::AgentNotifyPort>
        ]));
    // One shared compaction policy (LLM-summarizing + trim fallback) for the
    // harness turn loop, manual `thread/compact/start`, and the HTTP paths.
    let compact: Arc<dyn slab_agent::CompactPort> = Arc::new(
        crate::domain::services::agent::SummarizingCompactPort::new((*ctx.model_state).clone()),
    );
    let control = build_agent_control(
        ctx,
        Arc::clone(&store),
        Arc::clone(&event_hub),
        composite_notify,
        Arc::clone(&compact),
        rollout_store.clone() as Arc<dyn slab_agent::port::AgentStorePort>,
        Arc::clone(&rollout),
        Arc::clone(&rollout_store),
        trace_dir.clone(),
    );
    let agent_runtime = AgentRuntime::new(control);
    let core = AgentCore::new(
        agent_runtime.clone(),
        store_for_agent,
        Arc::clone(&event_hub),
        compact,
        Arc::clone(&rollout),
        Arc::clone(&rollout_store)
            as Arc<dyn crate::domain::services::agent::RolloutConversationStore>,
        trace_dir,
    );
    let runtime = AgentRuntimeReloader::new(
        (*ctx.model_state).clone(),
        core.runtime(),
        rollout,
        rollout_store,
    );
    schedule_agent_runtime_reload(runtime.clone());
    let harness = HarnessService::new(core.clone());
    let response = ResponseService::new(core, (*ctx.model_state).clone());

    AgentBootstrap { harness, response, runtime }
}

fn schedule_agent_runtime_reload(agent_runtime: AgentRuntimeReloader) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn(async move {
        if let Err(error) = agent_runtime.reload().await {
            tracing::warn!(%error, "failed to reload agent runtime settings at startup");
        }
    });
}

/// Construct the [`slab_agent::AgentControl`] singleton, wiring up the port
/// adapters and registering built-in tools.
#[allow(clippy::too_many_arguments)]
fn build_agent_control(
    ctx: &AppContext,
    store: Arc<AnyStore>,
    event_hub: Arc<AgentEventHub>,
    notify_port: Arc<dyn slab_agent::AgentNotifyPort>,
    compact: Arc<dyn slab_agent::CompactPort>,
    store_adapter: Arc<dyn slab_agent::port::AgentStorePort>,
    rollout: Arc<slab_agent_rollout::RolloutFileStore>,
    rollout_store: Arc<super::rollout_store::RolloutBackedAgentStore>,
    trace_dir: Option<PathBuf>,
) -> Arc<AgentControl> {
    let llm = Arc::new(super::adapter::ServerLlmAdapter::new(Arc::clone(&ctx.model_state)));
    // memory_store / exec_db stay on the original SQL store (metadata +
    // memory pipeline); only the conversation/turn-state/item surface is backed
    // by rollout via store_adapter.
    let memory_store = Arc::clone(&store);
    let exec_db = Arc::clone(&store);
    let workspace_root = crate::domain::services::workspace_root_from_config(&ctx.config);
    let sandbox_driver = workspace_root.clone().and_then(|root| {
        let env = SandboxEnvironment::new(Some(root), SandboxPolicy::WorkspaceWrite);
        match create_platform_driver(env) {
            Ok(driver) => available_sandbox_driver(driver),
            Err(error) => {
                tracing::warn!(%error, "sandbox driver is unavailable; shell tool stays blocked");
                None
            }
        }
    });
    let mut tool_router = ToolRouter::new();
    let web_search_config = ctx.pmid.config().agent.tools.websearch;
    let shell_config = ctx.pmid.config().agent.tools.shell;
    let shell_launcher = match shell_config.launcher {
        slab_config::ShellLauncherKind::Auto => slab_agent_tools::ShellLauncher::Auto,
        slab_config::ShellLauncherKind::Bash => slab_agent_tools::ShellLauncher::Bash,
        slab_config::ShellLauncherKind::PowerShell => slab_agent_tools::ShellLauncher::PowerShell,
        slab_config::ShellLauncherKind::Cmd => slab_agent_tools::ShellLauncher::Cmd,
    };
    let mcp_client = build_agent_mcp_client(ctx);
    slab_agent_tools::register_all_tools(
        &mut tool_router,
        sandbox_driver,
        workspace_root.clone(),
        mcp_client,
        false,
        web_search_config,
        shell_launcher,
        shell_config.bash_path.clone(),
    );
    tool_router.register(Box::new(super::code_tools::CodeLspStatusTool::new(
        WorkspaceLspService::new(
            Arc::clone(&ctx.config),
            PluginService::new((*ctx.model_state).clone()),
        ),
    )));

    let tool_router = Arc::new(tool_router);
    let approval_port: Arc<dyn slab_agent::ApprovalPort> = event_hub;
    let settings = ctx.pmid.config();
    // Trace sink decouple: the trace sink gate is `agent.debug` ONLY (computed
    // upstream in `build_agent_bootstrap`, which is why `trace_dir` is already
    // an Option here). Two INDEPENDENT diagnostic switches:
    //   - `agent.debug`       → this trace sink + trace bundle (`slab-agent-tracing`,
    //                           decoupled from `slab-otel`). When on,
    //                           a `BundleAgentTraceSink` records every slab-agent
    //                           event into a per-root-thread bundle AND keeps the
    //                           legacy per-session JSONL + `slab_otel::session`
    //                           telemetry wire alive (the sink composes a
    //                           `FileAgentTraceSink` internally).
    //   - `telemetry.enabled` → OTel PROVIDER assembly + export, gated separately
    //                           in the server/app/runtime init (intentionally
    //                           untouched here). On `agent.debug` alone the user
    //                           now gets the trace bundle even with OTel off.
    let (trace, trace_dir): (Arc<dyn AgentTraceSink>, Option<PathBuf>) = match trace_dir {
        Some(dir) => (BundleAgentTraceSink::shared(dir.clone()), Some(dir)),
        None => (Arc::new(NoopAgentTraceSink), None),
    };

    let memory_config = ctx.pmid.config().agent.memories.clone();
    let memory_root = memory_config
        .memory_root
        .as_deref()
        .and_then(normalize_non_empty_path)
        .unwrap_or_else(|| slab_utils::app_home::app_home_dir().join("memories"));
    if memory_config.enabled {
        let extra_roots = vec![memory_root.clone()];
        tool_router.register(Box::new(slab_agent_tools::ReadFileTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        tool_router.register(Box::new(slab_agent_tools::WriteFileTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        tool_router.register(Box::new(slab_agent_tools::ListDirTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        tool_router.register(Box::new(slab_agent_tools::FileGlobTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots.clone(),
        )));
        tool_router.register(Box::new(slab_agent_tools::GrepTool::new_with_extra_roots(
            workspace_root.clone(),
            extra_roots,
        )));
    }
    let memory_pipeline = super::memory::AgentMemoryPipeline::new(
        memory_store,
        Arc::clone(&rollout),
        Arc::clone(&rollout_store),
        workspace_root.clone(),
        Arc::clone(&ctx.model_state),
        memory_config.clone(),
        memory_root.clone(),
    );
    let exec_baseline =
        super::exec_policy::baseline_from_config(ctx.pmid.config().agent.permissions.baseline);
    let exec_policy = super::exec_policy::build_exec_policy_engine(
        exec_baseline,
        ctx.config.exec_rules_dir.clone(),
        exec_db,
        workspace_root.clone(),
    );
    // The memory read-side instruction is now folded into the context hook
    // (AppContextSources::memory_context); only the write-side pipeline stays.
    let mut hooks: Vec<Arc<dyn slab_agent::AgentHook>> = vec![
        Arc::new(super::memory::AgentMemoryStartupHook::new(memory_pipeline.clone())),
        Arc::new(slab_agent_context::ContextInstructionHook::new(Arc::new(
            super::context::AppContextSources::new(
                (*ctx.model_state).clone(),
                super::context::shell_kind(shell_config.launcher),
                Arc::clone(&exec_policy),
                memory_config.enabled,
                memory_root.clone(),
            ),
        ))),
    ];
    hooks.push(Arc::new(super::sleep_inhibitor_hook::SleepInhibitorHook::new(Arc::clone(
        &ctx.pmid,
    ))));
    if let Some(script_hook) =
        super::hooks::registered_script_hook(&ctx.pmid.config().agent.hooks, &ctx.config)
    {
        hooks.push(script_hook);
    }

    let thread_context = workspace_root
        .clone()
        .map(|root| WorkspaceRef { root, session_id: None })
        .map(|workspace| {
            AgentThreadContext::new().with_workspace(workspace).with_offline(settings.agent.offline)
        })
        .unwrap_or_else(|| AgentThreadContext::new().with_offline(settings.agent.offline));
    // ADR-013: concurrency limits are configurable via settings
    // (agent.runtime.limits), defaulting to the historical 32/4 ceiling.
    let runtime_limits = ctx.pmid.config().agent.runtime.limits.clamped();
    let control = AgentControl::new_with_hooks_and_tracing(
        llm,
        store_adapter,
        notify_port,
        approval_port,
        Arc::clone(&tool_router),
        slab_agent::AgentControlLimits {
            max_threads: runtime_limits.max_threads as usize,
            max_depth: runtime_limits.max_depth,
        },
        hooks,
        trace,
        trace_dir,
    )
    .with_thread_context(thread_context)
    .with_exec_policy(exec_policy)
    // Plan interaction mode: per-thread in-memory plan store (the durable
    // source of truth for the `plan` / `update_plan` / `present_plan` tools).
    .with_plan_store(
        Arc::new(super::plan_store::InMemoryPlanStore::default()) as Arc<dyn PlanStorePort>
    )
    // INFRA-05: FIFO wait queue for agent spawns (0 ⇒ legacy reject-at-cap).
    .with_queue_capacity(runtime_limits.queue_capacity as usize)
    .with_compact(compact);
    // INFRA-05: optional memory circuit breaker. When an RSS threshold is
    // configured, sample the host process and pause spawns while tripped.
    let control = if let Some(threshold_mb) = runtime_limits.rss_threshold_mb {
        let breaker = Arc::new(super::memory_breaker::MemoryCircuitBreaker::new(
            threshold_mb as u64,
            std::time::Duration::from_secs(runtime_limits.cooldown_secs as u64),
        ));
        super::memory_breaker::spawn_memory_sampler(Arc::clone(&breaker));
        control.with_memory_pressure(Arc::new(super::memory_breaker::BreakerPressurePort::new(
            breaker,
        )))
    } else {
        control
    };
    let control = Arc::new(control);
    tool_router
        .register(Box::new(slab_agent_tools::DelegateSubagentTool::new(Arc::clone(&control))));
    memory_pipeline.set_control(Arc::clone(&control));
    control
}

/// Decouple contract: the agent trace directory (and therefore the
/// trace sink + trace bundle) is gated by `agent.debug` ALONE. `telemetry_enabled`
/// is accepted as an explicit parameter purely so the independence is
/// unit-testable. Returning `agent_debug` and intentionally ignoring
/// `telemetry_enabled` IS the contract — do not AND the two here: the bundle
/// must be recorded even when the OTel provider/export is off.
fn agent_trace_enabled(agent_debug: bool, telemetry_enabled: bool) -> bool {
    let _ = telemetry_enabled;
    agent_debug
}

fn agent_trace_log_dir(ctx: &AppContext) -> PathBuf {
    let settings = ctx.pmid.config();
    settings
        .telemetry
        .exporter
        .local_directory()
        .cloned()
        .or_else(|| settings.telemetry.trace_exporter.local_directory().cloned())
        .or_else(|| settings.logging.path.as_deref().and_then(normalize_non_empty_path))
        .or_else(|| {
            ctx.config.log_file.as_ref().and_then(|path| path.parent()).map(Path::to_path_buf)
        })
        .unwrap_or_else(slab_utils::app_home::logs_dir)
}

fn normalize_non_empty_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

fn build_agent_mcp_client(ctx: &AppContext) -> Option<Arc<slab_mcp::McpClient>> {
    let settings = ctx.pmid.config().agent.tools.mcp;
    if !settings.enabled {
        return None;
    }

    let client = Arc::new(slab_mcp::McpClient::new());
    let launchers = agent_mcp_client_config(&settings).servers;
    if !launchers.is_empty() {
        schedule_agent_mcp_connections(Arc::clone(&client), launchers);
    }
    Some(client)
}

fn schedule_agent_mcp_connections(
    client: Arc<slab_mcp::McpClient>,
    launchers: Vec<slab_mcp::McpServerLauncher>,
) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("agent MCP server launchers are configured, but no Tokio runtime is active");
        return;
    };
    handle.spawn(async move {
        for launcher in launchers {
            let server_name = launcher.name.clone();
            match client.connect_stdio(launcher).await {
                Ok(()) => {
                    tracing::info!(server = %server_name, "connected configured MCP stdio server");
                }
                Err(error) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %error,
                        "failed to connect configured MCP stdio server"
                    );
                }
            }
        }
    });
}

fn agent_mcp_client_config(settings: &slab_config::AgentMcpConfig) -> slab_mcp::McpClientConfig {
    agent_mcp_client_config_with_env(settings, |name| std::env::var(name))
}

fn agent_mcp_client_config_with_env<F>(
    settings: &slab_config::AgentMcpConfig,
    mut env_lookup: F,
) -> slab_mcp::McpClientConfig
where
    F: FnMut(&str) -> Result<String, std::env::VarError>,
{
    let mut servers = Vec::new();
    for server in &settings.servers {
        if !server.enabled {
            continue;
        }
        let name = server.name.trim();
        let command = server.command.trim();
        if name.is_empty() || command.is_empty() {
            tracing::warn!("skipping MCP server with empty name or command");
            continue;
        }

        let mut env = HashMap::new();
        for (target_name, env_value) in &server.env {
            let target_name = target_name.trim();
            let env_var = env_value.env_var.trim();
            if target_name.is_empty() || env_var.is_empty() {
                tracing::warn!(server = %name, "skipping MCP env mapping with empty name");
                continue;
            }
            match env_lookup(env_var) {
                Ok(value) => {
                    env.insert(target_name.to_owned(), value);
                }
                Err(error) => {
                    tracing::warn!(
                        server = %name,
                        env = %target_name,
                        env_var = %env_var,
                        error = %error,
                        "skipping unresolved MCP env var reference"
                    );
                }
            }
        }

        servers.push(slab_mcp::McpServerLauncher {
            name: name.to_owned(),
            command: command.to_owned(),
            args: server.args.clone(),
            env,
            cwd: server
                .cwd
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
        });
    }

    slab_mcp::McpClientConfig { servers }
}

fn available_sandbox_driver(
    driver: Arc<dyn slab_sandboxing::SandboxDriver>,
) -> Option<Arc<dyn slab_sandboxing::SandboxDriver>> {
    let status = driver.setup_status();
    if !status.available {
        tracing::warn!(
            details = %status.details,
            "sandbox driver is unavailable; shell tool stays blocked"
        );
        return None;
    }
    if status.degraded {
        tracing::warn!(details = %status.details, "sandbox driver is degraded");
    }
    Some(driver)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use slab_config::{AgentMcpConfig, AgentMcpEnvValueConfig, AgentMcpServerConfig};
    use slab_sandboxing::{SandboxDriver, SandboxError, SandboxSetupStatus, SandboxedCommand};

    use super::{agent_mcp_client_config_with_env, agent_trace_enabled, available_sandbox_driver};

    struct StatusDriver {
        status: SandboxSetupStatus,
    }

    #[async_trait]
    impl SandboxDriver for StatusDriver {
        async fn run(
            &self,
            _cmd: SandboxedCommand,
        ) -> Result<slab_sandboxing::SandboxedOutput, SandboxError> {
            unreachable!("status tests do not execute the sandbox driver")
        }

        fn name(&self) -> &str {
            "status"
        }

        fn setup_status(&self) -> SandboxSetupStatus {
            self.status.clone()
        }
    }

    #[test]
    fn agent_trace_gate_is_independent_of_telemetry_enabled() {
        // The critical independence assertion: `agent.debug` alone must enable the
        // trace directory/sink/bundle even when `telemetry.enabled` is OFF. If a
        // future change re-couples the gate to `agent.debug && telemetry.enabled`,
        // this case flips to false and the assertion fails.
        assert!(
            agent_trace_enabled(true, false),
            "agent.debug must enable the trace bundle even when telemetry.enabled is off"
        );
        assert!(agent_trace_enabled(true, true));
        // `telemetry.enabled` alone must NOT enable the agent trace bundle.
        assert!(
            !agent_trace_enabled(false, true),
            "telemetry.enabled must not enable the agent trace bundle"
        );
        assert!(!agent_trace_enabled(false, false));
    }

    #[test]
    fn unavailable_sandbox_driver_is_rejected() {
        let driver = Arc::new(StatusDriver {
            status: SandboxSetupStatus::unavailable("missing sandbox runtime"),
        });

        assert!(available_sandbox_driver(driver).is_none());
    }

    #[test]
    fn degraded_available_sandbox_driver_is_allowed() {
        let driver =
            Arc::new(StatusDriver { status: SandboxSetupStatus::degraded("guard-only mode") });

        assert!(available_sandbox_driver(driver).is_some());
    }

    #[test]
    fn agent_mcp_config_maps_enabled_servers_and_env_refs() {
        let mut env = BTreeMap::new();
        env.insert(
            "GITHUB_PERSONAL_ACCESS_TOKEN".to_owned(),
            AgentMcpEnvValueConfig { env_var: "GITHUB_TOKEN".to_owned() },
        );
        let settings = AgentMcpConfig {
            enabled: true,
            servers: vec![
                AgentMcpServerConfig {
                    enabled: true,
                    name: " github ".to_owned(),
                    command: " npx ".to_owned(),
                    args: vec!["-y".to_owned(), "@modelcontextprotocol/server-github".to_owned()],
                    cwd: Some(" C:/workspace ".to_owned()),
                    env,
                },
                AgentMcpServerConfig {
                    enabled: false,
                    name: "disabled".to_owned(),
                    command: "node".to_owned(),
                    args: Vec::new(),
                    cwd: None,
                    env: BTreeMap::new(),
                },
            ],
        };

        let config = agent_mcp_client_config_with_env(&settings, |name| match name {
            "GITHUB_TOKEN" => Ok("secret".to_owned()),
            _ => Err(std::env::VarError::NotPresent),
        });

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "github");
        assert_eq!(config.servers[0].command, "npx");
        assert_eq!(config.servers[0].cwd.as_deref(), Some("C:/workspace"));
        assert_eq!(config.servers[0].env["GITHUB_PERSONAL_ACCESS_TOKEN"], "secret");
    }

    #[test]
    fn agent_mcp_config_omits_missing_env_refs() {
        let mut env = BTreeMap::new();
        env.insert(
            "TOKEN".to_owned(),
            AgentMcpEnvValueConfig { env_var: "MISSING_TOKEN".to_owned() },
        );
        let settings = AgentMcpConfig {
            enabled: true,
            servers: vec![AgentMcpServerConfig {
                enabled: true,
                name: "server".to_owned(),
                command: "node".to_owned(),
                args: Vec::new(),
                cwd: None,
                env,
            }],
        };

        let config =
            agent_mcp_client_config_with_env(&settings, |_| Err(std::env::VarError::NotPresent));

        assert_eq!(config.servers.len(), 1);
        assert!(config.servers[0].env.is_empty());
    }
}
