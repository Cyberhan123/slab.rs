use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{NetworkPolicy, SandboxEnvironment, SandboxError, SandboxPolicy, SandboxedCommand};

pub(crate) fn validate_command(
    env: &SandboxEnvironment,
    cmd: &SandboxedCommand,
) -> Result<(), SandboxError> {
    if matches!(env.policy, SandboxPolicy::DangerFullAccess) {
        return Ok(());
    }

    let cwd = effective_cwd(env, cmd);
    if let Some(cwd) = cwd.as_deref() {
        ensure_cwd_allowed(env, cwd)?;
    }

    let command_text = cmd.argv.join(" ");
    ensure_no_namespace_escape(&command_text)?;
    ensure_network_policy(env, &command_text)?;

    let write_targets = extract_write_targets(&command_text);
    if matches!(env.policy, SandboxPolicy::ReadOnly) && is_write_like(&command_text) {
        return Err(SandboxError::PermissionDenied(
            "read-only sandbox refused a write-like command".to_string(),
        ));
    }

    if matches!(env.policy, SandboxPolicy::ReadOnly) {
        return Ok(());
    }

    let allowed_roots = writable_roots(env);
    for target in write_targets {
        ensure_target_allowed(env, cwd.as_deref(), &target, &allowed_roots)?;
    }

    Ok(())
}

fn effective_cwd(env: &SandboxEnvironment, cmd: &SandboxedCommand) -> Option<PathBuf> {
    cmd.cwd.clone().or_else(|| env.workspace_root.clone())
}

fn ensure_cwd_allowed(env: &SandboxEnvironment, cwd: &Path) -> Result<(), SandboxError> {
    if let Some(workspace_root) = &env.workspace_root
        && path_is_within(cwd, workspace_root)
    {
        return Ok(());
    }

    for readable_root in &env.permissions.readable_roots {
        if path_is_within(cwd, readable_root) {
            return Ok(());
        }
    }

    Err(SandboxError::PermissionDenied(format!(
        "sandbox cwd is outside readable roots: {}",
        cwd.display()
    )))
}

fn writable_roots(env: &SandboxEnvironment) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(workspace_root) = &env.workspace_root {
        roots.push(workspace_root.clone());
    }
    roots.extend(env.permissions.writable_roots.iter().cloned());
    roots.push(std::env::temp_dir());
    roots
}

fn ensure_target_allowed(
    env: &SandboxEnvironment,
    cwd: Option<&Path>,
    target: &str,
    allowed_roots: &[PathBuf],
) -> Result<(), SandboxError> {
    if target.trim().is_empty() {
        return Ok(());
    }

    let expanded = expand_known_env(target);
    ensure_no_path_namespace_escape(&expanded)?;

    let path = path_from_token(cwd, &expanded);
    ensure_not_protected(env, &path)?;
    ensure_not_denied_by_glob(env, &path)?;

    if allowed_roots.iter().any(|root| path_is_within(&path, root)) {
        return Ok(());
    }

    Err(SandboxError::PermissionDenied(format!(
        "write target is outside sandbox writable roots: {}",
        path.display()
    )))
}

fn ensure_no_namespace_escape(command: &str) -> Result<(), SandboxError> {
    let lower = command.to_ascii_lowercase();
    if lower.contains("start-process") && (lower.contains("http://") || lower.contains("https://"))
    {
        return Err(SandboxError::PermissionDenied(
            "refused GUI URL launch from sandbox".to_string(),
        ));
    }

    for token in ["\\\\?\\", "\\\\.\\physicaldrive", "\\\\.\\pipe\\", "globalroot"] {
        if lower.contains(token) {
            return Err(SandboxError::PermissionDenied(format!(
                "refused Windows namespace escape: {token}"
            )));
        }
    }

    Ok(())
}

fn ensure_no_path_namespace_escape(path: &str) -> Result<(), SandboxError> {
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("\\\\") || lower.starts_with("//") {
        return Err(SandboxError::PermissionDenied(format!("refused UNC path: {path}")));
    }
    if lower.starts_with("\\\\?\\") || lower.starts_with("\\\\.\\") {
        return Err(SandboxError::PermissionDenied(format!("refused Windows device path: {path}")));
    }
    if has_alternate_data_stream(path) {
        return Err(SandboxError::PermissionDenied(format!(
            "refused alternate data stream path: {path}"
        )));
    }
    Ok(())
}

fn ensure_network_policy(env: &SandboxEnvironment, command: &str) -> Result<(), SandboxError> {
    if !matches!(env.permissions.network, NetworkPolicy::Blocked) {
        return Ok(());
    }

    let lower = command.to_ascii_lowercase();
    if !(lower.contains("http://") || lower.contains("https://")) {
        return Ok(());
    }

    if let Some(proxy) = &env.permissions.managed_proxy {
        let allowed_loopback = proxy.allowed_loopback_ports.iter().any(|port| {
            lower.contains(&format!("127.0.0.1:{port}"))
                || lower.contains(&format!("localhost:{port}"))
        });
        if allowed_loopback {
            return Ok(());
        }
    }

    Err(SandboxError::PermissionDenied("network access is blocked by sandbox policy".to_string()))
}

fn ensure_not_protected(env: &SandboxEnvironment, path: &Path) -> Result<(), SandboxError> {
    for component in path.components() {
        let name = match component {
            Component::Normal(name) => name.to_string_lossy(),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {
                continue;
            }
        };
        if env
            .permissions
            .protected_path_names
            .iter()
            .any(|protected| protected.eq_ignore_ascii_case(&name))
        {
            return Err(SandboxError::PermissionDenied(format!(
                "write target is protected: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn ensure_not_denied_by_glob(env: &SandboxEnvironment, path: &Path) -> Result<(), SandboxError> {
    if env.permissions.denied_paths.iter().any(|denied| path_is_within(path, denied)) {
        return Err(SandboxError::PermissionDenied(format!(
            "path is denied by sandbox policy: {}",
            path.display()
        )));
    }

    let Some(globs) = build_glob_set(&env.permissions.denied_globs) else {
        return Ok(());
    };
    if globs.is_match(path) {
        return Err(SandboxError::PermissionDenied(format!(
            "path is denied by sandbox glob: {}",
            path.display()
        )));
    }
    Ok(())
}

fn build_glob_set(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().ok()
}

fn is_write_like(command: &str) -> bool {
    if !extract_write_targets(command).is_empty() {
        return true;
    }
    let lower = command.to_ascii_lowercase();
    [
        "set-content",
        "writeallbytes",
        "new-item",
        "mkdir",
        " rmdir ",
        " del ",
        " move ",
        " ren ",
        "open(",
        "createfile",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

fn extract_write_targets(command: &str) -> Vec<String> {
    let mut targets = redirection_targets(command);
    let tokens = tokenize(command);
    for (index, token) in tokens.iter().enumerate() {
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "-literalpath" | "-path")
            && let Some(next) = tokens.get(index + 1)
        {
            targets.push(next.clone());
        }
        if matches!(lower.as_str(), "del" | "erase" | "mkdir" | "rmdir" | "rd")
            && let Some(next) = first_non_switch(&tokens[index + 1..])
        {
            targets.push(next.to_string());
        }
        if matches!(lower.as_str(), "ren" | "rename" | "move")
            && let Some(next) = tokens.get(index + 2)
        {
            targets.push(next.clone());
        }
    }
    targets.extend(quoted_call_targets(command, "writeallbytes"));
    targets.extend(quoted_call_targets(command, "open"));
    targets
        .into_iter()
        .map(|target| clean_path_token(&target))
        .filter(|target| !target.is_empty())
        .collect()
}

fn first_non_switch(tokens: &[String]) -> Option<&str> {
    tokens
        .iter()
        .find(|token| !token.starts_with('/') && !token.starts_with('-'))
        .map(String::as_str)
}

fn redirection_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let bytes = command.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'>' {
            index += 1;
            continue;
        }
        index += 1;
        if index < bytes.len() && bytes[index] == b'>' {
            index += 1;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let start = index;
        let mut quote = None;
        while index < bytes.len() {
            let ch = bytes[index] as char;
            if quote == Some(ch) {
                quote = None;
                index += 1;
                continue;
            }
            if quote.is_none() && matches!(ch, '"' | '\'') {
                quote = Some(ch);
                index += 1;
                continue;
            }
            if quote.is_none() && (ch.is_ascii_whitespace() || matches!(ch, '&' | '|')) {
                break;
            }
            index += 1;
        }
        targets.push(command[start..index].to_string());
    }
    targets
}

fn quoted_call_targets(command: &str, function_name: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let lower = command.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(found) = lower[search_from..].find(function_name) {
        let start = search_from + found + function_name.len();
        let Some(open_paren) = command[start..].find('(').map(|offset| start + offset) else {
            break;
        };
        let rest = &command[open_paren + 1..];
        let Some(quote_offset) = rest.find(['\'', '"']) else {
            break;
        };
        let quote_index = open_paren + 1 + quote_offset;
        let quote = command.as_bytes()[quote_index] as char;
        let value_start = quote_index + 1;
        let Some(value_end_offset) = command[value_start..].find(quote) else {
            break;
        };
        let value_end = value_start + value_end_offset;
        targets.push(command[value_start..value_end].to_string());
        search_from = value_end + 1;
    }
    targets
}

fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in command.chars() {
        if quote == Some(ch) {
            quote = None;
            continue;
        }
        if quote.is_none() && matches!(ch, '"' | '\'') {
            quote = Some(ch);
            continue;
        }
        if quote.is_none() && (ch.is_ascii_whitespace() || matches!(ch, '&' | '|')) {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn path_from_token(cwd: Option<&Path>, token: &str) -> PathBuf {
    let raw = PathBuf::from(token);
    if raw.is_absolute() {
        return raw;
    }
    cwd.map(|cwd| cwd.join(raw.clone())).unwrap_or(raw)
}

fn clean_path_token(token: &str) -> String {
    token
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(',')
        .trim_end_matches(')')
        .to_string()
}

fn expand_known_env(token: &str) -> String {
    let mut value = token.to_string();
    for key in ["TEMP", "TMP", "USERPROFILE", "HOME"] {
        if let Ok(env_value) = std::env::var(key) {
            value = value.replace(&format!("%{key}%"), &env_value);
            value = value.replace(&format!("$env:{key}"), &env_value);
            value = value.replace(&format!("${key}"), &env_value);
        }
    }
    value
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = canonical_or_lexical(path);
    let root = canonical_or_lexical(root);
    if cfg!(windows) {
        let path = path.to_string_lossy().to_ascii_lowercase();
        let root = root.to_string_lossy().to_ascii_lowercase();
        path == root || path.starts_with(&format!("{root}\\"))
    } else {
        path == root || path.starts_with(root)
    }
}

fn canonical_or_lexical(path: &Path) -> PathBuf {
    if let Ok(path) = dunce::canonicalize(path) {
        return normalize_lexically(&path);
    }
    if let Some(parent) = path.parent()
        && let Ok(parent) = dunce::canonicalize(parent)
        && let Some(name) = path.file_name()
    {
        return normalize_lexically(&parent.join(name));
    }
    normalize_lexically(path)
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn has_alternate_data_stream(path: &str) -> bool {
    let bytes = path.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b':' {
            continue;
        }
        if index == 1 && bytes.first().is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        if path[index..].contains("://") {
            continue;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SandboxPermissions, SandboxPolicy};

    #[test]
    fn read_only_rejects_redirection() {
        let env =
            SandboxEnvironment::new(Some(PathBuf::from("workspace")), SandboxPolicy::ReadOnly);
        let cmd = SandboxedCommand {
            argv: vec!["cmd".into(), "/c".into(), "echo x > file.txt".into()],
            env: Default::default(),
            cwd: Some(PathBuf::from("workspace")),
            timeout: None,
        };

        assert!(validate_command(&env, &cmd).is_err());
    }

    #[test]
    fn workspace_write_rejects_protected_path() {
        let env = SandboxEnvironment::new(
            Some(PathBuf::from("workspace")),
            SandboxPolicy::WorkspaceWrite,
        );
        let cmd = SandboxedCommand {
            argv: vec!["cmd".into(), "/c".into(), "echo x > .GiT\\config".into()],
            env: Default::default(),
            cwd: Some(PathBuf::from("workspace")),
            timeout: None,
        };

        assert!(validate_command(&env, &cmd).is_err());
    }

    #[test]
    fn workspace_write_allows_configured_root() {
        let mut permissions = SandboxPermissions::default();
        permissions.writable_roots.push(PathBuf::from("extra"));
        let env = SandboxEnvironment::with_permissions(
            Some(PathBuf::from(".")),
            SandboxPolicy::WorkspaceWrite,
            permissions,
        );
        let cmd = SandboxedCommand {
            argv: vec!["cmd".into(), "/c".into(), "echo x > extra\\ok.txt".into()],
            env: Default::default(),
            cwd: Some(PathBuf::from(".")),
            timeout: None,
        };

        assert!(validate_command(&env, &cmd).is_ok());
    }
}
