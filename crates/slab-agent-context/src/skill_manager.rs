//! Skill discovery.
//!
//! Scans workspace (`.agents/skills/**/SKILL.md`) and global app-home
//! (`<app_home>/skills/**/SKILL.md`) skill sources, parses the YAML
//! frontmatter for `name` and `description`, enforces size/scan budgets, and
//! deduplicates with workspace winning over global when both `name` and
//! `description` match.

use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::error::{Result, io_error};

/// Maximum length of a skill `name` (frontmatter), truncated with `…` past it.
pub const MAX_NAME_LEN: usize = 64;
/// Maximum length of a skill `description` (frontmatter).
pub const MAX_DESCRIPTION_LEN: usize = 1024;
/// Maximum directory depth of a skill-root walk.
pub const MAX_SCAN_DEPTH: usize = 6;
/// Maximum number of skill directories accepted per root before truncation.
pub const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;

/// Where a skill was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    Workspace,
    Global,
}

/// A discovered skill: frontmatter summary plus the absolute `SKILL.md` path.
#[derive(Debug, Clone, Serialize)]
pub struct SkillRecord {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: SkillSource,
}

impl SkillRecord {
    /// Short alias used inside prompts (`skill://<name>`), mapped to the real
    /// path via the skill-roots table.
    pub fn alias(&self) -> String {
        format!("skill://{}", self.name)
    }
}

/// Scan workspace then global skill sources.
///
/// Workspace skills live at `<workspace>/.agents/skills/**/SKILL.md`; global
/// skills at `<app_home>/skills/**/SKILL.md`. When a workspace and a global
/// skill share the same `name` AND `description`, the workspace entry wins.
pub fn scan_skills(workspace_root: Option<&Path>, app_home_skills_dir: &Path) -> Vec<SkillRecord> {
    let workspace = workspace_root
        .map(|root| scan_root(&root.join(".agents").join("skills"), SkillSource::Workspace))
        .unwrap_or_default();
    let global = scan_root(app_home_skills_dir, SkillSource::Global);

    // Merge with workspace-wins dedup on identical (name, description).
    let mut seen: std::collections::HashSet<(String, String)> =
        workspace.iter().map(|skill| (skill.name.clone(), skill.description.clone())).collect();
    let mut merged = workspace;
    for skill in global {
        if seen.insert((skill.name.clone(), skill.description.clone())) {
            merged.push(skill);
        }
    }
    merged
}

/// Read the full contents of a `SKILL.md` (frontmatter + body) for expansion.
pub fn read_skill_contents(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| io_error(path, source))
}

fn scan_root(root: &Path, source: SkillSource) -> Vec<SkillRecord> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if found.len() >= MAX_SKILLS_DIRS_PER_ROOT {
            break;
        }
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
            continue;
        }
        if let Some(record) = parse_skill_md(path, source) {
            found.push(record);
        }
    }
    found
}

fn parse_skill_md(path: &Path, source: SkillSource) -> Option<SkillRecord> {
    let text = std::fs::read_to_string(path).ok()?;
    let (name, description) = parse_frontmatter(&text)?;
    let name = crate::helper::truncate(name.trim(), MAX_NAME_LEN);
    let description = crate::helper::truncate(description.trim(), MAX_DESCRIPTION_LEN);
    if name.is_empty() {
        return None;
    }
    Some(SkillRecord { name, description, path: path.to_path_buf(), source })
}

/// Extract `(name, description)` from a `SKILL.md` YAML frontmatter block.
///
/// Returns `None` when there is no leading `---` fence. Handles inline scalars
/// (`name: foo`) and block scalars (`description: |` / `>` followed by indented
/// lines). This is a deliberately small parser — the repo's `SKILL.md` files
/// only use these forms.
fn parse_frontmatter(text: &str) -> Option<(String, String)> {
    let mut lines = text.lines();
    let first = lines.next()?.trim();
    if first != "---" {
        return None;
    }
    let mut block: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        block.push(line);
    }
    Some(parse_yaml_fields(&block))
}

fn parse_yaml_fields(block: &[&str]) -> (String, String) {
    let mut name = String::new();
    let mut description = String::new();
    let mut index = 0;
    while index < block.len() {
        let line = block[index];
        let trimmed = line.trim_start_matches(' ');
        if let Some(rest) = trimmed.strip_prefix("name:") {
            name = unquote(rest.trim());
        } else if let Some(rest) = trimmed.strip_prefix("description:") {
            let inline = rest.trim();
            let is_block_scalar = inline.is_empty()
                || inline == "|"
                || inline == ">"
                || inline == "|-"
                || inline == ">-";
            if is_block_scalar {
                let mut collected = String::new();
                index += 1;
                while index < block.len() {
                    let candidate = block[index];
                    if candidate.is_empty() {
                        collected.push('\n');
                        index += 1;
                        continue;
                    }
                    let dedented = candidate.trim_start_matches(' ');
                    // A non-indented line ends the block scalar.
                    if dedented.len() == candidate.len() {
                        break;
                    }
                    collected.push_str(dedented);
                    collected.push('\n');
                    index += 1;
                }
                description = collected.trim().to_owned();
                continue;
            } else {
                description = unquote(inline);
            }
        }
        index += 1;
    }
    (name, description)
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    let len = trimmed.len();
    if (len >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (len >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..len - 1].to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn write_skill(root: &Path, name: &str, body: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    #[test]
    fn parses_inline_frontmatter() {
        let tmp = TempDir::new().unwrap();
        write_skill(
            tmp.path(),
            "rust-code-style",
            "---\nname: rust-code-style\ndescription: Use when working on Rust code.\n---\n# Rust\nbody\n",
        );
        let skills = scan_root(tmp.path(), SkillSource::Workspace);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "rust-code-style");
        assert_eq!(skills[0].description, "Use when working on Rust code.");
        assert_eq!(skills[0].source, SkillSource::Workspace);
    }

    #[test]
    fn parses_block_scalar_description() {
        let tmp = TempDir::new().unwrap();
        write_skill(
            tmp.path(),
            "multi",
            "---\nname: multi\ndescription: |\n  line one\n  line two\n---\nbody\n",
        );
        let skills = scan_root(tmp.path(), SkillSource::Global);
        assert_eq!(skills[0].description, "line one\nline two");
    }

    #[test]
    fn truncates_long_fields() {
        let tmp = TempDir::new().unwrap();
        let long_name = "a".repeat(100);
        let long_desc = "d".repeat(2048);
        write_skill(
            tmp.path(),
            "skill",
            &format!("---\nname: {long_name}\ndescription: {long_desc}\n---\n"),
        );
        let skills = scan_root(tmp.path(), SkillSource::Workspace);
        assert!(skills[0].name.chars().count() <= MAX_NAME_LEN);
        assert!(skills[0].name.ends_with('…'));
        assert!(skills[0].description.chars().count() <= MAX_DESCRIPTION_LEN);
        assert!(skills[0].description.ends_with('…'));
    }

    #[test]
    fn skips_files_without_frontmatter() {
        let tmp = TempDir::new().unwrap();
        write_skill(tmp.path(), "plain", "# just a heading\nno frontmatter");
        let skills = scan_root(tmp.path(), SkillSource::Workspace);
        assert!(skills.is_empty());
    }

    #[test]
    fn workspace_wins_on_full_duplicate() {
        let ws = TempDir::new().unwrap();
        let global = TempDir::new().unwrap();
        let body = "---\nname: dup\ndescription: same\n---\n";
        write_skill(&ws.path().join(".agents").join("skills"), "dup", body);
        write_skill(global.path(), "dup", body);
        let skills = scan_skills(Some(ws.path()), global.path());
        // Exactly one entry, sourced from the workspace.
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].source, SkillSource::Workspace);
    }

    #[test]
    fn keeps_both_when_name_differs() {
        let ws = TempDir::new().unwrap();
        let global = TempDir::new().unwrap();
        write_skill(
            &ws.path().join(".agents").join("skills"),
            "a",
            "---\nname: shared\ndescription: ws-only\n---\n",
        );
        write_skill(global.path(), "b", "---\nname: shared\ndescription: global-only\n---\n");
        let skills = scan_skills(Some(ws.path()), global.path());
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn missing_root_is_empty() {
        let skills = scan_root(Path::new("/does/not/exist"), SkillSource::Global);
        assert!(skills.is_empty());
    }

    #[test]
    fn caps_skill_count_per_root() {
        let tmp = TempDir::new().unwrap();
        for i in 0..(MAX_SKILLS_DIRS_PER_ROOT + 5) {
            write_skill(tmp.path(), &format!("s{i}"), "---\nname: x\ndescription: y\n---\n");
        }
        let skills = scan_root(tmp.path(), SkillSource::Workspace);
        assert_eq!(skills.len(), MAX_SKILLS_DIRS_PER_ROOT);
    }
}
