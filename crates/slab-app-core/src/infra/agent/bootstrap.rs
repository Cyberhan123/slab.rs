use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slab_agent::{AgentControl, AgentThreadContext, ToolRouter, WorkspaceRef};
use slab_agent_tools::{ShellPolicy, ShellRuleSet};
use slab_agent_tracing::{AgentTraceSink, FileAgentTraceSink, NoopAgentTraceSink};
use slab_sandboxing::{SandboxEnvironment, SandboxPolicy, create_platform_driver};

use crate::context::AppContext;
use crate::domain::services::{AgentService, PluginService, WorkspaceLspService};
use crate::infra::db::AnyStore;

use super::event_hub::AgentEventHub;
use super::runtime::AgentRuntimeReloader;

pub(crate) struct AgentBootstrap {
    pub(crate) service: AgentService,
    pub(crate) runtime: AgentRuntimeReloader,
}

pub(crate) fn build_agent_bootstrap(ctx: &AppContext, store: Arc<AnyStore>) -> AgentBootstrap {
    let store_for_agent: Arc<dyn slab_agent::port::AgentStorePort> =
        Arc::clone(&store) as Arc<dyn slab_agent::port::AgentStorePort>;
    let event_hub = Arc::new(AgentEventHub::new());
    let control = build_agent_control(ctx, Arc::clone(&store), Arc::clone(&event_hub));
    let service = AgentService::new(control, store_for_agent, Arc::clone(&event_hub));
    let runtime = AgentRuntimeReloader::new((*ctx.model_state).clone(), service.control());
    schedule_agent_runtime_reload(runtime.clone());

    AgentBootstrap { service, runtime }
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
fn build_agent_control(
    ctx: &AppContext,
    store: Arc<AnyStore>,
    notify: Arc<AgentEventHub>,
) -> Arc<AgentControl> {
    let llm = Arc::new(super::adapter::ServerLlmAdapter::new(Arc::clone(&ctx.model_state)));
    let memory_store = Arc::clone(&store);
    let store_adapter: Arc<dyn slab_agent::port::AgentStorePort> = store;
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
    let mut shell_policy =
        if sandbox_driver.is_some() { ShellPolicy::Allow } else { ShellPolicy::Block };
    let shell_rules_dir = ctx.config.exec_rules_dir.clone();
    let shell_rules = match ShellRuleSet::from_dir(&shell_rules_dir) {
        Ok(rules) => rules,
        Err(error) => {
            tracing::warn!(
                rules_dir = %shell_rules_dir.display(),
                error = %error,
                "failed to load shell exec rules; shell tool will stay blocked"
            );
            shell_policy = ShellPolicy::Block;
            ShellRuleSet::default()
        }
    };

    let mut tool_router = ToolRouter::new();
    let web_search_config = ctx.pmid.config().agent.tools.websearch;
    let mcp_client = build_agent_mcp_client(ctx);
    slab_agent_tools::register_all_tools_with_shell_rules(
        &mut tool_router,
        shell_policy,
        sandbox_driver,
        workspace_root.clone(),
        mcp_client,
        false,
        web_search_config,
        shell_rules,
    );
    super::a2u_tools::register_builtin_a2u_tools(&tool_router);
    tool_router.register(Box::new(super::code_tools::CodeLspStatusTool::new(
        WorkspaceLspService::new(
            Arc::clone(&ctx.config),
            PluginService::new((*ctx.model_state).clone()),
        ),
    )));

    let tool_router = Arc::new(tool_router);
    let notify_port: Arc<dyn slab_agent::AgentNotifyPort> = notify.clone();
    let approval_port: Arc<dyn slab_agent::ApprovalPort> = notify;
    let settings = ctx.pmid.config();
    let (trace, trace_dir): (Arc<dyn AgentTraceSink>, Option<PathBuf>) =
        if settings.agent.debug && settings.telemetry.enabled {
            let dir = agent_trace_log_dir(ctx);
            (FileAgentTraceSink::shared(dir.clone()), Some(dir))
        } else {
            (Arc::new(NoopAgentTraceSink), None)
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
        Arc::clone(&ctx.model_state),
        memory_config.clone(),
        memory_root.clone(),
    );
    let mut hooks: Vec<Arc<dyn slab_agent::AgentHook>> = vec![
        Arc::new(slab_agent_memories::hooks::MemoryInstructionHook::new(
            memory_config.enabled,
            memory_root,
        )),
        Arc::new(super::memory::AgentMemoryStartupHook::new(memory_pipeline.clone())),
    ];
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
    let control = Arc::new(
        AgentControl::new_with_hooks_and_tracing(
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
        .with_thread_context(thread_context),
    );
    tool_router
        .register(Box::new(slab_agent_tools::DelegateSubagentTool::new(Arc::clone(&control))));
    memory_pipeline.set_control(Arc::clone(&control));
    control
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

    use super::{agent_mcp_client_config_with_env, available_sandbox_driver};

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
