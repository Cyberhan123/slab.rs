//! Generalized, per-category rule set (migrated + generalized from
//! `slab-shell-command::rules`).
//!
//! On-disk format — one rule per line, whitespace-separated:
//! - Legacy (shell-only): `<action> <matcher> <pattern>`
//! - Category-prefixed:   `<category> <action> <matcher> <pattern>`
//!
//! The parser disambiguates by checking whether the first token parses as a
//! category name (the action/matcher/category token sets do not overlap), so
//! legacy `.rule` files load unchanged with category = Shell.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::category::OperationCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Allow,
    RequireApproval,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleMatcher {
    Exact,
    Prefix,
    Contains,
    /// Glob pattern compiled via `globset` (`*` matches any characters
    /// including separators). A trailing ` *` also matches the bare prefix, so
    /// `git *` matches `git`. Shell subjects are still screened for control
    /// chars so a glob cannot smuggle `&&`/`|`.
    Glob,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub category: OperationCategory,
    pub action: RuleAction,
    pub matcher: RuleMatcher,
    pub pattern: String,
    /// Optional per-tool-name scope (Claude-Code-style `Bash(git *)`). `None`
    /// matches any tool of [`category`]; `Some(pattern)` matches only the named
    /// tool, where `pattern` supports a trailing `*` glob for namespaced tools
    /// (e.g. `mcp__github__*`).
    pub tool_name: Option<String>,
    pub source: RuleSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSource {
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("shell rule path is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("failed to read rule directory {path}: {source}")]
    ReadDir { path: PathBuf, source: io::Error },
    #[error("failed to read rule directory entry in {path}: {source}")]
    ReadDirEntry { path: PathBuf, source: io::Error },
    #[error("failed to read rule file {path}: {source}")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("invalid rule in {path}:{line}: {reason}")]
    InvalidLine { path: PathBuf, line: usize, reason: String },
}

impl RuleSet {
    /// Load every rule file in `path` (sorted by filename for stable ordering).
    pub fn from_dir(path: impl AsRef<Path>) -> Result<Self, RuleError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        if !path.is_dir() {
            return Err(RuleError::NotDirectory { path: path.to_path_buf() });
        }

        let mut files = Vec::new();
        for entry in fs::read_dir(path)
            .map_err(|source| RuleError::ReadDir { path: path.to_path_buf(), source })?
        {
            let entry = entry
                .map_err(|source| RuleError::ReadDirEntry { path: path.to_path_buf(), source })?;
            let entry_path = entry.path();
            if entry_path.is_file() && is_rule_file(&entry_path) {
                files.push(entry_path);
            }
        }
        files.sort();

        let mut rules = Vec::new();
        for file in files {
            let content = fs::read_to_string(&file)
                .map_err(|source| RuleError::ReadFile { path: file.clone(), source })?;
            for (line_index, raw_line) in content.lines().enumerate() {
                if let Some(rule) = parse_rule_line(&file, line_index + 1, raw_line)? {
                    rules.push(rule);
                }
            }
        }

        Ok(Self { rules })
    }

    /// Load rules from a single file (used by the lazy per-workspace loader).
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, RuleError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|source| RuleError::ReadFile { path: path.to_path_buf(), source })?;
        let mut rules = Vec::new();
        for (line_index, raw_line) in content.lines().enumerate() {
            if let Some(rule) = parse_rule_line(path, line_index + 1, raw_line)? {
                rules.push(rule);
            }
        }
        Ok(Self { rules })
    }

    pub fn from_rules(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Append a rule (used by persistence — the in-memory set updates
    /// immediately so subsequent evaluations see it without a reload).
    pub fn append(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// First-match-wins evaluation scoped to `category` (and optionally to
    /// `tool_name`). `tool_name` is the descriptor's tool name; a rule scoped
    /// via `tool=<pattern>` only matches when it agrees.
    pub fn evaluate(
        &self,
        category: OperationCategory,
        subject: &str,
        tool_name: Option<&str>,
    ) -> Option<&Rule> {
        let subject = subject.trim();
        self.rules.iter().find(|rule| rule.category == category && rule.matches(subject, tool_name))
    }
}

impl Rule {
    pub fn new(
        category: OperationCategory,
        action: RuleAction,
        matcher: RuleMatcher,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            category,
            action,
            matcher,
            pattern: pattern.into(),
            tool_name: None,
            source: RuleSource { path: PathBuf::new(), line: 0 },
        }
    }

    /// Scope the rule to a specific tool name (supports a trailing `*` glob for
    /// namespaced tools). Builder-style; `None` (the default) matches any tool.
    #[must_use]
    pub fn with_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.tool_name = Some(tool_name.into());
        self
    }

    /// Serialize back to the on-disk line format. An optional leading
    /// `tool=<pattern>` token encodes the per-tool-name scope; omit it for the
    /// legacy "any tool of this category" semantics.
    pub fn to_line(&self) -> String {
        let prefix = match &self.tool_name {
            Some(tool) => format!("tool={tool} "),
            None => String::new(),
        };
        format!(
            "{prefix}{} {} {} {}",
            self.category.as_str(),
            action_token(self.action),
            matcher_token(self.matcher),
            self.pattern
        )
    }

    fn matches(&self, subject: &str, tool_name: Option<&str>) -> bool {
        // Per-tool-name axis: a scoped rule only matches the named tool. If the
        // descriptor carries no tool name, a scoped rule does not apply.
        if let Some(rule_tool) = &self.tool_name {
            let Some(name) = tool_name else { return false };
            if !tool_name_matches(rule_tool, name) {
                return false;
            }
        }
        match self.matcher {
            RuleMatcher::Exact => subject == self.pattern,
            // Shell prefix matching enforces a token boundary and refuses shell
            // control chars (so `cargo check` does not match `cargo checkout`).
            // Other categories use a literal string prefix (paths, URLs).
            RuleMatcher::Prefix => match self.category {
                OperationCategory::Shell => prefix_matches(subject, &self.pattern),
                _ => subject.starts_with(self.pattern.as_str()),
            },
            RuleMatcher::Contains => subject.contains(&self.pattern),
            RuleMatcher::Glob => glob_matches(subject, &self.pattern, self.category),
        }
    }
}

/// Match a `tool=` pattern against a tool name. A trailing `*` is a wildcard
/// prefix (so `mcp__github__*` matches `mcp__github__create_issue` and `*`
/// alone matches everything); otherwise exact.
fn tool_name_matches(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        name == pattern
    }
}

/// Glob-match a subject against `pattern`. `*` matches any characters including
/// separators (`literal_separator(false)`). A trailing ` *` also matches the
/// bare prefix. Shell subjects are screened for control chars first.
fn glob_matches(subject: &str, pattern: &str, category: OperationCategory) -> bool {
    if category == OperationCategory::Shell && contains_shell_control(subject) {
        return false;
    }
    let Ok(glob) = globset::GlobBuilder::new(pattern).literal_separator(false).build() else {
        return false;
    };
    if glob.compile_matcher().is_match(subject) {
        return true;
    }
    // Claude-Code nicety: `git *` should also match the bare `git`.
    if let Some(prefix) = pattern.strip_suffix(" *") {
        return subject == prefix;
    }
    false
}

fn is_rule_file(path: &Path) -> bool {
    let matches_ext = |ext: &str| path.extension().and_then(|e| e.to_str()) == Some(ext);
    let matches_name = |name: &str| path.file_name().and_then(|n| n.to_str()) == Some(name);
    matches_ext("rule") || matches_ext("rules") || matches_name(".rule") || matches_name(".rules")
}

fn parse_rule_line(path: &Path, line: usize, raw_line: &str) -> Result<Option<Rule>, RuleError> {
    let raw_line = raw_line.trim();
    if raw_line.is_empty() || raw_line.starts_with('#') {
        return Ok(None);
    }

    // Optional leading `tool=<pattern>` scopes the rule to a tool name (a
    // trailing `*` globs namespaced tools, e.g. `mcp__github__*`). Omit it for
    // the legacy "any tool of this category" semantics. Recognized only at the
    // start of the line so a legitimate `tool=` pattern is unambiguous.
    let (tool_name, rest) = if let Some(after) = raw_line.strip_prefix("tool=") {
        let (token, rest) = take_token(after)
            .ok_or_else(|| invalid_rule(path, line, "missing tool name after 'tool='"))?;
        let token = token.trim_end_matches(',');
        if token.is_empty() {
            return Err(invalid_rule(path, line, "empty tool name after 'tool='"));
        }
        (Some(token.to_owned()), rest)
    } else {
        (None, raw_line)
    };

    // Disambiguate 4-token (category-prefixed) vs 3-token (legacy shell).
    let (category, rest) = match take_token(rest) {
        Some((first, after_first)) => match OperationCategory::parse(first) {
            Some(category) => (category, after_first),
            // Legacy 3-token form: `first` was the action, so re-parse the
            // action from the (post-tool) line, not from after `first`.
            None => (OperationCategory::Shell, rest),
        },
        None => return Err(invalid_rule(path, line, "missing action")),
    };

    let (action_token, rest) =
        take_token(rest).ok_or_else(|| invalid_rule(path, line, "missing action"))?;
    let (matcher_token, pattern) =
        take_token(rest).ok_or_else(|| invalid_rule(path, line, "missing matcher"))?;
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(invalid_rule(path, line, "missing pattern"));
    }

    Ok(Some(Rule {
        category,
        action: parse_action(action_token)
            .ok_or_else(|| invalid_rule(path, line, "unknown action"))?,
        matcher: parse_matcher(matcher_token)
            .ok_or_else(|| invalid_rule(path, line, "unknown matcher"))?,
        pattern: pattern.to_owned(),
        tool_name,
        source: RuleSource { path: path.to_path_buf(), line },
    }))
}

fn take_token(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    Some((&input[..end], &input[end..]))
}

fn parse_action(value: &str) -> Option<RuleAction> {
    match value.to_ascii_lowercase().as_str() {
        "allow" | "approve" | "auto" | "auto_approve" => Some(RuleAction::Allow),
        "ask" | "approval" | "require" | "require_approval" => Some(RuleAction::RequireApproval),
        "block" | "deny" => Some(RuleAction::Block),
        _ => None,
    }
}

fn parse_matcher(value: &str) -> Option<RuleMatcher> {
    match value.to_ascii_lowercase().as_str() {
        "exact" => Some(RuleMatcher::Exact),
        "prefix" | "starts_with" => Some(RuleMatcher::Prefix),
        "contains" => Some(RuleMatcher::Contains),
        "glob" | "wildcard" => Some(RuleMatcher::Glob),
        _ => None,
    }
}

fn action_token(action: RuleAction) -> &'static str {
    match action {
        RuleAction::Allow => "allow",
        RuleAction::RequireApproval => "require_approval",
        RuleAction::Block => "block",
    }
}

fn matcher_token(matcher: RuleMatcher) -> &'static str {
    match matcher {
        RuleMatcher::Exact => "exact",
        RuleMatcher::Prefix => "prefix",
        RuleMatcher::Contains => "contains",
        RuleMatcher::Glob => "glob",
    }
}

fn invalid_rule(path: &Path, line: usize, reason: &str) -> RuleError {
    RuleError::InvalidLine { path: path.to_path_buf(), line, reason: reason.to_owned() }
}

fn prefix_matches(subject: &str, pattern: &str) -> bool {
    let Some(rest) = subject.strip_prefix(pattern) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    if !rest.chars().next().is_some_and(char::is_whitespace) {
        return false;
    }
    !contains_shell_control(rest)
}

fn contains_shell_control(value: &str) -> bool {
    ["&&", "||", ";", "|", ">", "<", "$(", "${", "`", "&", "\n", "\r"]
        .iter()
        .any(|pattern| value.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::OperationCategory;

    fn temp_rules_dir() -> PathBuf {
        let name = format!(
            "slab-exec-policy-rules-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        std::env::temp_dir().join(name)
    }

    #[test]
    fn missing_rule_dir_is_empty() {
        let rules = RuleSet::from_dir(temp_rules_dir()).expect("missing dir allowed");
        assert!(rules.is_empty());
    }

    #[test]
    fn loads_rule_files_in_directory_order() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("rules dir");
        fs::write(dir.join("20-second.rules"), "shell block contains Remove-Item\n")
            .expect("write");
        fs::write(dir.join("10-first.rules"), "shell allow prefix cargo check\n").expect("write");

        let rules = RuleSet::from_dir(&dir).expect("rules load");

        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules
                .evaluate(OperationCategory::Shell, "cargo check -p slab-agent", None)
                .map(|r| r.action),
            Some(RuleAction::Allow)
        );
        assert_eq!(
            rules
                .evaluate(OperationCategory::Shell, "Remove-Item file.txt", None)
                .map(|r| r.action),
            Some(RuleAction::Block)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_three_token_lines_default_to_shell_category() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("rules dir");
        // No leading category token — legacy shell-only format.
        fs::write(dir.join("legacy.rule"), "allow prefix cargo check\n").expect("write");

        let rules = RuleSet::from_dir(&dir).expect("rules load");
        let rule = rules.evaluate(OperationCategory::Shell, "cargo check", None).expect("matched");
        assert_eq!(rule.category, OperationCategory::Shell);
        assert_eq!(rule.action, RuleAction::Allow);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn category_scoped_evaluation() {
        let rules = RuleSet::from_rules(vec![
            Rule::new(OperationCategory::Network, RuleAction::Allow, RuleMatcher::Prefix, "https"),
            Rule::new(OperationCategory::Shell, RuleAction::Block, RuleMatcher::Contains, "rm"),
        ]);

        assert_eq!(
            rules
                .evaluate(OperationCategory::Network, "https://example.com", None)
                .map(|r| r.action),
            Some(RuleAction::Allow)
        );
        // A shell subject containing "rm" must NOT match the network rule.
        assert_eq!(
            rules.evaluate(OperationCategory::Shell, "rm -rf target", None).map(|r| r.action),
            Some(RuleAction::Block)
        );
        // File-edit operations see no rules.
        assert!(rules.evaluate(OperationCategory::FileEdit, "/etc/hosts", None).is_none());
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = RuleSet::from_rules(vec![
            Rule::new(
                OperationCategory::Shell,
                RuleAction::RequireApproval,
                RuleMatcher::Prefix,
                "cargo",
            ),
            Rule::new(
                OperationCategory::Shell,
                RuleAction::Allow,
                RuleMatcher::Prefix,
                "cargo check",
            ),
        ]);

        assert_eq!(
            rules
                .evaluate(OperationCategory::Shell, "cargo check -p slab-agent", None)
                .map(|r| r.action),
            Some(RuleAction::RequireApproval)
        );
    }

    #[test]
    fn prefix_requires_token_boundary_and_single_shell_segment() {
        let rules = RuleSet::from_rules(vec![Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "cargo check",
        )]);

        assert!(
            rules.evaluate(OperationCategory::Shell, "cargo check -p slab-agent", None).is_some()
        );
        assert!(rules.evaluate(OperationCategory::Shell, "cargo checkout", None).is_none());
        assert!(
            rules
                .evaluate(OperationCategory::Shell, "cargo check && Remove-Item file.txt", None)
                .is_none()
        );
    }

    /// A remembered prefix Allow must not chain a second command via a SINGLE
    /// `&` (background operator) or expand attacker-controlled text via `${…}`
    /// — both smuggle a new command the user never approved.
    #[test]
    fn prefix_refuses_background_chaining_and_parameter_expansion_suffixes() {
        let rules = RuleSet::from_rules(vec![Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "cargo test",
        )]);

        assert!(
            rules.evaluate(OperationCategory::Shell, "cargo test & npm install x", None).is_none()
        );
        assert!(
            rules
                .evaluate(OperationCategory::Shell, "cargo test ${SNEAKY:-&&curl evil}", None)
                .is_none()
        );
        // The plain argument-extension form still matches.
        assert!(
            rules.evaluate(OperationCategory::Shell, "cargo test -- --nocapture", None).is_some()
        );
    }

    #[test]
    fn to_line_round_trips_through_parser() {
        let rule = Rule::new(
            OperationCategory::Network,
            RuleAction::Allow,
            RuleMatcher::Prefix,
            "https://example.com",
        );
        let line = rule.to_line();
        assert_eq!(line, "network allow prefix https://example.com");

        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("out.rules");
        fs::write(&file, format!("{line}\n")).expect("write");
        let parsed = RuleSet::from_file(&file).expect("load");
        let parsed_rule = &parsed.rules()[0];
        assert_eq!(parsed_rule.category, rule.category);
        assert_eq!(parsed_rule.action, rule.action);
        assert_eq!(parsed_rule.matcher, rule.matcher);
        assert_eq!(parsed_rule.pattern, rule.pattern);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_rule_lines() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("rules dir");
        fs::write(dir.join("bad.rule"), "allow prefix\n").expect("write");

        let error = RuleSet::from_dir(&dir).expect_err("invalid rule should fail");
        assert!(matches!(error, RuleError::InvalidLine { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    // ---- P2: Glob matcher ----

    #[test]
    fn glob_matcher_matches_wildcard_and_bare_prefix() {
        let rules = RuleSet::from_rules(vec![Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Glob,
            "git *",
        )]);
        // `*` matches any trailing args (including separators) …
        assert!(matches!(
            rules.evaluate(OperationCategory::Shell, "git add src/main.rs", None).map(|r| r.action),
            Some(RuleAction::Allow)
        ));
        // … and the trailing-" *" nicety also matches the bare prefix.
        assert!(matches!(
            rules.evaluate(OperationCategory::Shell, "git", None).map(|r| r.action),
            Some(RuleAction::Allow)
        ));
        // Non-matching command.
        assert!(rules.evaluate(OperationCategory::Shell, "cargo build", None).is_none());
    }

    #[test]
    fn glob_matcher_refuses_shell_control_chars() {
        let rules = RuleSet::from_rules(vec![Rule::new(
            OperationCategory::Shell,
            RuleAction::Allow,
            RuleMatcher::Glob,
            "git *",
        )]);
        // A glob must not smuggle shell control chars past the guard.
        assert!(rules.evaluate(OperationCategory::Shell, "git status && rm -rf /", None).is_none());
    }

    #[test]
    fn glob_path_pattern_matches_across_separators() {
        let rules = RuleSet::from_rules(vec![Rule::new(
            OperationCategory::FileEdit,
            RuleAction::Allow,
            RuleMatcher::Glob,
            "src/**/*.rs",
        )]);
        assert!(
            rules.evaluate(OperationCategory::FileEdit, "src/deep/nested/mod.rs", None).is_some()
        );
        assert!(rules.evaluate(OperationCategory::FileEdit, "tests/main.rs", None).is_none());
    }

    // ---- P1: per-tool-name axis ----

    #[test]
    fn tool_name_axis_scopes_rule_to_named_tool() {
        let rules = RuleSet::from_rules(vec![
            Rule::new(OperationCategory::FileEdit, RuleAction::Allow, RuleMatcher::Glob, "*")
                .with_tool_name("write_file"),
        ]);

        // Same category + subject, but only the named tool matches.
        assert_eq!(
            rules
                .evaluate(OperationCategory::FileEdit, "/ws/a.txt", Some("write_file"))
                .map(|r| r.action),
            Some(RuleAction::Allow)
        );
        assert!(
            rules.evaluate(OperationCategory::FileEdit, "/ws/a.txt", Some("apply_patch")).is_none()
        );
        // Unscoped (tool_name=None) descriptor does not match a scoped rule.
        assert!(rules.evaluate(OperationCategory::FileEdit, "/ws/a.txt", None).is_none());
    }

    #[test]
    fn tool_name_glob_matches_namespaced_tools() {
        let rules = RuleSet::from_rules(vec![
            Rule::new(OperationCategory::Shell, RuleAction::Block, RuleMatcher::Glob, "*")
                .with_tool_name("mcp__github__*"),
        ]);

        assert_eq!(
            rules
                .evaluate(OperationCategory::Shell, "anything", Some("mcp__github__create_issue"))
                .map(|r| r.action),
            Some(RuleAction::Block)
        );
        assert!(
            rules
                .evaluate(OperationCategory::Shell, "anything", Some("mcp__linear__create_issue"))
                .is_none()
        );
    }

    #[test]
    fn parses_and_round_trips_tool_prefix() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("scoped.rules"), "tool=write_file file_edit allow glob *\n")
            .expect("write");
        let rules = RuleSet::from_dir(&dir).expect("load");
        let rule = rules.rules()[0].clone();
        assert_eq!(rule.tool_name.as_deref(), Some("write_file"));
        assert_eq!(rule.matcher, RuleMatcher::Glob);
        assert_eq!(rule.to_line(), "tool=write_file file_edit allow glob *");
        assert_eq!(
            rules
                .evaluate(OperationCategory::FileEdit, "/anywhere", Some("write_file"))
                .map(|r| r.action),
            Some(RuleAction::Allow)
        );
        let _ = fs::remove_dir_all(dir);
    }
}
