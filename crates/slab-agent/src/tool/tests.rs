use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::TypedTool;
use crate::port::ToolSpec;

// Minimal handler stub to exercise the default trait methods.
struct StubTool;

#[async_trait::async_trait]
impl TypedTool for StubTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        "stub"
    }
    fn description(&self) -> &str {
        "stub"
    }
    async fn execute(
        &self,
        _ctx: &ToolContext,
        _arguments: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        Ok(ToolOutput { content: String::new(), metadata: None })
    }
}

#[test]
fn tool_visibility_default_is_direct() {
    assert_eq!(ToolVisibility::default(), ToolVisibility::Direct);
}

#[test]
fn tool_namespace_builtin_and_default() {
    assert_eq!(ToolNamespace::builtin().as_str(), "builtin");
    assert_eq!(ToolNamespace::default().as_str(), "builtin");
    assert_eq!(ToolNamespace::new("mcp:foo").as_str(), "mcp:foo");
    assert_eq!(ToolNamespace(ToolNamespace::BUILTIN.to_owned()).as_str(), "builtin");
}

#[test]
fn tool_name_builtin_to_wire_is_bare() {
    let n = ToolName::builtin("shell");
    assert!(n.is_builtin());
    assert_eq!(n.to_wire(), "shell");
    assert_eq!(n.namespace.as_str(), "builtin");
    assert_eq!(n.name, "shell");
}

#[test]
fn parse_wire_treats_bare_name_as_builtin() {
    assert!(ToolName::parse_wire("write_file").is_builtin());
    assert!(ToolName::parse_wire("task.complete").is_builtin());
    // No "__" at all → builtin.
    assert_eq!(ToolName::parse_wire("grep").to_wire(), "grep");
}

#[test]
fn tool_name_namespaced_round_trip() {
    let n = ToolName::parse_wire("mcp__server__tool");
    assert!(!n.is_builtin());
    assert_eq!(n.namespace.as_str(), "mcp");
    assert_eq!(n.name, "server__tool");
    assert_eq!(n.to_wire(), "mcp__server__tool");
}

#[test]
fn mcp_proxy_name_parses_back() {
    let wire = "mcp__team_server__search_web_v1";
    assert_eq!(ToolName::parse_wire(wire).to_wire(), wire);
    // A single-segment namespaced name round-trips too.
    let single = "plugin__my_tool";
    let parsed = ToolName::parse_wire(single);
    assert_eq!(parsed.namespace.as_str(), "plugin");
    assert_eq!(parsed.name, "my_tool");
    assert_eq!(parsed.to_wire(), single);
}

#[test]
fn tool_capability_new_defaults() {
    let cap = ToolCapability::new(slab_exec_policy::OperationCategory::FileEdit);
    assert_eq!(cap.category, slab_exec_policy::OperationCategory::FileEdit);
    assert_eq!(cap.visibility, ToolVisibility::Direct);
    assert_eq!(cap.namespace.as_str(), "builtin");
    assert_eq!(cap.risk_level, None);
}

#[test]
fn tool_capability_default_is_read_only_direct() {
    let cap = ToolCapability::default();
    assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
    assert_eq!(cap.visibility, ToolVisibility::Direct);
}

#[test]
fn handler_default_capability_derives_from_category_and_visibility() {
    let tool = StubTool;
    // StubTool uses the default category() (ReadOnly) + default visibility (Direct).
    let cap = tool.capability();
    assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
    assert_eq!(cap.visibility, ToolVisibility::Direct);
    assert_eq!(cap.namespace.as_str(), "builtin");
    assert_eq!(cap.risk_level, None);
}

#[test]
fn handler_visibility_and_namespace_defaults() {
    let tool = StubTool;
    assert_eq!(ToolHandler::visibility(&tool), ToolVisibility::Direct);
    assert_eq!(ToolHandler::namespace(&tool).as_str(), "builtin");
}

// ── visible_tool_specs projection ──────────────────────────────────────────

fn noop_output() -> Result<ToolOutput, crate::error::AgentError> {
    Ok(ToolOutput { content: String::new(), metadata: None })
}

struct ReadOnlyDirectTool;
#[async_trait::async_trait]
impl TypedTool for ReadOnlyDirectTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        "read_direct"
    }
    fn description(&self) -> &str {
        "read direct"
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }
}

struct ShellDirectTool;
#[async_trait::async_trait]
impl TypedTool for ShellDirectTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        "shell_direct"
    }
    fn description(&self) -> &str {
        "shell direct"
    }
    fn category(&self) -> slab_exec_policy::OperationCategory {
        slab_exec_policy::OperationCategory::Shell
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }
}

struct DeferredTool;
#[async_trait::async_trait]
impl TypedTool for DeferredTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        "deferred_read"
    }
    fn description(&self) -> &str {
        "deferred read"
    }
    fn visibility(&self) -> ToolVisibility {
        ToolVisibility::Deferred
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }
}

struct HiddenTool;
#[async_trait::async_trait]
impl TypedTool for HiddenTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        "hidden_helper"
    }
    fn description(&self) -> &str {
        "hidden helper"
    }
    fn visibility(&self) -> ToolVisibility {
        ToolVisibility::Hidden
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }
}

fn router_with_all_visibilities() -> ToolRouter {
    let router = ToolRouter::new();
    router.register(Box::new(ReadOnlyDirectTool));
    router.register(Box::new(ShellDirectTool));
    router.register(Box::new(DeferredTool));
    router.register(Box::new(HiddenTool));
    router
}

fn names(specs: &[ToolSpec]) -> Vec<&str> {
    let mut names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    names
}

#[test]
fn visible_tool_specs_shows_direct_and_hides_hidden_and_deferred_by_default() {
    let router = router_with_all_visibilities();
    // Full exposure: visibility is the only filter.
    let specs = router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &HashSet::new());
    assert_eq!(names(&specs), ["read_direct", "shell_direct"]);
}

#[test]
fn visible_tool_specs_injects_deferred_only_when_named() {
    let router = router_with_all_visibilities();
    let mut injected = HashSet::new();
    injected.insert("deferred_read".to_owned());
    let specs = router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &injected);
    assert_eq!(names(&specs), ["deferred_read", "read_direct", "shell_direct"]);
}

#[test]
fn visible_tool_specs_respects_category_exposure() {
    let router = router_with_all_visibilities();
    // read-only exposure hides shell_direct (Shell category).
    let specs =
        router.visible_tool_specs(slab_exec_policy::ToolExposure::read_only(), &HashSet::new());
    assert_eq!(names(&specs), ["read_direct"]);
}

#[test]
fn visible_tool_specs_hidden_tool_still_dispatchable_but_never_visible() {
    let router = router_with_all_visibilities();
    assert!(router.get("hidden_helper").is_some());
    let specs = router.visible_tool_specs(slab_exec_policy::ToolExposure::all(), &HashSet::new());
    assert!(specs.iter().all(|s| s.name != "hidden_helper"));
}

#[test]
fn deferred_tool_specs_lists_only_deferred_tools() {
    let router = router_with_all_visibilities();
    // Only the Deferred tool surfaces as a search candidate; Direct/Hidden
    // tools never appear here (Direct are already in the base list, Hidden
    // are internal helpers).
    let specs = router.deferred_tool_specs();
    assert_eq!(names(&specs), ["deferred_read"]);
}

#[test]
fn discovery_state_inject_and_snapshot_round_trip() {
    let state = ToolDiscoveryState::new();
    assert!(state.snapshot().is_empty());
    state.inject("mcp__srv__tool");
    state.inject("plugin__p__cap");
    let snap = state.snapshot();
    assert_eq!(snap.len(), 2);
    assert!(snap.contains("mcp__srv__tool"));
    // Two independent states don't share injected sets (per-thread isolation).
    let other = ToolDiscoveryState::new();
    assert!(other.snapshot().is_empty());
}

#[test]
fn capability_of_returns_cached_metadata() {
    let router = router_with_all_visibilities();
    let cap = router.capability_of("deferred_read").expect("cached capability");
    assert_eq!(cap.visibility, ToolVisibility::Deferred);
    assert_eq!(cap.category, slab_exec_policy::OperationCategory::ReadOnly);
    let shell_cap = router.capability_of("shell_direct").expect("cached capability");
    assert_eq!(shell_cap.category, slab_exec_policy::OperationCategory::Shell);
    assert!(router.capability_of("nonexistent").is_none());
}

#[test]
fn unregister_drops_capability_cache() {
    let router = router_with_all_visibilities();
    assert!(router.capability_of("deferred_read").is_some());
    router.unregister("deferred_read");
    assert!(router.capability_of("deferred_read").is_none());
    assert!(router.get("deferred_read").is_none());
}

// ── dispose lifecycle ──────────────────────────────────────────────────────

/// TypedTool stub that records dispose calls on a shared flag.
struct DisposeTrackingTool {
    name: &'static str,
    disposed: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl TypedTool for DisposeTrackingTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "dispose tracking"
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }

    fn dispose(&self) {
        self.disposed.store(true, Ordering::SeqCst);
    }
}

#[test]
fn replacing_a_registration_disposes_the_old_handler() {
    let router = ToolRouter::new();
    let first_disposed = Arc::new(AtomicBool::new(false));
    router.register(Box::new(DisposeTrackingTool {
        name: "tracked",
        disposed: Arc::clone(&first_disposed),
    }));
    assert!(!first_disposed.load(Ordering::SeqCst));

    // Re-registering under the same name replaces (and disposes) the old one.
    router.register(Box::new(DisposeTrackingTool {
        name: "tracked",
        disposed: Arc::new(AtomicBool::new(false)),
    }));
    assert!(first_disposed.load(Ordering::SeqCst), "replaced handler must be disposed");
    assert!(router.get("tracked").is_some(), "the replacement is live");
}

#[test]
fn unregister_disposes_the_removed_handler() {
    let router = ToolRouter::new();
    let disposed = Arc::new(AtomicBool::new(false));
    router.register(Box::new(DisposeTrackingTool {
        name: "tracked",
        disposed: Arc::clone(&disposed),
    }));
    router.unregister("tracked");
    assert!(disposed.load(Ordering::SeqCst), "unregistered handler must be disposed");
    assert!(router.get("tracked").is_none());
}

#[test]
fn default_dispose_is_a_noop() {
    // The built-in stubs (no dispose override) flow through
    // register/unregister/replace without panicking.
    let router = router_with_all_visibilities();
    router.register(Box::new(StubTool));
    router.unregister("stub");
}

// ── registration handles and bulk removal ──────────────────────────────────

#[test]
fn registration_handle_dispose_unregisters() {
    let router = ToolRouter::new();
    let disposed = Arc::new(AtomicBool::new(false));
    let registration = router.register(Box::new(DisposeTrackingTool {
        name: "tracked",
        disposed: Arc::clone(&disposed),
    }));
    assert_eq!(registration.name(), "tracked");
    assert!(router.get("tracked").is_some());

    registration.dispose();
    assert!(disposed.load(Ordering::SeqCst), "handle dispose must dispose the handler");
    assert!(router.get("tracked").is_none());
    assert!(router.capability_of("tracked").is_none());
}

#[test]
fn dropping_a_registration_handle_keeps_the_tool_registered() {
    let router = ToolRouter::new();
    let disposed = Arc::new(AtomicBool::new(false));
    {
        let _registration = router.register(Box::new(DisposeTrackingTool {
            name: "tracked",
            disposed: Arc::clone(&disposed),
        }));
    } // handle dropped — no Drop-based auto-unregister by design.
    assert!(router.get("tracked").is_some(), "dropped handle must NOT unregister");
    assert!(!disposed.load(Ordering::SeqCst));
}

/// TypedTool stub that appends its name to a shared dispose log and carries a
/// configurable namespace (so bulk predicates can select it).
struct OrderedDisposeTool {
    name: &'static str,
    namespace: &'static str,
    log: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl TypedTool for OrderedDisposeTool {
    type Input = serde_json::Value;
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "ordered dispose"
    }
    fn namespace(&self) -> ToolNamespace {
        ToolNamespace::new(self.namespace)
    }
    async fn execute(
        &self,
        _: &ToolContext,
        _: serde_json::Value,
    ) -> Result<ToolOutput, crate::error::AgentError> {
        noop_output()
    }

    fn dispose(&self) {
        self.log.lock().expect("dispose log lock poisoned").push(self.name.to_owned());
    }
}

#[test]
fn unregister_where_disposes_in_reverse_registration_order() {
    let router = ToolRouter::new();
    let log: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let tool = |name: &'static str, namespace: &'static str| {
        Box::new(OrderedDisposeTool { name, namespace, log: Arc::clone(&log) })
            as Box<dyn ToolHandler>
    };
    // Registration order: builtin_a, plugin_x, plugin_y, builtin_b.
    router.register(tool("builtin_a", "builtin"));
    router.register(tool("plugin_x", "plugin"));
    router.register(tool("plugin_y", "plugin"));
    router.register(tool("builtin_b", "builtin"));

    let removed = router.unregister_where(|_name, cap| cap.namespace.as_str() == "plugin");

    // Removed names come back in registration order…
    assert_eq!(removed, ["plugin_x", "plugin_y"]);
    // …while disposal ran in REVERSE registration order.
    assert_eq!(*log.lock().expect("dispose log lock poisoned"), ["plugin_y", "plugin_x"]);
    // Non-matching registrations are untouched.
    assert!(router.get("builtin_a").is_some());
    assert!(router.get("builtin_b").is_some());
    assert!(router.get("plugin_x").is_none());
    assert!(router.get("plugin_y").is_none());
    assert!(router.capability_of("plugin_x").is_none());
}

#[test]
fn re_registration_keeps_its_position_for_bulk_dispose() {
    let router = ToolRouter::new();
    let log: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    router.register(Box::new(OrderedDisposeTool {
        name: "plugin_first",
        namespace: "plugin",
        log: Arc::clone(&log),
    }));
    router.register(Box::new(OrderedDisposeTool {
        name: "plugin_second",
        namespace: "plugin",
        log: Arc::clone(&log),
    }));
    // Replace `plugin_first` (registered first) with a fresh handler; the
    // replacement must inherit the original earliest position. The replace
    // itself disposes the v1 handler (logged), so clear the log to isolate
    // the bulk-dispose ordering under test.
    router.register(Box::new(OrderedDisposeTool {
        name: "plugin_first",
        namespace: "plugin",
        log: Arc::clone(&log),
    }));
    assert_eq!(
        log.lock().expect("dispose log lock poisoned").as_slice(),
        ["plugin_first"],
        "the replace disposed the v1 handler"
    );
    log.lock().expect("dispose log lock poisoned").clear();

    let removed = router.unregister_where(|_name, cap| cap.namespace.as_str() == "plugin");
    assert_eq!(removed, ["plugin_first", "plugin_second"]);
    // The replacement inherited the original (earliest) position, so it is
    // disposed LAST even though it was registered most recently. (Had the
    // replacement moved to the end, the log would be reversed.)
    assert_eq!(*log.lock().expect("dispose log lock poisoned"), ["plugin_second", "plugin_first"]);
}
