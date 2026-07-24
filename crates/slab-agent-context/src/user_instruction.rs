//! User-role fragments.
//!
//! [`AgentMdFragment`] renders a discovered `AGENTS.md` wrapped in the default
//! `<INSTRUCTIONS>` block. [`SkillFragment`] renders a single skill's full
//! `SKILL.md` body inside a `<skill>` block (progressive disclosure: only the
//! *invoked* skill body is expanded into the user role).
//!
//! [`SkillFragment::detect_in_text`] is the server-side detector that finds
//! which known skills a user message refers to (`/name`, `$name`, or an exact
//! delimited name token). It deliberately does no fuzzy/description matching.

use std::collections::HashSet;

use crate::fragment::ContextFragment;
use crate::helper::{AGENTS_MD_TEMPLATE_NAME, SKILL_TEMPLATE_NAME};
use crate::skill_manager::SkillRecord;

/// Full `AGENTS.md` body wrapped for injection into the user role.
#[derive(Debug, Clone)]
pub struct AgentMdFragment {
    pub path: String,
    pub body: String,
}

impl ContextFragment for AgentMdFragment {
    fn role(&self) -> &'static str {
        "user"
    }

    fn template_name(&self) -> &'static str {
        AGENTS_MD_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "body": self.body,
        })
    }
}

/// A single invoked skill's full body, wrapped for injection into the user role.
#[derive(Debug, Clone)]
pub struct SkillFragment {
    pub name: String,
    pub path: String,
    pub contents: String,
}

impl ContextFragment for SkillFragment {
    fn role(&self) -> &'static str {
        "user"
    }

    fn template_name(&self) -> &'static str {
        SKILL_TEMPLATE_NAME
    }

    fn render_context(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "path": self.path,
            "contents": self.contents,
        })
    }
}

impl SkillFragment {
    /// Find skills the user message refers to.
    ///
    /// Triggers, in priority order: a `/name` or `$name` slash-mention, or an
    /// exact name token delimited by non-identifier characters. No fuzzy or
    /// description matching. Results preserve the input order of `known`
    /// (workspace skills first) and are de-duplicated by name.
    pub fn detect_in_text<'a>(text: &str, known: &'a [SkillRecord]) -> Vec<&'a SkillRecord> {
        let mut matched = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        for skill in known {
            let name = skill.name.as_str();
            if name.is_empty() {
                continue;
            }
            if (has_slash_mention(text, name) || has_delimited_token(text, name))
                && seen.insert(name)
            {
                matched.push(skill);
            }
        }
        matched
    }
}

fn has_slash_mention(text: &str, name: &str) -> bool {
    for prefix in ['/', '$'] {
        let needle = format!("{prefix}{name}");
        let mut from = 0;
        while let Some(relative) = text[from..].find(&needle) {
            let start = from + relative;
            let after = start + needle.len();
            if is_delimiter(text[after..].chars().next()) {
                return true;
            }
            from = start + prefix.len_utf8();
        }
    }
    false
}

fn has_delimited_token(text: &str, name: &str) -> bool {
    let mut from = 0;
    while let Some(relative) = text[from..].find(name) {
        let start = from + relative;
        let end = start + name.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if is_delimiter(before) && is_delimiter(after) {
            return true;
        }
        from = end.max(start + 1);
    }
    false
}

/// A character (or string boundary) that delimits a skill-name token. Anything
/// that may appear in a skill id (`[A-Za-z0-9_-]`) is a continuation, not a
/// delimiter.
fn is_delimiter(ch: Option<char>) -> bool {
    match ch {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric() && c != '-' && c != '_',
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn skill(name: &str) -> SkillRecord {
        SkillRecord {
            name: name.to_owned(),
            description: String::new(),
            path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            source: crate::skill_manager::SkillSource::Workspace,
        }
    }

    #[test]
    fn detects_slash_mention() {
        let known = vec![skill("rust-code-style"), skill("vitest")];
        let matched = SkillFragment::detect_in_text("please use /rust-code-style now", &known);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].name, "rust-code-style");
    }

    #[test]
    fn detects_dollar_mention() {
        let known = vec![skill("vitest")];
        let matched = SkillFragment::detect_in_text("run $vitest checks", &known);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn detects_exact_delimited_token() {
        let known = vec![skill("rust-code-style")];
        let matched = SkillFragment::detect_in_text("I want rust-code-style applied", &known);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn ignores_substring_inside_larger_token() {
        let known = vec![skill("rust")];
        // "rusting" should not match "rust" because the token continues.
        let matched = SkillFragment::detect_in_text("we are rusting fast", &known);
        assert!(matched.is_empty());
    }

    #[test]
    fn deduplicates_and_keeps_input_order() {
        let known = vec![skill("a"), skill("b")];
        let matched = SkillFragment::detect_in_text("/a and also a again and /b", &known);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].name, "a");
        assert_eq!(matched[1].name, "b");
    }

    #[test]
    fn no_match_for_unrelated_text() {
        let known = vec![skill("rust-code-style")];
        assert!(SkillFragment::detect_in_text("just chatting about nothing", &known).is_empty());
    }
}
