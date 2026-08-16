use async_trait::async_trait;

#[cfg(target_os = "macos")]
use crate::guard::validate_command;
use crate::{
    IsolationStrength, NetworkPolicy, SandboxCapabilities, SandboxDriver, SandboxEnvironment,
    SandboxError, SandboxIsolation, SandboxPlatform, SandboxPolicy, SandboxSetupStatus,
    SandboxedCommand, SandboxedOutput, SetupKind,
};

pub struct MacosSandboxDriver {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    env: SandboxEnvironment,
}

impl MacosSandboxDriver {
    pub fn new(env: SandboxEnvironment) -> Self {
        Self { env }
    }
}

#[async_trait]
impl SandboxDriver for MacosSandboxDriver {
    fn name(&self) -> &str {
        "macos-seatbelt"
    }

    async fn run(&self, cmd: SandboxedCommand) -> Result<SandboxedOutput, SandboxError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = cmd;
            return Err(SandboxError::UnsupportedPlatform);
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Stdio;

            use crate::driver::{command_env, unix_kill_tree, wait_for_child};

            validate_command(&self.env, &cmd)?;

            // Escape hatch: when the seatbelt wrapper is disabled, run the child directly without
            // `/usr/bin/sandbox-exec`. Only the lexical guard (`validate_command`) applies, and
            // `capabilities()` honestly reports `Degraded`/lexical isolation for this case.
            if !self.env.permissions.platform.macos_use_sandbox_exec {
                let argv0 = cmd
                    .argv
                    .first()
                    .ok_or_else(|| SandboxError::SpawnFailed("empty argv".to_string()))?;
                let mut command = tokio::process::Command::new(argv0);
                if cmd.argv.len() > 1 {
                    command.args(&cmd.argv[1..]);
                }
                for (key, value) in command_env(&self.env, &cmd) {
                    command.env(key, value);
                }
                if let Some(ref cwd) = cmd.cwd {
                    command.current_dir(cwd);
                }
                command.kill_on_drop(true);
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());
                command.process_group(0);
                let spawned =
                    command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
                let kill_tree = unix_kill_tree(spawned.id());
                return wait_for_child(spawned, cmd.timeout, cmd.output_sink.clone(), kill_tree)
                    .await;
            }

            let profile = build_seatbelt_profile(&self.env);
            let profile_path = std::env::temp_dir().join(format!(
                "slab-seatbelt-{}-{}.sbpl",
                std::process::id(),
                monotonic_nanos()
            ));
            std::fs::write(&profile_path, profile)
                .map_err(|e| SandboxError::SetupFailed(e.to_string()))?;

            // Fixed absolute path — never resolve via PATH, preventing PATH injection.
            let mut command = tokio::process::Command::new("/usr/bin/sandbox-exec");
            command.arg("-f");
            command.arg(&profile_path);
            command.arg("--");
            command.args(&cmd.argv);
            for (key, value) in command_env(&self.env, &cmd) {
                command.env(key, value);
            }
            if let Some(ref cwd) = cmd.cwd {
                command.current_dir(cwd);
            }
            command.kill_on_drop(true);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            // sandbox-exec doesn't create a new session, so put it in its own
            // process group so the whole tree can be killed after it exits.
            command.process_group(0);

            let spawned = command.spawn().map_err(|e| SandboxError::SpawnFailed(e.to_string()))?;
            let kill_tree = unix_kill_tree(spawned.id());
            let output =
                wait_for_child(spawned, cmd.timeout, cmd.output_sink.clone(), kill_tree).await;
            let _ = std::fs::remove_file(profile_path);
            output
        }
    }

    fn capabilities(&self) -> SandboxCapabilities {
        let on_macos = cfg!(target_os = "macos");
        // seatbelt `(deny default)` is OS-enforced for both dimensions — but only when the wrapper
        // is actually enabled. When the knob is off, only the lexical guard applies.
        let seatbelt_active = on_macos && self.env.permissions.platform.macos_use_sandbox_exec;
        SandboxCapabilities {
            platform: SandboxPlatform::Macos,
            isolation: if seatbelt_active {
                SandboxIsolation::Full
            } else if on_macos {
                SandboxIsolation::Degraded
            } else {
                SandboxIsolation::Unsupported
            },
            filesystem: seatbelt_active,
            network: seatbelt_active,
            filesystem_isolation: if seatbelt_active {
                IsolationStrength::OsEnforced
            } else if on_macos {
                IsolationStrength::Lexical
            } else {
                IsolationStrength::None
            },
            network_isolation: if seatbelt_active {
                IsolationStrength::OsEnforced
            } else {
                IsolationStrength::None
            },
            process_cleanup: on_macos,
            setup_required: false,
            setup_kind: if seatbelt_active { SetupKind::Seatbelt } else { SetupKind::None },
        }
    }

    fn setup_status(&self) -> SandboxSetupStatus {
        #[cfg(target_os = "macos")]
        {
            if std::path::Path::new("/usr/bin/sandbox-exec").exists() {
                SandboxSetupStatus::ready("sandbox-exec is available")
            } else {
                SandboxSetupStatus::unavailable("sandbox-exec is not available")
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            SandboxSetupStatus::unavailable("macOS sandbox is only available on macOS")
        }
    }
}

/// Build the seatbelt `sandbox-exec` profile text for the given environment.
///
/// The profile is deny-by-default (`(deny default)`) and selectively re-opens access. seatbelt is
/// last-match-wins, so the deny directives for `denied_paths` / `denied_globs` / protected metadata
/// are emitted AFTER the writable-root allows so they override. Each denied path gets BOTH a
/// `(literal ...)` and a `(subpath ...)` deny: `file-write*` does not cover directory creation
/// (`mkdir`) of the literal entry in all seatbelt versions, so the literal deny blocks re-creating
/// a denied directory (e.g. `.git/`) inside a writable root.
///
/// Pure string-building (no macOS APIs) so it compiles + unit-tests on all platforms; only the
/// `run()` invocation of `/usr/bin/sandbox-exec` is `cfg(target_os = "macos")`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn build_seatbelt_profile(env: &SandboxEnvironment) -> String {
    let mut lines = vec![
        "(version 1)".to_string(),
        "(deny default)".to_string(),
        "(allow process*)".to_string(),
        "(allow file-read*)".to_string(),
        "(allow sysctl-read)".to_string(),
        "(allow mach-lookup)".to_string(),
    ];

    if matches!(env.permissions.network, NetworkPolicy::Allowed)
        || env.permissions.managed_proxy.is_some()
    {
        lines.push("(allow network*)".to_string());
    }

    match env.policy {
        SandboxPolicy::ReadOnly => {}
        SandboxPolicy::WorkspaceWrite => {
            if let Some(root) = &env.workspace_root {
                allow_write_subpaths(&mut lines, root);
            }
            for root in &env.permissions.writable_roots {
                allow_write_subpaths(&mut lines, root);
            }
            allow_write_subpaths(&mut lines, &std::env::temp_dir());
        }
        SandboxPolicy::DangerFullAccess => {
            lines.push("(allow file-write*)".to_string());
        }
    }

    // Denied paths: literal + subpath dual-deny (blocks dir-creation bypass), read+write.
    for denied in &env.permissions.denied_paths {
        deny_file_both(&mut lines, denied);
    }
    // Denied globs: translate to an anchored ICU regex and deny read+write.
    deny_globs_regex(&mut lines, &env.permissions.denied_globs);
    // Protected metadata (e.g. .git/.slab/.agents) inside the writable root: read-only, so
    // write-only dual-deny (literal + subpath). Read stays allowed by the global `(allow file-read*)`.
    for name in &env.permissions.protected_path_names {
        if let Some(root) = &env.workspace_root {
            deny_write_both(&mut lines, &root.join(name));
        }
    }

    lines.join("\n")
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn allow_write_subpaths(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        lines.push(format!("(allow file-write* (subpath \"{}\"))", escape_sbpl_path(&path)));
    }
}

/// Deny ALL file operations (read + write) on a path via BOTH literal and subpath matches.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn deny_file_both(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        let escaped = escape_sbpl_path(&path);
        lines.push(format!("(deny file* (literal \"{escaped}\"))"));
        lines.push(format!("(deny file* (subpath \"{escaped}\"))"));
    }
}

/// Deny file-WRITE operations on a path via BOTH literal and subpath matches (read stays allowed).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn deny_write_both(lines: &mut Vec<String>, path: &std::path::Path) {
    for path in seatbelt_paths(path) {
        let escaped = escape_sbpl_path(&path);
        lines.push(format!("(deny file-write* (literal \"{escaped}\"))"));
        lines.push(format!("(deny file-write* (subpath \"{escaped}\"))"));
    }
}

/// Translate each denied glob into an anchored seatbelt `(regex ...)` deny (read + write).
/// Anchored to the full path (`^...$`) to mirror `globset::GlobSet` full-match semantics.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn deny_globs_regex(lines: &mut Vec<String>, globs: &[String]) {
    for glob in globs {
        let regex = glob_to_seatbelt_regex(glob);
        // The regex sits inside an SBPL double-quoted string: escape backslashes (doubling) and
        // quotes so the SBPL parser yields the intended regex to the seatbelt regex engine.
        let escaped = regex.replace('\\', "\\\\").replace('"', "\\\"");
        lines.push(format!("(deny file* (regex \"{escaped}\"))"));
    }
}

/// Translate a glob into an ICU-regex (seatbelt `(regex ...)`) anchored to the full path.
///
/// Handles the common globset metacharacters: `**` -> `.*`, `*` -> `[^/]*`, `?` -> `[^/]`. Other
/// ICU regex metacharacters in literal segments are escaped. Character classes (`[...]`) and brace
/// alternation (`{...}`) are treated as literals — a documented limitation; use `*`/`**`/`?`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn glob_to_seatbelt_regex(glob: &str) -> String {
    let chars: Vec<char> = glob.chars().collect();
    let mut out = String::with_capacity(glob.len() + 4);
    out.push('^');
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    out.push_str(".*");
                    i += 2;
                    continue;
                }
                out.push_str("[^/]*");
            }
            '?' => out.push_str("[^/]"),
            // Escape ICU regex metacharacters so literal text matches verbatim.
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out.push('$');
    out
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn seatbelt_paths(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths = vec![path.to_path_buf()];
    if let Ok(canonical) = dunce::canonicalize(path)
        && !paths.iter().any(|candidate| candidate == &canonical)
    {
        paths.push(canonical);
    }
    paths
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn escape_sbpl_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn monotonic_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NetworkPolicy, SandboxPermissions};

    fn env_with(policy: SandboxPolicy, permissions: SandboxPermissions) -> SandboxEnvironment {
        SandboxEnvironment::with_permissions(
            Some(std::path::PathBuf::from("/ws/root")),
            policy,
            permissions,
        )
    }

    fn lines_with<'a>(profile: &'a str, needle: &str) -> Vec<&'a str> {
        profile.lines().filter(|line| line.contains(needle)).collect()
    }

    #[test]
    fn profile_denies_denied_paths_with_literal_and_subpath() {
        let mut perms = SandboxPermissions::default();
        perms.denied_paths.push(std::path::PathBuf::from("/deny/deniedpathmarker"));
        let env = env_with(SandboxPolicy::WorkspaceWrite, perms);
        let profile = build_seatbelt_profile(&env);
        let hits = lines_with(&profile, "deniedpathmarker");
        assert!(
            hits.iter().any(|line| line.contains("(deny file* (literal ")),
            "missing literal deny for denied_path: {profile}"
        );
        assert!(
            hits.iter().any(|line| line.contains("(deny file* (subpath ")),
            "missing subpath deny for denied_path: {profile}"
        );
    }

    #[test]
    fn profile_makes_protected_metadata_read_only() {
        let env = env_with(SandboxPolicy::WorkspaceWrite, SandboxPermissions::default());
        let profile = build_seatbelt_profile(&env);
        // .git is a default protected_path_names entry; under WorkspaceWrite it must be write-denied
        // via BOTH literal and subpath (read stays allowed).
        let git_hits = lines_with(&profile, ".git");
        assert!(
            git_hits.iter().any(|line| line.contains("(deny file-write* (literal ")),
            "missing literal write-deny for protected metadata: {profile}"
        );
        assert!(
            git_hits.iter().any(|line| line.contains("(deny file-write* (subpath ")),
            "missing subpath write-deny for protected metadata: {profile}"
        );
        // No read deny on .git (it stays read-only, not unreadable).
        assert!(
            !git_hits.iter().any(|line| line.contains("(deny file*")),
            "protected metadata must not be read-denied: {profile}"
        );
    }

    #[test]
    fn profile_converts_denied_globs_to_regex_deny() {
        let mut perms = SandboxPermissions::default();
        perms.denied_globs.push("**/secretglob".to_string());
        let env = env_with(SandboxPolicy::WorkspaceWrite, perms);
        let profile = build_seatbelt_profile(&env);
        let hits = lines_with(&profile, "secretglob");
        assert!(
            hits.iter().any(|line| line.starts_with("(deny file* (regex ")),
            "missing regex deny for denied_glob: {profile}"
        );
    }

    #[test]
    fn glob_to_regex_translates_metacharacters() {
        assert_eq!(glob_to_seatbelt_regex("*.env"), "^[^/]*\\.env$");
        assert_eq!(glob_to_seatbelt_regex("**/.env"), "^.*/\\.env$");
        assert_eq!(glob_to_seatbelt_regex("a?b"), "^a[^/]b$");
        // Literal dots in paths are escaped, not treated as "any char".
        assert!(glob_to_seatbelt_regex("config.json").contains("\\.json"));
    }

    #[test]
    fn profile_allows_writable_subpaths_under_workspacewrite() {
        let env = env_with(SandboxPolicy::WorkspaceWrite, SandboxPermissions::default());
        let profile = build_seatbelt_profile(&env);
        assert!(
            profile.contains("(allow file-write* (subpath "),
            "WorkspaceWrite must allow writes under writable roots: {profile}"
        );
    }

    #[test]
    fn profile_network_allowed_only_when_permitted_or_managed_proxy() {
        let perms =
            SandboxPermissions { network: NetworkPolicy::Blocked, ..SandboxPermissions::default() };
        let env = env_with(SandboxPolicy::WorkspaceWrite, perms);
        assert!(!build_seatbelt_profile(&env).contains("(allow network*)"));

        let perms =
            SandboxPermissions { network: NetworkPolicy::Allowed, ..SandboxPermissions::default() };
        let env = env_with(SandboxPolicy::WorkspaceWrite, perms);
        assert!(build_seatbelt_profile(&env).contains("(allow network*)"));
    }

    #[test]
    fn escape_sbpl_path_escapes_backslash_and_quote() {
        let escaped = escape_sbpl_path(std::path::Path::new("a\\b\"c"));
        assert_eq!(escaped, "a\\\\b\\\"c");
    }

    #[test]
    fn capabilities_reflects_macos_use_sandbox_exec_knob() {
        // Knob enabled (default): full seatbelt isolation on macOS, none off-macOS.
        let env = env_with(SandboxPolicy::WorkspaceWrite, SandboxPermissions::default());
        let caps = MacosSandboxDriver::new(env).capabilities();
        if cfg!(target_os = "macos") {
            assert_eq!(caps.filesystem_isolation, IsolationStrength::OsEnforced);
            assert_eq!(caps.setup_kind, SetupKind::Seatbelt);
        } else {
            assert_eq!(caps.filesystem_isolation, IsolationStrength::None);
            assert_eq!(caps.setup_kind, SetupKind::None);
        }

        // Knob disabled: honestly degrades to lexical (macOS) / none (elsewhere).
        let perms = SandboxPermissions {
            platform: crate::SandboxPlatformConfig {
                macos_use_sandbox_exec: false,
                ..crate::SandboxPlatformConfig::default()
            },
            ..SandboxPermissions::default()
        };
        let env = env_with(SandboxPolicy::WorkspaceWrite, perms);
        let caps = MacosSandboxDriver::new(env).capabilities();
        if cfg!(target_os = "macos") {
            assert_eq!(caps.filesystem_isolation, IsolationStrength::Lexical);
            assert_eq!(caps.network_isolation, IsolationStrength::None);
            assert_eq!(caps.setup_kind, SetupKind::None);
            assert!(!caps.filesystem);
            assert!(!caps.network);
        } else {
            assert_eq!(caps.filesystem_isolation, IsolationStrength::None);
        }
    }
}
