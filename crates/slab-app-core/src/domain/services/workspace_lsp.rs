use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::context::AppConfig;
use crate::domain::services::PluginService;
use crate::error::AppCoreError;
use crate::infra::process_supervisor::resolve_sibling_sidecar_exe;
use slab_types::plugin::{PluginLanguageServerContribution, PluginLanguageServerTransport};

const SLAB_JS_RUNTIME_COMMAND: &str = "slab-js-runtime";
const BUILTIN_WEB_LANGUAGE_SERVER_PROVIDERS: &[(&str, &[&str], &str)] = &[
    (
        "builtin.typescript",
        &["typescript", "javascript", "typescriptreact", "javascriptreact"],
        "typescript",
    ),
    ("builtin.json", &["json", "jsonc"], "json"),
    ("builtin.css", &["css", "less", "scss"], "css"),
    ("builtin.html", &["html"], "html"),
];
const BUILTIN_NATIVE_LANGUAGE_SERVER_PROVIDERS: &[(&str, &[&str], &str, &[&str])] = &[
    ("builtin.pyright", &["python"], "pyright-langserver", &["--stdio"]),
    ("builtin.clangd", &["c", "cpp"], "clangd", &[]),
    ("builtin.gopls", &["go"], "gopls", &[]),
    ("builtin.rust-analyzer", &["rust"], "rust-analyzer", &[]),
];

#[derive(Clone)]
pub struct WorkspaceLspService {
    config: Arc<AppConfig>,
    plugin: PluginService,
}

pub struct WorkspaceLspProvider {
    pub contribution: PluginLanguageServerContribution,
    pub install_root: Option<PathBuf>,
}

impl WorkspaceLspService {
    pub fn new(config: Arc<AppConfig>, plugin: PluginService) -> Self {
        Self { config, plugin }
    }

    pub fn workspace_root(&self) -> Result<PathBuf, AppCoreError> {
        workspace_root_from_config(&self.config).ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "settings path {} is not inside a workspace `.slab` directory and workspace_root is not set",
                self.config.settings_path.display()
            ))
        })
    }

    pub async fn resolve_provider(
        &self,
        language_id: &str,
    ) -> Result<Option<WorkspaceLspProvider>, AppCoreError> {
        let language_id = normalize_language_id(language_id);
        if language_id.is_empty() {
            return Ok(None);
        }

        let plugins = self.plugin.list_plugins().await?;
        for plugin in plugins.into_iter().filter(|plugin| plugin.valid && plugin.enabled) {
            let Some(contributes) = plugin.contributions else {
                continue;
            };

            if let Some(provider) = contributes
                .language_servers
                .into_iter()
                .find(|provider| provider_matches_language(provider, &language_id))
            {
                return Ok(Some(WorkspaceLspProvider {
                    contribution: provider,
                    install_root: plugin
                        .install_root
                        .filter(|root| !root.trim().is_empty())
                        .map(PathBuf::from),
                }));
            }
        }

        Ok(builtin_language_server_provider(&language_id, &self.config)
            .map(|contribution| WorkspaceLspProvider { contribution, install_root: None }))
    }

    pub async fn spawn_stdio_process(
        &self,
        provider: &WorkspaceLspProvider,
        workspace_root: &Path,
    ) -> Result<WorkspaceLspProcess, AppCoreError> {
        let (command, args, env) = match &provider.contribution.transport {
            PluginLanguageServerTransport::Stdio { command, args, env } => {
                (command.as_str(), args.as_slice(), env)
            }
            PluginLanguageServerTransport::NodePackage { package, command, args, env } => {
                let cmd = command.as_deref().unwrap_or(package.as_str());
                (cmd, args.as_slice(), env)
            }
            _ => {
                return Err(AppCoreError::BadRequest(format!(
                    "language server '{}' does not use stdio transport",
                    provider.contribution.id
                )));
            }
        };

        if command.trim().is_empty() {
            return Err(AppCoreError::BadRequest(format!(
                "language server '{}' has an empty stdio command",
                provider.contribution.id
            )));
        }

        let search_dirs = language_server_search_dirs(
            workspace_root,
            &self.config,
            provider.install_root.as_deref(),
        );
        let resolution = resolve_language_server_command(
            command,
            workspace_root,
            &self.config,
            provider.install_root.as_deref(),
        );
        let mut process = Command::new(&resolution.command);
        process.args(args);
        process.current_dir(workspace_root);
        process.stdin(Stdio::piped());
        process.stdout(Stdio::piped());
        process.stderr(Stdio::inherit());
        process.kill_on_drop(true);
        if let Some(path) = language_server_path_env(&search_dirs) {
            process.env("PATH", path);
        }
        for (key, value) in env {
            process.env(key, value);
        }

        let mut child = process.spawn().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to start language server '{}' using `{}` resolved as `{}` from workspace `{}`; searched locations: {}; {error}",
                provider.contribution.id,
                command,
                resolution.command.to_string_lossy(),
                workspace_root.display(),
                describe_searched_locations(&resolution.searched_locations)
            ))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AppCoreError::Internal(format!(
                "language server '{}' did not expose stdin",
                provider.contribution.id
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppCoreError::Internal(format!(
                "language server '{}' did not expose stdout",
                provider.contribution.id
            ))
        })?;

        Ok(WorkspaceLspProcess { child, stdin, stdout })
    }
}

pub struct WorkspaceLspProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl WorkspaceLspProcess {
    pub fn io_mut(&mut self) -> (&mut ChildStdin, &mut ChildStdout) {
        (&mut self.stdin, &mut self.stdout)
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

pub fn workspace_root_from_settings_path(settings_path: &Path) -> Option<PathBuf> {
    let slab_dir = settings_path.parent()?;
    if slab_dir.file_name()?.to_str()? != ".slab" {
        return None;
    }
    slab_dir.parent().map(Path::to_path_buf)
}

pub fn workspace_root_from_config(config: &AppConfig) -> Option<PathBuf> {
    config
        .workspace_root
        .clone()
        .or_else(|| workspace_root_from_settings_path(&config.settings_path))
}

fn normalize_language_id(language_id: &str) -> String {
    language_id.trim().to_lowercase()
}

fn provider_matches_language(
    provider: &PluginLanguageServerContribution,
    language_id: &str,
) -> bool {
    provider.languages.iter().any(|language| normalize_language_id(language) == language_id)
}

fn builtin_language_server_provider(
    language_id: &str,
    config: &AppConfig,
) -> Option<PluginLanguageServerContribution> {
    for (id, languages, bundle) in BUILTIN_WEB_LANGUAGE_SERVER_PROVIDERS {
        if !languages.contains(&language_id) {
            continue;
        }

        return Some(PluginLanguageServerContribution {
            id: (*id).to_owned(),
            languages: languages.iter().map(|language| (*language).to_owned()).collect(),
            transport: PluginLanguageServerTransport::Stdio {
                command: SLAB_JS_RUNTIME_COMMAND.to_owned(),
                args: builtin_web_language_server_args(config, bundle),
                env: HashMap::new(),
            },
        });
    }

    for (id, languages, command, args) in BUILTIN_NATIVE_LANGUAGE_SERVER_PROVIDERS {
        if !languages.contains(&language_id) {
            continue;
        }

        return Some(PluginLanguageServerContribution {
            id: (*id).to_owned(),
            languages: languages.iter().map(|language| (*language).to_owned()).collect(),
            transport: PluginLanguageServerTransport::Stdio {
                command: (*command).to_owned(),
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                env: HashMap::new(),
            },
        });
    }

    None
}

fn builtin_web_language_server_args(config: &AppConfig, bundle: &str) -> Vec<String> {
    vec![
        "lsp".to_owned(),
        "--entry".to_owned(),
        builtin_web_language_server_entry(config, bundle).to_string_lossy().into_owned(),
        "--".to_owned(),
        "--stdio".to_owned(),
    ]
}

fn builtin_web_language_server_entry(config: &AppConfig, bundle: &str) -> PathBuf {
    let relative = Path::new("language-servers").join("web").join(format!("{bundle}.mjs"));
    config.lib_dir.as_ref().map_or(relative.clone(), |lib_dir| lib_dir.join(relative))
}

struct LanguageServerCommandResolution {
    command: OsString,
    searched_locations: Vec<PathBuf>,
}

fn resolve_language_server_command(
    command: &str,
    workspace_root: &Path,
    config: &AppConfig,
    provider_root: Option<&Path>,
) -> LanguageServerCommandResolution {
    let searched_locations = language_server_command_candidates(
        command,
        workspace_root,
        &language_server_search_dirs(workspace_root, config, provider_root),
        provider_root,
    );

    let command = resolve_sibling_language_server_sidecar(command)
        .map(|path| path.into_os_string())
        .or_else(|| {
            searched_locations
                .iter()
                .find(|candidate| candidate.is_file())
                .map(|candidate| candidate.as_os_str().to_owned())
        })
        .unwrap_or_else(|| OsString::from(command));

    LanguageServerCommandResolution { command, searched_locations }
}

fn resolve_sibling_language_server_sidecar(command: &str) -> Option<PathBuf> {
    if command != SLAB_JS_RUNTIME_COMMAND {
        return None;
    }

    std::env::current_exe()
        .ok()
        .and_then(|server_exe| resolve_sibling_sidecar_exe(&server_exe, command).ok())
}

fn language_server_command_candidates(
    command: &str,
    workspace_root: &Path,
    search_dirs: &[PathBuf],
    provider_root: Option<&Path>,
) -> Vec<PathBuf> {
    if command_has_path_separator(command) {
        let command_path = PathBuf::from(command);
        let mut candidates = Vec::new();
        for variant in command_path_variants(&command_path) {
            if variant.is_absolute() {
                candidates.push(variant);
            } else {
                for root in relative_command_roots(workspace_root, provider_root) {
                    candidates.push(root.join(&variant));
                }
            }
        }
        return candidates;
    }

    let mut candidates = Vec::new();
    for dir in search_dirs {
        for variant in command_path_variants(Path::new(command)) {
            candidates.push(dir.join(variant));
        }
    }
    candidates
}

fn command_path_variants(command: &Path) -> Vec<PathBuf> {
    if !cfg!(windows) || command.extension().is_some() {
        return vec![command.to_path_buf()];
    }

    let command = command.to_string_lossy();
    [
        command.to_string(),
        format!("{command}.exe"),
        format!("{command}.cmd"),
        format!("{command}.bat"),
        format!("{command}.ps1"),
        format!("{command}.bunx"),
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn command_has_path_separator(command: &str) -> bool {
    command.contains('/') || command.contains('\\')
}

fn relative_command_roots(workspace_root: &Path, provider_root: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Some(provider_root) = provider_root {
        push_unique_dir(&mut roots, &mut seen, provider_root.to_path_buf());
    }
    push_unique_dir(&mut roots, &mut seen, workspace_root.to_path_buf());
    if let Ok(current_dir) = std::env::current_dir() {
        push_unique_dir(&mut roots, &mut seen, current_dir);
    }

    roots
}

fn language_server_search_dirs(
    workspace_root: &Path,
    _config: &AppConfig,
    provider_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    push_node_bin_ancestors(&mut dirs, &mut seen, workspace_root);

    if let Some(provider_root) = provider_root {
        push_unique_dir(&mut dirs, &mut seen, provider_root.join("node_modules").join(".bin"));
        push_unique_dir(&mut dirs, &mut seen, provider_root.join("bin"));
        push_unique_dir(&mut dirs, &mut seen, provider_root.to_path_buf());
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_node_bin_ancestors(&mut dirs, &mut seen, &current_dir);
    }

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        push_unique_dir(&mut dirs, &mut seen, exe_dir.to_path_buf());
    }

    dirs
}

fn push_node_bin_ancestors(dirs: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: &Path) {
    for ancestor in path.ancestors() {
        push_unique_dir(dirs, seen, ancestor.join("node_modules").join(".bin"));
    }
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if seen.insert(path.clone()) {
        dirs.push(path);
    }
}

fn language_server_path_env(search_dirs: &[PathBuf]) -> Option<OsString> {
    let mut paths: Vec<PathBuf> =
        search_dirs.iter().filter(|path| path.is_dir()).cloned().collect();
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }

    std::env::join_paths(paths).ok()
}

fn describe_searched_locations(locations: &[PathBuf]) -> String {
    if locations.is_empty() {
        return "PATH only".to_owned();
    }

    locations.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_language_server_provider, language_server_command_candidates,
        resolve_language_server_command, workspace_root_from_config,
        workspace_root_from_settings_path,
    };
    use crate::config::Config;
    use slab_config::{PluginJsRuntimeTransport, PluginPythonRuntimeTransport};
    use slab_types::plugin::PluginLanguageServerTransport;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn test_config(settings_path: PathBuf) -> Config {
        let root = settings_path.parent().expect("settings parent").to_path_buf();

        Config {
            bind_address: "127.0.0.1:0".to_owned(),
            database_url: "sqlite::memory:".to_owned(),
            log_level: "info".to_owned(),
            log_json: false,
            log_file: None,
            cloud_http_trace: false,
            queue_capacity: 1,
            backend_capacity: 1,
            enable_swagger: false,
            cors_allowed_origins: None,
            admin_api_token: None,
            transport_mode: "http".to_owned(),
            llama_grpc_endpoint: None,
            whisper_grpc_endpoint: None,
            diffusion_grpc_endpoint: None,
            candle_llama_grpc_endpoint: None,
            candle_whisper_grpc_endpoint: None,
            candle_diffusion_grpc_endpoint: None,
            lib_dir: Some(root.join("resources").join("libs")),
            session_state_dir: root.join("sessions").to_string_lossy().into_owned(),
            settings_path,
            settings_overlay_path: None,
            workspace_root: None,
            model_config_dir: root.join("models"),
            plugins_dir: root.join("plugins"),
            exec_rules_dir: root.join("rules"),
            plugin_js_runtime_transport: PluginJsRuntimeTransport::Stdio,
            plugin_python_runtime_transport: PluginPythonRuntimeTransport::Stdio,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("slab-workspace-lsp-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn command_file_name(command: &str) -> String {
        if cfg!(windows) { format!("{command}.exe") } else { command.to_owned() }
    }

    fn write_file(path: &Path) {
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(path, "").expect("file");
    }

    #[test]
    fn resolves_workspace_root_from_slab_settings_path() {
        #[cfg(windows)]
        let (settings_path, expected_root) = (
            std::path::Path::new(r"C:\Users\cyberhan\Desktop\demo\.slab\settings.json"),
            std::path::PathBuf::from(r"C:\Users\cyberhan\Desktop\demo"),
        );
        #[cfg(not(windows))]
        let (settings_path, expected_root) = (
            std::path::Path::new("/Users/cyberhan/Desktop/demo/.slab/settings.json"),
            std::path::PathBuf::from("/Users/cyberhan/Desktop/demo"),
        );

        let root = workspace_root_from_settings_path(settings_path).expect("workspace root");

        assert_eq!(root, expected_root);
    }

    #[test]
    fn rejects_non_workspace_settings_path() {
        #[cfg(windows)]
        let settings_path = std::path::Path::new(r"C:\Users\cyberhan\Desktop\demo\settings.json");
        #[cfg(not(windows))]
        let settings_path = std::path::Path::new("/Users/cyberhan/Desktop/demo/settings.json");

        assert!(workspace_root_from_settings_path(settings_path).is_none());
    }

    #[test]
    fn explicit_workspace_root_overrides_settings_path_in_config() {
        let mut config = test_config(PathBuf::from(
            "C:/Users/example/AppData/Roaming/cn.cyberhan.slab/settings.json",
        ));
        config.workspace_root = Some(PathBuf::from("D:/Workspace"));

        assert_eq!(workspace_root_from_config(&config), Some(PathBuf::from("D:/Workspace")));
    }

    #[test]
    fn builtin_provider_matches_supported_language() {
        let root = temp_dir("builtin-provider");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);

        let provider = builtin_language_server_provider("typescript", &config).expect("provider");

        assert_eq!(provider.id, "builtin.typescript");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builtin_web_provider_runs_through_slab_js_runtime_lsp_mode() {
        let root = temp_dir("builtin-web-provider");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);

        let provider = builtin_language_server_provider("json", &config).expect("provider");

        let PluginLanguageServerTransport::Stdio { command, args, .. } = provider.transport else {
            panic!("expected stdio transport");
        };
        assert_eq!(command, "slab-js-runtime");
        assert_eq!(args[0], "lsp");
        assert_eq!(args[1], "--entry");
        assert!(
            Path::new(&args[2])
                .ends_with(Path::new("language-servers").join("web").join("json.mjs"))
        );
        assert_eq!(args[3], "--");
        assert_eq!(args[4], "--stdio");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_workspace_node_bin_language_server_before_path_fallback() {
        let root = temp_dir("workspace-node-bin");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let binary = root
            .join("node_modules")
            .join(".bin")
            .join(command_file_name("typescript-language-server"));
        write_file(&binary);

        let resolution =
            resolve_language_server_command("typescript-language-server", &root, &config, None);

        assert_eq!(PathBuf::from(resolution.command), binary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_packaged_native_language_server_payloads() {
        let root = temp_dir("lib-dir");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let binary = root
            .join(".slab")
            .join("resources")
            .join("libs")
            .join("language-servers")
            .join("bin")
            .join(command_file_name("rust-analyzer"));
        write_file(&binary);

        let resolution = resolve_language_server_command("rust-analyzer", &root, &config, None);

        assert_eq!(resolution.command, "rust-analyzer");
        assert!(!resolution.searched_locations.contains(&binary));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_packaged_language_server_node_modules_bin() {
        let root = temp_dir("lib-dir-node-modules");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let binary = root
            .join(".slab")
            .join("resources")
            .join("libs")
            .join("language-servers")
            .join("node_modules")
            .join(".bin")
            .join(command_file_name("typescript-language-server"));
        write_file(&binary);

        let resolution =
            resolve_language_server_command("typescript-language-server", &root, &config, None);

        assert_eq!(resolution.command, "typescript-language-server");
        assert!(!resolution.searched_locations.contains(&binary));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_plugin_node_bin_language_server() {
        let root = temp_dir("plugin-node-bin");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let plugin_root = root.join("plugins").join("example-lsp");
        let binary = plugin_root
            .join("node_modules")
            .join(".bin")
            .join(command_file_name("example-language-server"));
        write_file(&binary);

        let resolution = resolve_language_server_command(
            "example-language-server",
            &root,
            &config,
            Some(&plugin_root),
        );

        assert_eq!(PathBuf::from(resolution.command), binary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_relative_plugin_command_before_workspace() {
        let root = temp_dir("plugin-relative-command");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let plugin_root = root.join("plugins").join("example-lsp");
        let plugin_binary = plugin_root.join("bin").join("language-server.cmd");
        let workspace_binary = root.join("bin").join("language-server.cmd");
        write_file(&plugin_binary);
        write_file(&workspace_binary);

        let resolution = resolve_language_server_command(
            "bin/language-server.cmd",
            &root,
            &config,
            Some(&plugin_root),
        );

        assert_eq!(PathBuf::from(resolution.command), plugin_binary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_relative_command_inside_workspace() {
        let root = temp_dir("relative-command");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);
        let binary = root.join("tools").join("language-server.cmd");
        write_file(&binary);

        let resolution =
            resolve_language_server_command("tools/language-server.cmd", &root, &config, None);

        assert_eq!(PathBuf::from(resolution.command), binary);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unresolved_command_keeps_path_fallback_and_records_candidates() {
        let root = temp_dir("missing-command");
        let settings_path = root.join(".slab").join("settings.json");
        let config = test_config(settings_path);

        let resolution = resolve_language_server_command("missing-lsp", &root, &config, None);

        assert_eq!(resolution.command, "missing-lsp");
        assert!(resolution.searched_locations.iter().any(|path| path.ends_with(
            Path::new("node_modules").join(".bin").join(command_file_name("missing-lsp"))
        )));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn relative_command_candidates_start_in_workspace() {
        let root = PathBuf::from(r"C:\workspace");
        let candidates = language_server_command_candidates("tools/server.cmd", &root, &[], None);

        assert_eq!(candidates[0], root.join("tools").join("server.cmd"));
    }

    #[test]
    fn relative_command_candidates_start_in_provider_root_when_available() {
        let root = PathBuf::from(r"C:\workspace");
        let provider_root = PathBuf::from(r"C:\plugins\example");
        let candidates = language_server_command_candidates(
            "tools/server.cmd",
            &root,
            &[],
            Some(&provider_root),
        );

        assert_eq!(candidates[0], provider_root.join("tools").join("server.cmd"));
    }
}
