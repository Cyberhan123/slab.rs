use std::collections::{HashMap, HashSet};
use std::path::Path;

use slab_types::plugin::{
    PluginAgentCapabilityContribution, PluginAgentHookContribution, PluginAgentHookRuntime,
    PluginCommandContribution, PluginLanguageServerContribution, PluginLanguageServerTransport,
    PluginManifest, PluginNetworkMode, PluginPermissionsManifest, PluginRouteContribution,
    PluginSettingsContribution, PluginSidebarContribution,
};
use slab_utils::hash::{sha256_hex_file, verify_sha256_hex_expected};

use super::SOURCE_KIND_DEV;

pub(super) fn validate_plugin_manifest(
    root_dir: &Path,
    manifest: &PluginManifest,
    source_kind: &str,
) -> Result<(), String> {
    if !is_valid_plugin_id(&manifest.id) {
        return Err(format!(
            "invalid plugin id `{}`: use lowercase letters, numbers, '-' or '_' and length 2..64",
            manifest.id
        ));
    }

    let folder_name = root_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid plugin directory path".to_owned())?;
    if folder_name != manifest.id {
        return Err(format!(
            "plugin folder `{folder_name}` does not match manifest id `{}`",
            manifest.id
        ));
    }

    validate_declared_file(
        root_dir,
        &manifest.integrity.files_sha256,
        &manifest.runtime.ui.entry,
        "runtime.ui.entry",
        source_kind,
    )?;
    if let Some(wasm) = &manifest.runtime.wasm {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            &wasm.entry,
            "runtime.wasm.entry",
            source_kind,
        )?;
    }
    if let Some(js) = &manifest.runtime.js {
        let entry = validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            &js.entry,
            "runtime.js.entry",
            source_kind,
        )?;
        validate_js_entry_extension(&entry)?;
    }
    if let Some(python) = &manifest.runtime.python {
        let entry = normalize_relative_path(&python.entry)?;
        validate_python_entry_extension(&entry)?;
        if let Some(bundle) = &python.bundle {
            let bundle = validate_declared_file(
                root_dir,
                &manifest.integrity.files_sha256,
                bundle,
                "runtime.python.bundle",
                source_kind,
            )?;
            validate_python_bundle_extension(&bundle)?;
        } else {
            validate_declared_file(
                root_dir,
                &manifest.integrity.files_sha256,
                &python.entry,
                "runtime.python.entry",
                source_kind,
            )?;
        }
    }

    if manifest.permissions.network.mode == PluginNetworkMode::Blocked
        && !manifest.permissions.network.allow_hosts.is_empty()
    {
        return Err("permissions.network.allowHosts must be empty when mode is blocked".to_owned());
    }

    validate_contributions(root_dir, manifest, source_kind)?;
    Ok(())
}

fn validate_contributions(
    root_dir: &Path,
    manifest: &PluginManifest,
    source_kind: &str,
) -> Result<(), String> {
    for rule in CONTRIBUTION_VALIDATION_RULES {
        let ids = (rule.extract_ids)(manifest);
        validate_duplicate_ids(rule.context, ids.iter())?;
        if (rule.has_items)(manifest) {
            (rule.ensure_permission)(&manifest.permissions)?;
        }
    }

    let route_ids =
        manifest.contributes.routes.iter().map(|route| route.id.clone()).collect::<HashSet<_>>();
    let path_prefix = format!("/plugins/{}", manifest.id);

    for route in &manifest.contributes.routes {
        validate_route(root_dir, route, manifest, &path_prefix, source_kind)?;
    }
    for command in &manifest.contributes.commands {
        validate_command(command, &route_ids)?;
    }
    for setting in &manifest.contributes.settings {
        validate_setting(root_dir, setting, manifest, source_kind)?;
    }
    for capability in &manifest.contributes.agent_capabilities {
        validate_agent_capability(root_dir, capability, manifest, source_kind)?;
    }
    for hook in &manifest.contributes.agent_hooks {
        validate_agent_hook(hook, manifest)?;
    }
    for provider in &manifest.contributes.language_servers {
        validate_language_server(provider)?;
    }
    for sidebar in &manifest.contributes.sidebar {
        validate_sidebar(sidebar, &route_ids)?;
    }

    Ok(())
}

type ContributionIdExtractor = fn(&PluginManifest) -> Vec<String>;
type ContributionPermissionChecker = fn(&PluginPermissionsManifest) -> Result<(), String>;
type ContributionHasItems = fn(&PluginManifest) -> bool;

struct ContributionValidationRule {
    context: &'static str,
    extract_ids: ContributionIdExtractor,
    has_items: ContributionHasItems,
    ensure_permission: ContributionPermissionChecker,
}

const CONTRIBUTION_VALIDATION_RULES: &[ContributionValidationRule] = &[
    ContributionValidationRule {
        context: "contributes.routes",
        extract_ids: extract_route_ids,
        has_items: has_routes,
        ensure_permission: ensure_routes_permission,
    },
    ContributionValidationRule {
        context: "contributes.sidebar",
        extract_ids: extract_sidebar_ids,
        has_items: has_sidebar,
        ensure_permission: ensure_sidebar_permission,
    },
    ContributionValidationRule {
        context: "contributes.commands",
        extract_ids: extract_command_ids,
        has_items: has_commands,
        ensure_permission: ensure_commands_permission,
    },
    ContributionValidationRule {
        context: "contributes.settings",
        extract_ids: extract_setting_ids,
        has_items: has_settings,
        ensure_permission: ensure_settings_permission,
    },
    ContributionValidationRule {
        context: "contributes.agentCapabilities",
        extract_ids: extract_agent_capability_ids,
        has_items: has_agent_capabilities,
        ensure_permission: ensure_agent_capabilities_permission,
    },
    ContributionValidationRule {
        context: "contributes.agentHooks",
        extract_ids: extract_agent_hook_ids,
        has_items: has_agent_hooks,
        ensure_permission: ensure_agent_hooks_permission,
    },
    ContributionValidationRule {
        context: "contributes.languageServers",
        extract_ids: extract_language_server_ids,
        has_items: has_language_servers,
        ensure_permission: ensure_language_servers_permission,
    },
];

fn extract_route_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.routes.iter().map(|route| route.id.clone()).collect()
}

fn extract_sidebar_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.sidebar.iter().map(|item| item.id.clone()).collect()
}

fn extract_command_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.commands.iter().map(|item| item.id.clone()).collect()
}

fn extract_setting_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.settings.iter().map(|item| item.id.clone()).collect()
}

fn extract_agent_capability_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.agent_capabilities.iter().map(|item| item.id.clone()).collect()
}

fn extract_agent_hook_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.agent_hooks.iter().map(|item| item.id.clone()).collect()
}

fn extract_language_server_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest.contributes.language_servers.iter().map(|item| item.id.clone()).collect()
}

fn has_routes(manifest: &PluginManifest) -> bool {
    !manifest.contributes.routes.is_empty()
}

fn has_sidebar(manifest: &PluginManifest) -> bool {
    !manifest.contributes.sidebar.is_empty()
}

fn has_commands(manifest: &PluginManifest) -> bool {
    !manifest.contributes.commands.is_empty()
}

fn has_settings(manifest: &PluginManifest) -> bool {
    !manifest.contributes.settings.is_empty()
}

fn has_agent_capabilities(manifest: &PluginManifest) -> bool {
    !manifest.contributes.agent_capabilities.is_empty()
}

fn has_agent_hooks(manifest: &PluginManifest) -> bool {
    !manifest.contributes.agent_hooks.is_empty()
}

fn has_language_servers(manifest: &PluginManifest) -> bool {
    !manifest.contributes.language_servers.is_empty()
}

fn ensure_routes_permission(permissions: &PluginPermissionsManifest) -> Result<(), String> {
    ensure_permission(
        permissions,
        "route:create",
        "contributes.routes requires permissions.ui to include route:create",
    )
}

fn ensure_sidebar_permission(permissions: &PluginPermissionsManifest) -> Result<(), String> {
    ensure_permission(
        permissions,
        "sidebar:item:create",
        "contributes.sidebar requires permissions.ui to include sidebar:item:create",
    )
}

fn ensure_commands_permission(permissions: &PluginPermissionsManifest) -> Result<(), String> {
    ensure_permission(
        permissions,
        "command:create",
        "contributes.commands requires permissions.ui to include command:create",
    )
}

fn ensure_settings_permission(permissions: &PluginPermissionsManifest) -> Result<(), String> {
    ensure_permission(
        permissions,
        "settings:section:create",
        "contributes.settings requires permissions.ui to include settings:section:create",
    )
}

fn ensure_agent_capabilities_permission(
    permissions: &PluginPermissionsManifest,
) -> Result<(), String> {
    ensure_agent_permission(
        permissions,
        "capability:declare",
        "contributes.agentCapabilities requires permissions.agent to include capability:declare",
    )
}

fn ensure_agent_hooks_permission(permissions: &PluginPermissionsManifest) -> Result<(), String> {
    ensure_agent_permission(
        permissions,
        "hook:declare",
        "contributes.agentHooks requires permissions.agent to include hook:declare",
    )
}

fn ensure_language_servers_permission(
    permissions: &PluginPermissionsManifest,
) -> Result<(), String> {
    ensure_lsp_permission(
        permissions,
        "languageServer:declare",
        "contributes.languageServers requires permissions.lsp to include languageServer:declare",
    )
}

fn validate_route(
    root_dir: &Path,
    route: &PluginRouteContribution,
    manifest: &PluginManifest,
    path_prefix: &str,
    source_kind: &str,
) -> Result<(), String> {
    if !(route.path == *path_prefix || route.path.starts_with(&format!("{path_prefix}/"))) {
        return Err(format!("route `{}` must use a path inside `{path_prefix}`", route.id));
    }
    if let Some(entry) = &route.entry {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            entry,
            "contributes.routes[].entry",
            source_kind,
        )?;
    }
    Ok(())
}

fn validate_command(
    command: &PluginCommandContribution,
    route_ids: &HashSet<String>,
) -> Result<(), String> {
    if command.action.as_deref() == Some("openRoute") {
        let route = command.route.as_deref().ok_or_else(|| {
            format!("command `{}` with action `openRoute` must declare route", command.id)
        })?;
        if !route_ids.contains(route) {
            return Err(format!("command `{}` references unknown route `{route}`", command.id));
        }
    }
    Ok(())
}

fn validate_sidebar(
    sidebar: &PluginSidebarContribution,
    route_ids: &HashSet<String>,
) -> Result<(), String> {
    if let Some(route) = sidebar.route.as_deref()
        && !route_ids.contains(route)
    {
        return Err(format!(
            "sidebar contribution `{}` references unknown route `{route}`",
            sidebar.id
        ));
    }
    Ok(())
}

fn validate_setting(
    root_dir: &Path,
    setting: &PluginSettingsContribution,
    manifest: &PluginManifest,
    source_kind: &str,
) -> Result<(), String> {
    validate_declared_file(
        root_dir,
        &manifest.integrity.files_sha256,
        &setting.schema,
        "contributes.settings[].schema",
        source_kind,
    )?;
    Ok(())
}

fn validate_agent_capability(
    root_dir: &Path,
    capability: &PluginAgentCapabilityContribution,
    manifest: &PluginManifest,
    source_kind: &str,
) -> Result<(), String> {
    if let Some(input_schema) = &capability.input_schema {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            input_schema,
            "contributes.agentCapabilities[].inputSchema",
            source_kind,
        )?;
    }
    if let Some(output_schema) = &capability.output_schema {
        validate_declared_file(
            root_dir,
            &manifest.integrity.files_sha256,
            output_schema,
            "contributes.agentCapabilities[].outputSchema",
            source_kind,
        )?;
    }
    if capability.expose_as_mcp_tool {
        ensure_agent_permission(
            &manifest.permissions,
            "mcpTool:expose",
            "contributes.agentCapabilities[].exposeAsMcpTool requires permissions.agent to include mcpTool:expose",
        )?;
    }
    Ok(())
}

fn validate_agent_hook(
    hook: &PluginAgentHookContribution,
    manifest: &PluginManifest,
) -> Result<(), String> {
    if hook.id.trim().is_empty() {
        return Err("contributes.agentHooks[].id must not be empty".to_owned());
    }
    if hook.events.is_empty() {
        return Err(format!("agent hook `{}` must declare events", hook.id));
    }
    if hook.transport.function.trim().is_empty() {
        return Err(format!("agent hook `{}` must declare transport.function", hook.id));
    }
    match hook.transport.runtime {
        PluginAgentHookRuntime::JavaScript => {
            if manifest.runtime.js.is_none() {
                return Err(format!("agent hook `{}` requires runtime.js", hook.id));
            }
        }
        PluginAgentHookRuntime::Python => {
            if manifest.runtime.python.is_none() {
                return Err(format!("agent hook `{}` requires runtime.python", hook.id));
            }
        }
    }
    Ok(())
}

fn validate_language_server(provider: &PluginLanguageServerContribution) -> Result<(), String> {
    if provider.languages.is_empty() {
        return Err(format!("language server `{}` must declare languages", provider.id));
    }
    for language in &provider.languages {
        if !is_valid_language_id(language) {
            return Err(format!(
                "language server `{}` has invalid language `{language}`",
                provider.id
            ));
        }
    }

    match &provider.transport {
        PluginLanguageServerTransport::Stdio { command, .. } => {
            if command.trim().is_empty() {
                return Err(format!(
                    "language server `{}` must declare transport.command",
                    provider.id
                ));
            }
        }
        PluginLanguageServerTransport::NodePackage { package, .. } => {
            if package.trim().is_empty() {
                return Err(format!(
                    "language server `{}` must declare transport.package",
                    provider.id
                ));
            }
        }
        PluginLanguageServerTransport::WebSocket { url } => {
            if !(url.starts_with("ws://") || url.starts_with("wss://")) {
                return Err(format!(
                    "language server `{}` websocket url must start with ws:// or wss://",
                    provider.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_duplicate_ids<'a>(
    context: &str,
    ids: impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            return Err(format!("duplicated contribution id `{id}` in {context}"));
        }
    }
    Ok(())
}

fn ensure_lsp_permission(
    permissions: &PluginPermissionsManifest,
    expected: &str,
    error: &str,
) -> Result<(), String> {
    if permissions.lsp.iter().any(|value| value == expected) {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn ensure_permission(
    permissions: &PluginPermissionsManifest,
    expected: &str,
    error: &str,
) -> Result<(), String> {
    if permissions.ui.iter().any(|value| value == expected) {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn ensure_agent_permission(
    permissions: &PluginPermissionsManifest,
    expected: &str,
    error: &str,
) -> Result<(), String> {
    if permissions.agent.iter().any(|value| value == expected) {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn validate_declared_file(
    root_dir: &Path,
    files_sha256: &HashMap<String, String>,
    raw_path: &str,
    context: &str,
    source_kind: &str,
) -> Result<String, String> {
    let normalized_path = normalize_relative_path(raw_path)?;
    let file_path = root_dir.join(&normalized_path);
    if !file_path.is_file() {
        return Err(format!("{context} `{normalized_path}` does not exist on disk"));
    }
    if source_kind != SOURCE_KIND_DEV {
        let expected_hash = files_sha256.get(&normalized_path).ok_or_else(|| {
            format!("{context} `{normalized_path}` is missing from integrity.filesSha256")
        })?;
        let actual_hash = sha256_hex_file(&file_path)
            .map_err(|error| format!("failed to hash `{normalized_path}`: {error}"))?;
        if verify_sha256_hex_expected(&actual_hash, expected_hash).is_err() {
            return Err(format!(
                "integrity.filesSha256 mismatch for `{normalized_path}`: expected {expected_hash}, got {actual_hash}"
            ));
        }
    }
    Ok(normalized_path)
}

fn validate_js_entry_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "ts" | "tsx" | "js" | "mjs") {
        return Ok(());
    }
    Err("runtime.js.entry must use .ts, .tsx, .js, or .mjs".to_owned())
}

fn validate_python_entry_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "py" {
        return Ok(());
    }
    Err("runtime.python.entry must use .py".to_owned())
}

fn validate_python_bundle_extension(entry: &str) -> Result<(), String> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "slabpy" {
        return Ok(());
    }
    Err("runtime.python.bundle must use .slabpy".to_owned())
}

pub(super) fn normalize_relative_path(raw: &str) -> Result<String, String> {
    slab_utils::path::normalize_relative_path(raw).map_err(|error| error.to_string())
}

fn is_valid_plugin_id(id: &str) -> bool {
    if !(2..=64).contains(&id.len()) {
        return false;
    }
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'
            || character == '_'
    })
}

fn is_valid_language_id(id: &str) -> bool {
    if !(1..=64).contains(&id.len()) {
        return false;
    }
    id.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'
            || character == '_'
    })
}
