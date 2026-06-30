//! Helpers for exposing plugin capabilities as agent-visible tools (B-7 / ADR-009).
//!
//! - [`plugin_agent_tool_name`]: stable, identifier-safe tool name
//!   `plugin__<plugin_id>__<capability_id>` (mirrors the MCP proxy naming so the
//!   agent tool namespace stays consistent).
//! - [`CapabilityEffectTrust`] / [`infer_effect_trust`]: a static, **host-inferred**
//!   trust tier derived from the plugin's runtime kind. Plugins cannot self-report
//!   effects/trust (red-team must_add; ADR-009 local-first trust model) — the host
//!   derives it from which runtime manifest the plugin ships.

/// Sanitize a name fragment to identifier-safe chars, mirroring the MCP proxy
/// sanitize in `crates/slab-agent-tools/src/mcp.rs`.
fn sanitize_name(value: &str) -> String {
    value.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' }).collect()
}

/// Build the agent-visible tool name for a plugin capability.
///
/// The result is stable (same inputs ⇒ same name) and identifier-safe, so it can
/// be matched against LLM tool-call names and used as a caller-id source.
pub fn plugin_agent_tool_name(plugin_id: &str, capability_id: &str) -> String {
    format!("plugin__{}__{}", sanitize_name(plugin_id), sanitize_name(capability_id))
}

/// Host-inferred trust tier for a plugin capability's effects, derived from the
/// plugin runtime kind (NOT self-reported by the plugin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityEffectTrust {
    /// JS plugin → Tauri child WebView sandbox.
    TauriSandbox,
    /// Python plugin → PyO3 isolate.
    PyoIsolate,
    /// WASM plugin → extism.
    Extism,
}

impl CapabilityEffectTrust {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TauriSandbox => "tauri_sandbox",
            Self::PyoIsolate => "pyo3_isolate",
            Self::Extism => "extism",
        }
    }
}

/// Infer the effect trust tier from which plugin runtimes are present.
/// When multiple runtimes are present, the stricter isolation wins
/// (Extism > PyO3 isolate > Tauri sandbox). Returns `None` when the plugin
/// ships no recognized runtime.
pub fn infer_effect_trust(
    has_js: bool,
    has_python: bool,
    has_wasm: bool,
) -> Option<CapabilityEffectTrust> {
    if has_wasm {
        Some(CapabilityEffectTrust::Extism)
    } else if has_python {
        Some(CapabilityEffectTrust::PyoIsolate)
    } else if has_js {
        Some(CapabilityEffectTrust::TauriSandbox)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_tool_name_is_stable_and_identifier_safe() {
        // Hyphens, spaces, and dots are sanitized to `_` (identifier-safe).
        assert_eq!(
            plugin_agent_tool_name("video-subtitle-translator", "translate"),
            "plugin__video_subtitle_translator__translate"
        );
        assert_eq!(
            plugin_agent_tool_name("Team Plugin", "search.v1"),
            "plugin__Team_Plugin__search_v1"
        );
        // Same inputs ⇒ same name.
        let a = plugin_agent_tool_name("p", "c");
        let b = plugin_agent_tool_name("p", "c");
        assert_eq!(a, b);
    }

    #[test]
    fn infer_effect_trust_picks_stricter_isolation() {
        assert_eq!(
            infer_effect_trust(true, false, false),
            Some(CapabilityEffectTrust::TauriSandbox)
        );
        assert_eq!(infer_effect_trust(false, true, false), Some(CapabilityEffectTrust::PyoIsolate));
        assert_eq!(infer_effect_trust(false, false, true), Some(CapabilityEffectTrust::Extism));
        // Multiple runtimes ⇒ strictest (Extism wins over PyO3 and Tauri).
        assert_eq!(infer_effect_trust(true, true, true), Some(CapabilityEffectTrust::Extism));
        assert_eq!(infer_effect_trust(true, true, false), Some(CapabilityEffectTrust::PyoIsolate));
        // No recognized runtime.
        assert_eq!(infer_effect_trust(false, false, false), None);
    }

    #[test]
    fn capability_effect_trust_has_stable_string_form() {
        assert_eq!(CapabilityEffectTrust::TauriSandbox.as_str(), "tauri_sandbox");
        assert_eq!(CapabilityEffectTrust::PyoIsolate.as_str(), "pyo3_isolate");
        assert_eq!(CapabilityEffectTrust::Extism.as_str(), "extism");
    }
}
