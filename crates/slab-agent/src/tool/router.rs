//! The tool router registry.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use crate::port::ToolSpec;

use super::capability::{ToolCapability, ToolVisibility};
use super::dep::ToolServiceKey;
use super::handler::ToolHandler;

/// Cached registration metadata: the static capability plus the tool's
/// declared host-service dependencies, both captured once at registration so
/// the per-turn projections never re-query handlers.
#[derive(Clone)]
struct Entry {
    capability: ToolCapability,
    service_deps: Vec<ToolServiceKey>,
}

impl Entry {
    /// Whether every declared service dep is currently satisfied (an empty
    /// declaration is always satisfied).
    fn active(&self, satisfied: &HashSet<ToolServiceKey>) -> bool {
        self.service_deps.iter().all(|key| satisfied.contains(key))
    }
}

/// Registry of available tools for a given agent thread.
///
/// Acts as both the dispatch table (name → [`ToolHandler`]) and the source of
/// the model-facing spec projection. It caches each tool's static
/// [`ToolCapability`] (category / visibility / namespace / risk) at
/// registration so the per-turn projection never re-queries handlers, and
/// tracks registration order so bulk removals ([`ToolRouter::unregister_where`])
/// can tear handlers down in reverse. A future refactor may physically split
/// dispatch (`ToolRegistry`) from projection (`ToolSpecProvider`); until then
/// both live here behind a stable facade.
#[derive(Clone)]
pub struct ToolRouter {
    handlers: Arc<RwLock<HashMap<String, Arc<dyn ToolHandler>>>>,
    entries: Arc<RwLock<HashMap<String, Entry>>>,
    /// Registration order (first-registration position survives re-registration)
    /// so bulk disposal can run newest-first.
    order: Arc<RwLock<Vec<String>>>,
    /// Host-service keys currently marked satisfied (see
    /// [`ToolRouter::set_dep_satisfied`]). Tools whose declared deps are not
    /// all in here are filtered from the model-facing projections (PENDING)
    /// but stay dispatchable.
    satisfied: Arc<RwLock<HashSet<ToolServiceKey>>>,
}

impl ToolRouter {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            entries: Arc::new(RwLock::new(HashMap::new())),
            order: Arc::new(RwLock::new(Vec::new())),
            satisfied: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Register a tool handler.  A handler with the same name replaces any
    /// previously registered handler (the replaced handler is disposed — see
    /// [`ToolHandler::dispose`]). Its [`ToolCapability`] is cached once so
    /// the projection need not re-query the handler each turn.
    ///
    /// Returns a [`ToolRegistration`] handle. Dropping the handle does NOT
    /// unregister (the common fire-and-forget registrations just ignore it);
    /// scoped registrations call [`ToolRegistration::dispose`] explicitly.
    pub fn register(&self, handler: Box<dyn ToolHandler>) -> ToolRegistration {
        let handler: Arc<dyn ToolHandler> = handler.into();
        let name = handler.name().to_owned();
        let entry =
            Entry { capability: handler.capability(), service_deps: handler.service_deps() };
        let replaced = {
            let mut handlers = self.handlers.write().expect("tool registry lock poisoned");
            handlers.insert(name.clone(), handler)
        };
        self.entries.write().expect("tool registry lock poisoned").insert(name.clone(), entry);
        {
            // A re-registration keeps its original position; only new names
            // append to the order.
            let mut order = self.order.write().expect("tool registration order lock poisoned");
            if replaced.is_none() && !order.contains(&name) {
                order.push(name.clone());
            }
        }
        // Dispose the replaced handler OUTSIDE the registry locks: a dispose
        // that re-entered the registry would deadlock on the write locks.
        if let Some(old) = replaced {
            old.dispose();
        }
        ToolRegistration { router: self.clone(), name }
    }

    /// Remove a registered tool handler by name. The removed handler is
    /// disposed outside the registry locks; the returned `Arc` stays
    /// memory-safe but the handler's resources are released.
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.entries.write().expect("tool registry lock poisoned").remove(name);
        let removed = self.handlers.write().expect("tool registry lock poisoned").remove(name);
        self.order
            .write()
            .expect("tool registration order lock poisoned")
            .retain(|registered| registered != name);
        if let Some(handler) = &removed {
            handler.dispose();
        }
        removed
    }

    /// Remove every registration whose cached [`ToolCapability`] matches
    /// `pred`, disposing each removed handler in REVERSE registration order
    /// (registrations made later are torn down first). Returns the removed
    /// tool names in registration order.
    ///
    /// `pred` runs under the capability read lock — keep it cheap and pure.
    pub fn unregister_where(
        &self,
        mut pred: impl FnMut(&str, &ToolCapability) -> bool,
    ) -> Vec<String> {
        let matched: HashSet<String> = {
            let entries = self.entries.read().expect("tool registry lock poisoned");
            entries
                .iter()
                .filter(|(name, entry)| pred(name, &entry.capability))
                .map(|(name, _)| name.clone())
                .collect()
        };
        if matched.is_empty() {
            return Vec::new();
        }
        let mut removed: Vec<Arc<dyn ToolHandler>> = Vec::new();
        let removed_names: Vec<String> = {
            let mut handlers = self.handlers.write().expect("tool registry lock poisoned");
            let mut entries = self.entries.write().expect("tool registry lock poisoned");
            let mut order = self.order.write().expect("tool registration order lock poisoned");
            let mut removed_names = Vec::new();
            order.retain(|name| {
                if !matched.contains(name) {
                    return true;
                }
                entries.remove(name);
                match handlers.remove(name) {
                    Some(handler) => {
                        removed_names.push(name.clone());
                        removed.push(handler);
                        false
                    }
                    None => true,
                }
            });
            removed_names
        };
        // `order.retain` walks in registration order, so `removed` is too;
        // tear down in reverse so later registrations go first.
        for handler in removed.iter().rev() {
            handler.dispose();
        }
        removed_names
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
        self.entries
            .read()
            .expect("tool registry lock poisoned")
            .get(name)
            .map(|entry| entry.capability.clone())
    }

    /// Map every registered tool name to its exposure category. Used by the
    /// turn loop to filter the tool list by the current permission behavior
    /// without leaking categories onto the LLM-facing [`ToolSpec`].
    pub fn categories(&self) -> HashMap<String, slab_exec_policy::OperationCategory> {
        self.entries
            .read()
            .expect("tool registry lock poisoned")
            .iter()
            .map(|(name, entry)| (name.clone(), entry.capability.category))
            .collect()
    }

    /// Mark a host-service key satisfied (or not). Tools whose declared
    /// [`ToolHandler::service_deps`] include the key re-enter the
    /// model-facing projections on the next turn (PENDING → active) — or are
    /// filtered out again when it flips back to unsatisfied. Dispatchability
    /// via [`Self::get`] is unchanged.
    pub fn set_dep_satisfied(&self, key: ToolServiceKey, satisfied: bool) {
        let mut set = self.satisfied.write().expect("tool service-dep lock poisoned");
        if satisfied {
            set.insert(key);
        } else {
            set.remove(&key);
        }
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
        let entries = self.entries.read().expect("tool registry lock poisoned");
        let satisfied = self.satisfied.read().expect("tool service-dep lock poisoned");
        let all_exposed = exposure == slab_exec_policy::ToolExposure::all();
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let entry = entries.get(name)?;
                // PENDING tools (unsatisfied service deps) stay out of the
                // projections; they remain dispatchable via `get`.
                if !entry.active(&satisfied) {
                    return None;
                }
                let visibility = entry.capability.visibility;
                let category = entry.capability.category;
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
        let entries = self.entries.read().expect("tool registry lock poisoned");
        let satisfied = self.satisfied.read().expect("tool service-dep lock poisoned");
        handlers
            .values()
            .filter_map(|handler| {
                let name = handler.name();
                let entry = entries.get(name)?;
                let is_deferred = entry.capability.visibility == ToolVisibility::Deferred
                    && entry.active(&satisfied);
                is_deferred.then(|| ToolSpec {
                    name: handler.name().to_owned(),
                    description: handler.description().to_owned(),
                    parameters_schema: handler.parameters_schema(),
                })
            })
            .collect()
    }
}

/// Owned handle to a [`ToolRouter`] registration, returned by
/// [`ToolRouter::register`].
///
/// `dispose()` unregisters the tool and runs its [`ToolHandler::dispose`].
/// Dropping the handle does NOT unregister — fire-and-forget registrations
/// (the common case) simply ignore the return value, and scoped registrations
/// (plugin proxies, workspace-bound tools) close over the handle and dispose
/// it explicitly when their owner goes away.
pub struct ToolRegistration {
    router: ToolRouter,
    name: String,
}

impl ToolRegistration {
    /// Unregister the tool and dispose its handler.
    pub fn dispose(self) {
        self.router.unregister(&self.name);
    }

    /// The registered tool name.
    pub fn name(&self) -> &str {
        &self.name
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
