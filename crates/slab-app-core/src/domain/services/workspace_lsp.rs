use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::context::AppConfig;
use crate::domain::services::PluginService;
use crate::error::AppCoreError;
use slab_types::plugin::{
    PluginContributesManifest, PluginLanguageServerContribution, PluginLanguageServerTransport,
};

const BUILTIN_LANGUAGE_SERVER_PROVIDERS: &[(&str, &[&str], &str, &[&str])] = &[
    (
        "builtin.typescript",
        &["typescript", "javascript", "typescriptreact", "javascriptreact"],
        "typescript-language-server",
        &["--stdio"],
    ),
    ("builtin.json", &["json", "jsonc"], "vscode-json-language-server", &["--stdio"]),
    ("builtin.css", &["css", "less", "scss"], "vscode-css-language-server", &["--stdio"]),
    ("builtin.html", &["html"], "vscode-html-language-server", &["--stdio"]),
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

impl WorkspaceLspService {
    pub fn new(config: Arc<AppConfig>, plugin: PluginService) -> Self {
        Self { config, plugin }
    }

    pub fn workspace_root(&self) -> Result<PathBuf, AppCoreError> {
        workspace_root_from_settings_path(&self.config.settings_path).ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "settings path {} is not inside a workspace `.slab` directory",
                self.config.settings_path.display()
            ))
        })
    }

    pub async fn resolve_provider(
        &self,
        language_id: &str,
    ) -> Result<Option<PluginLanguageServerContribution>, AppCoreError> {
        let language_id = normalize_language_id(language_id);
        if language_id.is_empty() {
            return Ok(None);
        }

        let plugins = self.plugin.list_plugins().await?;
        for plugin in plugins.into_iter().filter(|plugin| plugin.valid && plugin.enabled) {
            let Ok(contributes) =
                serde_json::from_value::<PluginContributesManifest>(plugin.contributions.clone())
            else {
                continue;
            };

            if let Some(provider) = contributes
                .language_servers
                .into_iter()
                .find(|provider| provider_matches_language(provider, &language_id))
            {
                return Ok(Some(provider));
            }
        }

        Ok(builtin_language_server_provider(&language_id))
    }

    pub async fn spawn_stdio_process(
        &self,
        provider: &PluginLanguageServerContribution,
        workspace_root: &Path,
    ) -> Result<WorkspaceLspProcess, AppCoreError> {
        let PluginLanguageServerTransport::Stdio { command, args, env } = &provider.transport
        else {
            return Err(AppCoreError::BadRequest(format!(
                "language server '{}' does not use stdio transport",
                provider.id
            )));
        };

        let mut process = Command::new(command);
        process.args(args);
        process.current_dir(workspace_root);
        process.stdin(Stdio::piped());
        process.stdout(Stdio::piped());
        process.stderr(Stdio::inherit());
        process.kill_on_drop(true);
        for (key, value) in env {
            process.env(key, value);
        }

        let mut child = process.spawn().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to start language server '{}' using `{}`: {error}",
                provider.id, command
            ))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AppCoreError::Internal(format!(
                "language server '{}' did not expose stdin",
                provider.id
            ))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppCoreError::Internal(format!(
                "language server '{}' did not expose stdout",
                provider.id
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

fn normalize_language_id(language_id: &str) -> String {
    language_id.trim().to_lowercase()
}

fn provider_matches_language(
    provider: &PluginLanguageServerContribution,
    language_id: &str,
) -> bool {
    provider.languages.iter().any(|language| normalize_language_id(language) == language_id)
}

fn builtin_language_server_provider(language_id: &str) -> Option<PluginLanguageServerContribution> {
    for (id, languages, command, args) in BUILTIN_LANGUAGE_SERVER_PROVIDERS {
        if !languages.iter().any(|language| *language == language_id) {
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

#[cfg(test)]
mod tests {
    use super::{builtin_language_server_provider, workspace_root_from_settings_path};

    #[test]
    fn resolves_workspace_root_from_slab_settings_path() {
        let root = workspace_root_from_settings_path(std::path::Path::new(
            r"C:\Users\cyberhan\Desktop\demo\.slab\settings.json",
        ))
        .expect("workspace root");

        assert_eq!(root, std::path::PathBuf::from(r"C:\Users\cyberhan\Desktop\demo"));
    }

    #[test]
    fn rejects_non_workspace_settings_path() {
        assert!(
            workspace_root_from_settings_path(std::path::Path::new(
                r"C:\Users\cyberhan\Desktop\demo\settings.json",
            ))
            .is_none()
        );
    }

    #[test]
    fn builtin_provider_matches_supported_language() {
        let provider = builtin_language_server_provider("typescript").expect("provider");

        assert_eq!(provider.id, "builtin.typescript");
    }
}
