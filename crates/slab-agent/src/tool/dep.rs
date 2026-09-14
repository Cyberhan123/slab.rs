//! Host-service dependency keys that gate tool visibility.

/// A host service a tool can declare a dependency on (via
/// [`crate::ToolHandler::service_deps`]). Tools whose declared keys are not
/// all satisfied stay registered and dispatchable but are filtered out of the
/// model-facing projections — a PENDING state: once the host marks the key
/// satisfied, the tool becomes visible again without re-registration.
///
/// Closed on purpose: every variant has a host site that actually maintains
/// it (the app-core agent runtime reloader), so adding/renaming a variant is
/// a compile error at those sites instead of a silent dangling key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolServiceKey {
    /// The plugin `<id>` is currently enabled. Maintained by the agent
    /// runtime reloader: proxy tools stay registered for every installed
    /// plugin and this key gates them on the enable state (a disabled
    /// plugin's proxies disappear from the projections until re-enabled).
    PluginEnabled(String),
    /// The MCP server `<name>` is currently reachable. Maintained by the
    /// runtime reloader's server probe: dead-server proxies are hidden (not
    /// removed) so a transient disconnect heals on the next probe.
    McpServer(String),
}
