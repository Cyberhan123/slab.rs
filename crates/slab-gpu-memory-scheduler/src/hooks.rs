//! Model lifecycle hooks — a ToolHandler-style registry (name-keyed trait
//! objects with default no-op methods). Hooks are advisory: dispatch collects
//! per-hook failures as warnings and never fails the load/unload/inference
//! path. Hosts fire them at model boundaries; the runtime process never does
//! (cross-process participation is via the sizing functions and wire-reported
//! resolved values).

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use slab_types::RuntimeBackendId;
use tracing::warn;

use crate::error::GpuMemoryError;

/// Everything the host knows when a model load is dispatched.
#[derive(Debug, Clone)]
pub struct LoadContext {
    pub backend: RuntimeBackendId,
    pub model_id: Option<String>,
    pub model_path: String,
    pub num_workers: usize,
    /// Requested fixed context; `None` = `auto` (resolved by the runtime).
    pub requested_context: Option<u32>,
    pub mmproj_path: Option<String>,
    /// Free VRAM sampled at dispatch time (scheduler-provided).
    pub free_vram_bytes: Option<u64>,
}

/// What the runtime reported once the load finished.
#[derive(Debug, Clone, Default)]
pub struct LoadOutcome {
    /// Resolved engine `n_ctx` — the value `auto` sized to.
    pub resolved_context_length: Option<u32>,
    pub training_context_length: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnloadReason {
    IdleTimeout,
    MemoryPressure,
    Manual,
    /// The runtime process restarted out from under a resident model.
    RuntimeRestart,
}

#[derive(Debug, Clone)]
pub struct UnloadContext {
    pub backend: RuntimeBackendId,
    pub reason: UnloadReason,
}

#[derive(Debug, Clone, Copy)]
pub struct InferenceContext {
    pub backend: RuntimeBackendId,
}

/// Advisory observer over the model lifecycle. Implement only the events you
/// care about; register with [`HookRegistry::register`] under `name()`
/// (re-registering a name replaces the hook, mirroring the tool router).
#[async_trait]
pub trait ModelLifecycleHook: Send + Sync {
    fn name(&self) -> &str;

    async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryError> {
        Ok(())
    }

    async fn after_load(
        &self,
        _ctx: &LoadContext,
        _outcome: &LoadOutcome,
    ) -> Result<(), GpuMemoryError> {
        Ok(())
    }

    async fn before_unload(&self, _ctx: &UnloadContext) -> Result<(), GpuMemoryError> {
        Ok(())
    }

    async fn after_unload(&self, _ctx: &UnloadContext) -> Result<(), GpuMemoryError> {
        Ok(())
    }

    async fn before_inference(&self, _ctx: &InferenceContext) -> Result<(), GpuMemoryError> {
        Ok(())
    }

    async fn after_inference(&self, _ctx: &InferenceContext) -> Result<(), GpuMemoryError> {
        Ok(())
    }
}

/// Name-keyed hook registry. The map is the source of truth; a cached
/// `Arc<[..]>` snapshot is rebuilt on register/unregister so each dispatch
/// pays one refcount bump instead of a `Vec` + N `Arc` clones (the inference
/// events sit on the hot path), and no lock is held across hook awaits — a
/// hook may register/unregister mid-dispatch without deadlocking, and the
/// in-flight dispatch finishes against its registration-time view.
/// Dispatch is sequential over registered hooks; one failing hook warns and
/// does not block the others.
pub struct HookRegistry {
    hooks: RwLock<HashMap<String, Arc<dyn ModelLifecycleHook>>>,
    snapshot: RwLock<Arc<[Arc<dyn ModelLifecycleHook>]>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self { hooks: RwLock::new(HashMap::new()), snapshot: RwLock::new(Vec::new().into()) }
    }
}

impl HookRegistry {
    pub fn register(&self, hook: Arc<dyn ModelLifecycleHook>) {
        let name = hook.name().to_owned();
        self.hooks.write().expect("hook registry lock poisoned").insert(name, hook);
        self.rebuild_snapshot();
    }

    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ModelLifecycleHook>> {
        let removed = self.hooks.write().expect("hook registry lock poisoned").remove(name);
        if removed.is_some() {
            self.rebuild_snapshot();
        }
        removed
    }

    pub fn names(&self) -> Vec<String> {
        self.hooks.read().expect("hook registry lock poisoned").keys().cloned().collect()
    }

    fn rebuild_snapshot(&self) {
        let snapshot = {
            let hooks = self.hooks.read().expect("hook registry lock poisoned");
            hooks.values().cloned().collect::<Vec<_>>()
        };
        *self.snapshot.write().expect("hook registry lock poisoned") = snapshot.into();
    }

    /// Clone of the cached dispatch list. The read guard is released at the
    /// end of this statement (block-scope idiom; an explicit `drop` does not
    /// shorten async-fn captures), so callers never hold it across awaits.
    fn hooks_snapshot(&self) -> Arc<[Arc<dyn ModelLifecycleHook>]> {
        Arc::clone(&self.snapshot.read().expect("hook registry lock poisoned"))
    }

    pub async fn dispatch_before_load(&self, ctx: &LoadContext) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.before_load(ctx).await {
                warn!(hook = hook.name(), %error, event = "before_load", "model lifecycle hook failed");
            }
        }
    }

    pub async fn dispatch_after_load(&self, ctx: &LoadContext, outcome: &LoadOutcome) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.after_load(ctx, outcome).await {
                warn!(hook = hook.name(), %error, event = "after_load", "model lifecycle hook failed");
            }
        }
    }

    pub async fn dispatch_before_unload(&self, ctx: &UnloadContext) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.before_unload(ctx).await {
                warn!(hook = hook.name(), %error, event = "before_unload", "model lifecycle hook failed");
            }
        }
    }

    pub async fn dispatch_after_unload(&self, ctx: &UnloadContext) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.after_unload(ctx).await {
                warn!(hook = hook.name(), %error, event = "after_unload", "model lifecycle hook failed");
            }
        }
    }

    pub async fn dispatch_before_inference(&self, ctx: &InferenceContext) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.before_inference(ctx).await {
                warn!(hook = hook.name(), %error, event = "before_inference", "model lifecycle hook failed");
            }
        }
    }

    pub async fn dispatch_after_inference(&self, ctx: &InferenceContext) {
        let hooks = self.hooks_snapshot();
        if hooks.is_empty() {
            return;
        }
        for hook in hooks.iter() {
            if let Err(error) = hook.after_inference(ctx).await {
                warn!(hook = hook.name(), %error, event = "after_inference", "model lifecycle hook failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHook {
        name: &'static str,
        loads: AtomicUsize,
    }

    #[async_trait]
    impl ModelLifecycleHook for CountingHook {
        fn name(&self) -> &str {
            self.name
        }

        async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailingHook;

    #[async_trait]
    impl ModelLifecycleHook for FailingHook {
        fn name(&self) -> &str {
            "failing"
        }

        async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryError> {
            Err(GpuMemoryError::WorkerPanic)
        }
    }

    fn load_ctx() -> LoadContext {
        LoadContext {
            backend: RuntimeBackendId::GgmlLlama,
            model_id: None,
            model_path: String::new(),
            num_workers: 1,
            requested_context: None,
            mmproj_path: None,
            free_vram_bytes: None,
        }
    }

    #[tokio::test]
    async fn register_replaces_by_name_and_unregister_removes() {
        let registry = HookRegistry::default();
        registry.register(Arc::new(CountingHook { name: "counter", loads: AtomicUsize::new(0) }));
        assert_eq!(registry.names(), vec!["counter".to_owned()]);

        // Re-registering the same name replaces (single entry, counter reset).
        let replacement = Arc::new(CountingHook { name: "counter", loads: AtomicUsize::new(5) });
        registry.register(replacement.clone());
        assert_eq!(registry.names().len(), 1);

        registry.dispatch_before_load(&load_ctx()).await;
        assert_eq!(replacement.loads.load(Ordering::SeqCst), 6);

        assert!(registry.unregister("counter").is_some());
        assert!(registry.names().is_empty());
    }

    #[tokio::test]
    async fn failing_hook_does_not_block_other_hooks() {
        let registry = HookRegistry::default();
        let counter = Arc::new(CountingHook { name: "counter", loads: AtomicUsize::new(0) });
        registry.register(Arc::new(FailingHook));
        registry.register(counter.clone());

        registry.dispatch_before_load(&load_ctx()).await;
        assert_eq!(counter.loads.load(Ordering::SeqCst), 1, "other hooks still ran");
    }

    #[tokio::test]
    async fn default_hook_methods_are_noops() {
        struct BareHook;
        #[async_trait]
        impl ModelLifecycleHook for BareHook {
            fn name(&self) -> &str {
                "bare"
            }
        }

        let registry = HookRegistry::default();
        registry.register(Arc::new(BareHook));
        let ctx =
            UnloadContext { backend: RuntimeBackendId::GgmlWhisper, reason: UnloadReason::Manual };
        registry.dispatch_before_unload(&ctx).await;
        registry.dispatch_after_unload(&ctx).await;
        registry.dispatch_before_inference(&InferenceContext { backend: ctx.backend }).await;
        registry.dispatch_after_inference(&InferenceContext { backend: ctx.backend }).await;
        registry.dispatch_after_load(&load_ctx(), &LoadOutcome::default()).await;
    }

    #[tokio::test]
    async fn register_during_dispatch_takes_effect_next_dispatch() {
        /// Registers a counting hook against the shared registry from inside
        /// `before_load`. The dispatch loop must not hold its snapshot lock
        /// across the await (that would deadlock against the rebuild's write)
        /// and the newly registered hook must only fire on the next dispatch.
        struct SharedCountHook {
            name: &'static str,
            loads: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl ModelLifecycleHook for SharedCountHook {
            fn name(&self) -> &str {
                self.name
            }

            async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryError> {
                self.loads.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        struct RegisteringHook {
            registry: RwLock<Option<std::sync::Weak<HookRegistry>>>,
            late_loads: Arc<AtomicUsize>,
            loads: AtomicUsize,
        }

        #[async_trait]
        impl ModelLifecycleHook for RegisteringHook {
            fn name(&self) -> &str {
                "registering"
            }

            async fn before_load(&self, _ctx: &LoadContext) -> Result<(), GpuMemoryError> {
                self.loads.fetch_add(1, Ordering::SeqCst);
                if let Some(registry) = self
                    .registry
                    .read()
                    .unwrap()
                    .clone()
                    .and_then(|weak| std::sync::Weak::upgrade(&weak))
                {
                    registry.register(Arc::new(SharedCountHook {
                        name: "late",
                        loads: Arc::clone(&self.late_loads),
                    }));
                }
                Ok(())
            }
        }

        let registry = Arc::new(HookRegistry::default());
        let registering = Arc::new(RegisteringHook {
            registry: RwLock::new(None),
            late_loads: Arc::new(AtomicUsize::new(0)),
            loads: AtomicUsize::new(0),
        });
        registry.register(registering.clone());
        *registering.registry.write().unwrap() = Some(Arc::downgrade(&registry));

        // First dispatch: the mid-dispatch registration lands only in the
        // map — the in-flight dispatch finishes against its snapshot.
        registry.dispatch_before_load(&load_ctx()).await;
        assert_eq!(registering.loads.load(Ordering::SeqCst), 1);
        assert_eq!(registering.late_loads.load(Ordering::SeqCst), 0, "not yet dispatched");
        assert!(registry.names().contains(&"late".to_owned()), "but already registered");

        // Second dispatch sees the rebuilt snapshot.
        registry.dispatch_before_load(&load_ctx()).await;
        assert_eq!(registering.loads.load(Ordering::SeqCst), 2);
        assert_eq!(registering.late_loads.load(Ordering::SeqCst), 1, "fires on next dispatch");
    }

    #[tokio::test]
    async fn empty_registry_dispatch_completes_without_hooks() {
        let registry = HookRegistry::default();
        assert!(registry.names().is_empty());

        // Exercises the empty fast path on all six dispatch methods.
        registry.dispatch_before_load(&load_ctx()).await;
        registry.dispatch_after_load(&load_ctx(), &LoadOutcome::default()).await;
        let ctx =
            UnloadContext { backend: RuntimeBackendId::GgmlLlama, reason: UnloadReason::Manual };
        registry.dispatch_before_unload(&ctx).await;
        registry.dispatch_after_unload(&ctx).await;
        registry.dispatch_before_inference(&InferenceContext { backend: ctx.backend }).await;
        registry.dispatch_after_inference(&InferenceContext { backend: ctx.backend }).await;
    }
}
