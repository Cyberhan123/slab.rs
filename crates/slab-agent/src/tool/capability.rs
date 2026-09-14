//! Tool capability metadata: visibility, namespace, identity, static capability.

/// When/how a tool appears in the model-facing tool list. Orthogonal to the
/// category-based exposure filter driven by permission/interaction mode:
/// visibility governs *whether* a tool is a candidate at all, exposure governs
/// *which categories* are permitted this turn. Together they let the registry
/// scale to many tools (plugins/MCP) without bloating the LLM tool list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolVisibility {
    /// Always a candidate for the model-facing tool list, subject to category
    /// exposure. The default for built-in tools.
    #[default]
    Direct,
    /// Not shown to the model until `tool_search` injects it for the current
    /// turn. The default for plugin/MCP tools — keeps the base tool list small
    /// and the model discovers them on demand.
    Deferred,
    /// Never shown to the model, but still dispatchable via the registry.
    /// Used for internal/helper tools invoked only by other tools.
    Hidden,
}

/// Namespace a tool belongs to (e.g. `builtin`, `mcp:<server>`, `plugin:<id>`).
///
/// Used for namespaced dispatch (the `namespace__name` wire form) and capability
/// metadata. The default for built-in tools is [`ToolNamespace::builtin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNamespace(pub String);

impl ToolNamespace {
    /// The namespace all built-in tools belong to.
    pub const BUILTIN: &'static str = "builtin";

    /// Create a namespace from a string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The `builtin` namespace.
    pub fn builtin() -> Self {
        Self::new(Self::BUILTIN)
    }

    /// View the namespace string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ToolNamespace {
    fn default() -> Self {
        Self::builtin()
    }
}

/// A structured tool identity: a [`ToolNamespace`] plus a name within it.
///
/// The canonical wire form is `namespace__name` for namespaced tools and a bare
/// `name` for built-in tools (e.g. `shell`, `write_file`). MCP proxy names like
/// `mcp__server__tool` parse to namespace `mcp` / name `server__tool` and
/// round-trip losslessly via [`ToolName::to_wire`].
///
/// `ToolName` is a parse/classify helper only — tool structs keep returning a
/// cached wire-form `&str` from [`crate::ToolHandler::name`] (the trait requires `&str`,
/// not `String`); `ToolName` is used where structured namespace reasoning is
/// needed (e.g. deciding whether a name is built-in vs. namespaced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolName {
    /// Namespace the tool belongs to.
    pub namespace: ToolNamespace,
    /// Name within the namespace. For namespaced tools this may itself contain
    /// `__` (e.g. the `server__tool` part of an MCP proxy name).
    pub name: String,
}

impl ToolName {
    /// Create a namespaced tool name.
    pub fn new(namespace: ToolNamespace, name: impl Into<String>) -> Self {
        Self { namespace, name: name.into() }
    }

    /// Create a built-in tool name (namespace = `builtin`).
    pub fn builtin(name: impl Into<String>) -> Self {
        Self { namespace: ToolNamespace::builtin(), name: name.into() }
    }

    /// Whether this is a built-in (un-namespaced) tool.
    pub fn is_builtin(&self) -> bool {
        self.namespace.as_str() == ToolNamespace::BUILTIN
    }

    /// Render the canonical wire form. Built-in tools render as their bare name;
    /// namespaced tools render as `namespace__name`.
    pub fn to_wire(&self) -> String {
        if self.is_builtin() {
            self.name.clone()
        } else {
            format!("{}__{}", self.namespace.as_str(), self.name)
        }
    }

    /// Parse a wire-form name. A string with no `__` (or an empty side) is
    /// treated as a built-in bare name; otherwise the first `__` splits
    /// namespace from the (possibly multi-segment) name.
    pub fn parse_wire(value: &str) -> Self {
        match value.split_once("__") {
            Some((ns, name)) if !ns.is_empty() && !name.is_empty() => {
                Self::new(ToolNamespace::new(ns), name)
            }
            _ => Self::builtin(value),
        }
    }
}

/// Static capability metadata for a tool — the single source of truth consumed
/// by tool-exposure filtering, approval/risk routing, and (future) per-agent
/// tool constraints.
///
/// Per-call operation descriptors (which depend on the invocation arguments)
/// stay on [`crate::ToolHandler::describe_operation`]; this struct captures the static
/// metadata declared once per tool and cached by the registry.
#[derive(Debug, Clone)]
pub struct ToolCapability {
    /// Coarse operation category — drives category-based exposure filtering
    /// (read-only / shell / file-edit / network).
    pub category: slab_exec_policy::OperationCategory,
    /// Whether/when the tool appears in the model-facing tool list.
    pub visibility: ToolVisibility,
    /// Namespace the tool belongs to.
    pub namespace: ToolNamespace,
    /// Optional static risk hint. `None` (the default) defers to the runtime
    /// [`ToolRiskAnalyzer`](crate::ToolRiskAnalyzer); tools with a known static
    /// risk may declare it here so the analyzer need not infer it by name.
    pub risk_level: Option<crate::port::ToolRiskLevel>,
}

impl ToolCapability {
    /// Build a capability from a category, defaulting visibility to
    /// [`ToolVisibility::Direct`] and namespace to builtin.
    pub fn new(category: slab_exec_policy::OperationCategory) -> Self {
        Self {
            category,
            visibility: ToolVisibility::Direct,
            namespace: ToolNamespace::builtin(),
            risk_level: None,
        }
    }
}

impl Default for ToolCapability {
    fn default() -> Self {
        Self::new(slab_exec_policy::OperationCategory::ReadOnly)
    }
}
