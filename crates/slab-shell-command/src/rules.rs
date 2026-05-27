use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRuleAction {
    Allow,
    RequireApproval,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRuleMatcher {
    Exact,
    Prefix,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRule {
    pub action: ShellRuleAction,
    pub matcher: ShellRuleMatcher,
    pub pattern: String,
    pub source: RuleSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSource {
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellRuleSet {
    rules: Vec<ShellRule>,
}

impl ShellRuleSet {
    pub fn from_dir(path: impl AsRef<Path>) -> Result<Self, ShellRuleError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        if !path.is_dir() {
            return Err(ShellRuleError::NotDirectory { path: path.to_path_buf() });
        }

        let mut files = Vec::new();
        for entry in fs::read_dir(path)
            .map_err(|source| ShellRuleError::ReadDir { path: path.to_path_buf(), source })?
        {
            let entry = entry.map_err(|source| ShellRuleError::ReadDirEntry {
                path: path.to_path_buf(),
                source,
            })?;
            let entry_path = entry.path();
            if entry_path.is_file() && is_rule_file(&entry_path) {
                files.push(entry_path);
            }
        }
        files.sort();

        let mut rules = Vec::new();
        for file in files {
            let content = fs::read_to_string(&file)
                .map_err(|source| ShellRuleError::ReadFile { path: file.clone(), source })?;
            for (line_index, raw_line) in content.lines().enumerate() {
                if let Some(rule) = parse_rule_line(&file, line_index + 1, raw_line)? {
                    rules.push(rule);
                }
            }
        }

        Ok(Self { rules })
    }

    pub fn from_rules(rules: Vec<ShellRule>) -> Self {
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn evaluate(&self, command: &str) -> Option<&ShellRule> {
        let command = command.trim();
        self.rules.iter().find(|rule| rule.matches(command))
    }
}

impl ShellRule {
    pub fn new(
        action: ShellRuleAction,
        matcher: ShellRuleMatcher,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            action,
            matcher,
            pattern: pattern.into(),
            source: RuleSource { path: PathBuf::new(), line: 0 },
        }
    }

    fn matches(&self, command: &str) -> bool {
        match self.matcher {
            ShellRuleMatcher::Exact => command == self.pattern,
            ShellRuleMatcher::Prefix => prefix_matches(command, &self.pattern),
            ShellRuleMatcher::Contains => command.contains(&self.pattern),
        }
    }
}

#[derive(Debug, Error)]
pub enum ShellRuleError {
    #[error("shell rule path is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    #[error("failed to read shell rule directory {path}: {source}")]
    ReadDir { path: PathBuf, source: io::Error },
    #[error("failed to read shell rule directory entry in {path}: {source}")]
    ReadDirEntry { path: PathBuf, source: io::Error },
    #[error("failed to read shell rule file {path}: {source}")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("invalid shell rule in {path}:{line}: {reason}")]
    InvalidLine { path: PathBuf, line: usize, reason: String },
}

fn is_rule_file(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(".rule")
        || path.extension().and_then(|extension| extension.to_str()) == Some("rule")
}

fn parse_rule_line(
    path: &Path,
    line: usize,
    raw_line: &str,
) -> Result<Option<ShellRule>, ShellRuleError> {
    let raw_line = raw_line.trim();
    if raw_line.is_empty() || raw_line.starts_with('#') {
        return Ok(None);
    }

    let (action, rest) =
        take_token(raw_line).ok_or_else(|| invalid_rule(path, line, "missing action"))?;
    let (matcher, pattern) =
        take_token(rest).ok_or_else(|| invalid_rule(path, line, "missing matcher"))?;
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(invalid_rule(path, line, "missing pattern"));
    }

    Ok(Some(ShellRule {
        action: parse_action(action).ok_or_else(|| invalid_rule(path, line, "unknown action"))?,
        matcher: parse_matcher(matcher)
            .ok_or_else(|| invalid_rule(path, line, "unknown matcher"))?,
        pattern: pattern.to_owned(),
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

fn parse_action(value: &str) -> Option<ShellRuleAction> {
    match value.to_ascii_lowercase().as_str() {
        "allow" | "approve" | "auto" | "auto_approve" => Some(ShellRuleAction::Allow),
        "ask" | "approval" | "require" | "require_approval" => {
            Some(ShellRuleAction::RequireApproval)
        }
        "block" | "deny" => Some(ShellRuleAction::Block),
        _ => None,
    }
}

fn parse_matcher(value: &str) -> Option<ShellRuleMatcher> {
    match value.to_ascii_lowercase().as_str() {
        "exact" => Some(ShellRuleMatcher::Exact),
        "prefix" | "starts_with" => Some(ShellRuleMatcher::Prefix),
        "contains" => Some(ShellRuleMatcher::Contains),
        _ => None,
    }
}

fn invalid_rule(path: &Path, line: usize, reason: &str) -> ShellRuleError {
    ShellRuleError::InvalidLine { path: path.to_path_buf(), line, reason: reason.to_owned() }
}

fn prefix_matches(command: &str, pattern: &str) -> bool {
    let Some(rest) = command.strip_prefix(pattern) else {
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
    ["&&", "||", ";", "|", "\n", "\r"].iter().any(|pattern| value.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_rules_dir() -> PathBuf {
        let name = format!(
            "slab-shell-rules-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        std::env::temp_dir().join(name)
    }

    #[test]
    fn missing_rule_dir_is_empty() {
        let rules =
            ShellRuleSet::from_dir(temp_rules_dir()).expect("missing dir should be allowed");

        assert!(rules.is_empty());
    }

    #[test]
    fn loads_rule_files_in_directory_order() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("rules dir");
        fs::write(dir.join("20-second.rule"), "block contains Remove-Item\n").expect("write");
        fs::write(dir.join("10-first.rule"), "allow prefix cargo check\n").expect("write");

        let rules = ShellRuleSet::from_dir(&dir).expect("rules should load");

        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules.evaluate("cargo check -p slab-agent").map(|rule| rule.action),
            Some(ShellRuleAction::Allow)
        );
        assert_eq!(
            rules.evaluate("Remove-Item file.txt").map(|rule| rule.action),
            Some(ShellRuleAction::Block)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = ShellRuleSet::from_rules(vec![
            ShellRule::new(ShellRuleAction::RequireApproval, ShellRuleMatcher::Prefix, "cargo"),
            ShellRule::new(ShellRuleAction::Allow, ShellRuleMatcher::Prefix, "cargo check"),
        ]);

        assert_eq!(
            rules.evaluate("cargo check -p slab-agent").map(|rule| rule.action),
            Some(ShellRuleAction::RequireApproval)
        );
    }

    #[test]
    fn prefix_requires_token_boundary_and_single_shell_segment() {
        let rules = ShellRuleSet::from_rules(vec![ShellRule::new(
            ShellRuleAction::Allow,
            ShellRuleMatcher::Prefix,
            "cargo check",
        )]);

        assert!(rules.evaluate("cargo check -p slab-agent").is_some());
        assert!(rules.evaluate("cargo checkout").is_none());
        assert!(rules.evaluate("cargo check && Remove-Item file.txt").is_none());
        assert!(rules.evaluate("cargo check; Remove-Item file.txt").is_none());
    }

    #[test]
    fn rejects_invalid_rule_lines() {
        let dir = temp_rules_dir();
        fs::create_dir_all(&dir).expect("rules dir");
        fs::write(dir.join("bad.rule"), "allow prefix\n").expect("write");

        let error = ShellRuleSet::from_dir(&dir).expect_err("invalid rule should fail");

        assert!(matches!(error, ShellRuleError::InvalidLine { .. }));

        let _ = fs::remove_dir_all(dir);
    }
}
