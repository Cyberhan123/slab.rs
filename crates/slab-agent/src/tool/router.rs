//! The tool router registry.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use crate::port::ToolSpec;

use super::capability::{ToolCapability, ToolVisibility};
use super::handler::ToolHandler;

/// Registry of available tools for a given agent thread.
///
/// Acts as both the dispatch table (name → [`ToolHandler`]) and the source of
/// the model-facing spec projection. It caches each tool's static
/// [`ToolCapability`] (category / visibility / namespace / risk) at
/// registration so the per-turn projection never re-queries handlers. A future
/// refactor may physically split dispatch (`ToolRegistry`) from projection
/// (`ToolSpecProvider`); until then both live here behind a stable facade.
#[derive(Clone)]
pub struct ToolRouter {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
    capabilities: Arc<RwLock<HashMap<String, ToolCapability>>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool handler.  A handler with the same name replaces any
    /// previously registered handler (the replaced handler is disposed — see
    /// [`ToolHandler::dispose`]). Its [`ToolCapability`] is cached once so
    /// the projection need not re-query the handler each turn.
    pub fn register(&self, handler: Box<dyn ToolHandler>) {
        let handler: Arc<dyn ToolHandler> = handler.into();
        let name = handler.name().to_owned();
        let capability = handler.capability();
        let replaced = {
            let mut handlers = self.handlers.write().expect("tool registry lock poisoned");
            handlers.insert(name.clone(), handler)
        };
        self.capabilities.write().expect("tool registry lock poisoned").insert(name, capability);
        // Dispose the replaced handler OUTSIDE the registry locks: a dispose
        // that re-entered the registry would deadlock on the write locks.
        if let Some(old) = replaced {
            old.dispose();
        }
    }

    /// Remove a registered tool handler by name. The removed handler is
    /// disposed outside the registry locks; the returned `Arc` stays
    /// memory-safe but the handler's resources are released.
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.capabilities.write().expect("tool registry lock poisoned").remove(name);
        let removed = self.handlers.write().expect("tool registry lock poisoned").remove(name);
        if let Some(handler) = &removed {
            handler.dispose();
        }
        removed
    }

    /// Look up a handler by tool name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.read().expect("tool registry lock poisoned").get(name).cloned()
    }

    /// Return [`ToolSpec`] descriptors for all registered tools (regardless of
    /// visibility/exposure). Use [`Self::visible_tool_specs`] for the
    /// model-facing projection.
    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.handlers
            .read()
            .expect("tool registry lock poisoned")
            .values()
            .map(|h| ToolSpec {
                name: h.name().to_owned(),
                description: h.description().to_owned(),
                parameters_schema: h.parameters_schema(),
            })
            .collect()
    }

    /// The cached [`ToolCapability`] for a registered tool, if any.
    pub fn capability_of(&self, name: &str) -> Option<ToolCapability> {
        self.capabilities.read().expect("tool registry lock poisoned").get(name).cloned()
    }

    /// Map every registered tool name to its exposure category. Used by the
    /// turn loop to filter the tool list by the current permission behavior
    /// without leaking categories onto the LLM-facing [`ToolSpec`].
    pub fn categories(&self) -> HashMap<String, slab_exec_policy::OperationCategory> {
        self.capabilities
            .read()
            .expect("tool registry lock poisoned")
            .iter()
            .map(|(name, cap)| (name.clone(), cap.category))
            .collect()
    }

    /// Project the model-facing tool list for a turn: applies both the
    /// per-tool [`ToolVisibility`] tri-state and the category-based
    /// `exposure` filter (driven by permission/interaction mode).
    ///
    /// - [`ToolVisibility::Direct`]   → included iff its category is exposed.
    /// - [`ToolVisibility::Deferred`]  → included iff its name is in
    ///   `injected_deferred` (populated by `tool_search`) AND its category is
    ///   exposed. Keeps plugin/MCP tools out of the base list until discovered.
    /// - [`ToolVisibility::Hidden`]    → never included, but still dispatchable
    ///   via [`Self::get`].
    ///
    /// Under `ToolExposure::all()` (FullControl / RequestApproval) every
    /// category passes, so the result is the visibility filter alone.
    pub fn visible_tool_specs(
        &self,
        exposure: slab_exec_policy::ToolExposure,
        injected_deferred: &HashSet<String>,
    ) -> Vec<ToolSpec> {
        let handlers = self.handlers.read().expect("tool registry lock poisoned");
        let caps = self.capabilities.read().expect("tool registry lock poisoned");
        let all_exposed = exposure == slab_exec_policy::ToolExposure::all();
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let visibility =
                    caps.get(name).map(|cap| cap.visibility).unwrap_or(ToolVisibility::Direct);
                let category = caps
                    .get(name)
                    .map(|cap| cap.category)
                    .unwrap_or(slab_exec_policy::OperationCategory::ReadOnly);
                let exposed = all_exposed || exposure.contains(category);
                let include = match visibility {
                    ToolVisibility::Hidden => false,
                    ToolVisibility::Deferred => injected_deferred.contains(name) && exposed,
                    ToolVisibility::Direct => exposed,
                };
                include.then(|| ToolSpec {
                    name: handler.name().to_owned(),
                    description: handler.description().to_owned(),
                    parameters_schema: handler.parameters_schema(),
                })
            })
            .collect()
    }

    /// Specs for every registered [`ToolVisibility::Deferred`] tool, regardless
    /// of category exposure. These are the candidates `tool_search` matches
    /// against; whether a hit becomes callable still depends on exposure (see
    /// [`Self::visible_tool_specs`]).
    pub fn deferred_tool_specs(&self) -> Vec<ToolSpec> {
        let handlers = self.handlers.read().expect("tool registry lock poisoned");
        let caps = self.capabilities.read().expect("tool registry lock poisoned");
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let visibility =
                    caps.get(name).map(|cap| cap.visibility).unwrap_or(ToolVisibility::Direct);
                (visibility == ToolVisibility::Deferred).then(|| ToolSpec {
                    name: handler.name().to_owned(),
                    description: handler.description().to_owned(),
                    parameters_schema: handler.parameters_schema(),
                })
            })
            .collect()
    }
}

/// Per-thread state tracking which `Deferred` tools `tool_search` has injected
/// for the current thread. Lives on the thread runtime (not the process-wide
/// [`ToolRouter`]) so discovery is isolated per thread and cleaned up when the
/// thread ends — no manual `clear` needed.
#[derive(Debug, Default)]
pub struct ToolDiscoveryState {
    injected: std::sync::Mutex<HashSet<String>>,
}

impl ToolDiscoveryState {
    /// Create an empty discovery state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a Deferred tool as injected (visible/callable for subsequent turns
    /// of this thread, subject to category exposure).
    pub fn inject(&self, name: &str) {
        self.injected.lock().expect("tool discovery state lock poisoned").insert(name.to_owned());
    }

    /// Snapshot the set of injected tool names (wire form), for the per-turn
    /// [`ToolRouter::visible_tool_specs`] projection.
    pub fn snapshot(&self) -> HashSet<String> {
        self.injected.lock().expect("tool discovery state lock poisoned").clone()
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::new()
    }
}
