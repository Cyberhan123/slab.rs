//! Method-keyed dispatch for typed JSON-RPC request handlers.
//!
//! [`Router<C>`] stores type-erased handlers keyed by method string and
//! dispatches inbound requests to them. Each handler is a typed function
//! `Fn(C, P) -> Future<Output = Result<R, E>>`; [`HandlerWrapper`] decodes
//! `Value → P`, calls the function, encodes `R → Value`, and converts `E`
//! into the `String` error that [`crate::ws::serve_websocket`] turns into a
//! `-32000` JSON-RPC error response.
//!
//! ## Context is passed by value
//!
//! `C` (the context) is passed **by value**, never by reference, and must be
//! `Clone` (typically `Arc<Session>`). This is a hard constraint: handlers run
//! inside a spawned task in [`crate::ws::serve_websocket`], so their future
//! must be `Send + 'static`. A context borrowed for `'a` would encode `'a`
//! into the future and break that bound. The per-request `Arc::clone` is the
//! price, and it is negligible.
//!
//! ## Why `HandlerWrapper` (not a bare blanket impl)
//!
//! A blanket `impl ErasedHandler<C> for F where F: Fn(C, P) -> Fut` is rejected
//! (E0207): `P`/`R`/`E`/`Fut` would be unconstrained because a single `F` may
//! implement `Fn(C, P1)` and `Fn(C, P2)` for different `P`. Wrapping the
//! function in a concrete `HandlerWrapper<C, P, R, E, Fut, F>` moves those
//! parameters into the impl head, where they are constrained. Callers never
//! name `HandlerWrapper` — [`Router::on`] infers it.

// Six type parameters on `HandlerWrapper` are inherent to type-erased handler
// dispatch (mirrors axum's approach). The complexity is by design.
#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// A type-erased request handler for one method.
///
/// `ctx` is taken by value (see module docs). Custom adapters with post-await
/// side effects (e.g. harness `establish_op`) may `impl ErasedHandler` directly
/// and register via [`Router::on_erased`]; ordinary typed functions go through
/// [`Router::on`] and are wrapped in [`HandlerWrapper`].
#[async_trait::async_trait]
pub trait ErasedHandler<C: Send + Sync + 'static>: Send + Sync + 'static {
    async fn handle(&self, ctx: C, params: Value) -> Result<Value, String>;
}

/// Wraps a typed `Fn(C, P) -> Fut<Result<R, E>>` as an [`ErasedHandler<C>`].
/// The `PhantomData` carries the parameter/result types so the impl head names
/// every type parameter (satisfying coherence).
struct HandlerWrapper<C, P, R, E, Fut, F> {
    f: F,
    _marker: PhantomData<fn(C, P) -> (R, E, Fut)>,
}

#[async_trait::async_trait]
#[allow(clippy::type_complexity)] // 6 type params are inherent to the type-erasure design.
impl<C, P, R, E, Fut, F> ErasedHandler<C> for HandlerWrapper<C, P, R, E, Fut, F>
where
    C: Send + Sync + Clone + 'static,
    F: Fn(C, P) -> Fut + Send + Sync + 'static,
    P: DeserializeOwned + Send + 'static,
    R: Serialize + Send + 'static,
    E: std::fmt::Display + Send + 'static,
    Fut: std::future::Future<Output = Result<R, E>> + Send + 'static,
{
    async fn handle(&self, ctx: C, params: Value) -> Result<Value, String> {
        let parsed: P =
            serde_json::from_value(params).map_err(|e| format!("invalid params: {e}"))?;
        let result = (self.f)(ctx, parsed).await.map_err(|e| e.to_string())?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    }
}

/// Method → handler registry. Dispatch is a lookup + [`ErasedHandler::handle`].
///
/// A host that wants to plug into [`crate::ws::serve_websocket`] holds a
/// `Router` alongside an `Arc<Ctx>` and forwards
/// `RequestHandler::handle_request` to [`Router::dispatch`].
pub struct Router<C: Send + Sync + 'static> {
    handlers: HashMap<&'static str, Arc<dyn ErasedHandler<C>>>,
}

impl<C: Send + Sync + Clone + 'static> Router<C> {
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    /// Register a typed handler for `method`. `handler` may be a plain
    /// `async fn(ctx, params) -> Result<...>`, a closure, or the result of a
    /// transformer that returns a `Fn` — anything matching
    /// `Fn(C, P) -> Fut<Result<R, E>>`.
    pub fn on<F, P, R, E, Fut>(&mut self, method: &'static str, handler: F)
    where
        F: Fn(C, P) -> Fut + Send + Sync + 'static,
        P: DeserializeOwned + Send + 'static,
        R: Serialize + Send + 'static,
        E: std::fmt::Display + Send + 'static,
        Fut: std::future::Future<Output = Result<R, E>> + Send + 'static,
    {
        let wrapped = HandlerWrapper::<C, P, R, E, Fut, F> { f: handler, _marker: PhantomData };
        self.handlers.insert(method, Arc::new(wrapped));
    }

    /// Register a custom [`ErasedHandler`] (e.g. an adapter with post-await
    /// side effects that can't be expressed as a plain `Fn`).
    pub fn on_erased<H>(&mut self, method: &'static str, handler: H)
    where
        H: ErasedHandler<C> + 'static,
    {
        self.handlers.insert(method, Arc::new(handler));
    }

    /// Dispatch `method` / `params` to the registered handler, passing `ctx`
    /// by value. Unknown methods yield `Err("unknown method \`...\`")`;
    /// un-decodable params yield `Err("invalid params: ...")`.
    pub async fn dispatch(&self, ctx: C, method: &str, params: Value) -> Result<Value, String> {
        let handler =
            self.handlers.get(method).ok_or_else(|| format!("unknown method `{method}`"))?;
        handler.handle(ctx, params).await
    }
}

impl<C: Send + Sync + Clone + 'static> Default for Router<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Clone, Default)]
    struct Ctx;

    #[derive(Debug, Deserialize)]
    struct EchoParams {
        value: u32,
    }

    #[derive(Debug, Serialize)]
    struct EchoResult {
        doubled: u32,
    }

    async fn echo(_ctx: Ctx, params: EchoParams) -> Result<EchoResult, String> {
        Ok(EchoResult { doubled: params.value * 2 })
    }

    #[tokio::test]
    async fn dispatches_typed_handler_with_decode_and_encode() {
        let mut router: Router<Ctx> = Router::new();
        router.on("echo", echo);

        let result = router.dispatch(Ctx, "echo", json!({ "value": 21 })).await.unwrap();
        assert_eq!(result, json!({ "doubled": 42 }));
    }

    #[tokio::test]
    async fn closure_handler_via_on() {
        // Transformer-style: a closure returning an async block (this is the
        // shape `thread_op` produces in the harness layer).
        let mut router: Router<Ctx> = Router::new();
        router.on("closure", |_ctx: Ctx, params: EchoParams| async move {
            Ok::<EchoResult, String>(EchoResult { doubled: params.value + 1 })
        });
        let result = router.dispatch(Ctx, "closure", json!({ "value": 41 })).await.unwrap();
        assert_eq!(result, json!({ "doubled": 42 }));
    }

    #[tokio::test]
    async fn custom_erased_handler_via_on_erased() {
        // An adapter that impls ErasedHandler directly (the shape `establish_op`
        // produces when it needs post-await side effects).
        struct Doubler;
        #[async_trait::async_trait]
        impl ErasedHandler<Ctx> for Doubler {
            async fn handle(&self, _ctx: Ctx, params: Value) -> Result<Value, String> {
                let p: EchoParams =
                    serde_json::from_value(params).map_err(|e| format!("invalid params: {e}"))?;
                serde_json::to_value(EchoResult { doubled: p.value * 3 }).map_err(|e| e.to_string())
            }
        }
        let mut router: Router<Ctx> = Router::new();
        router.on_erased("triple", Doubler);
        let result = router.dispatch(Ctx, "triple", json!({ "value": 14 })).await.unwrap();
        assert_eq!(result, json!({ "doubled": 42 }));
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let router: Router<Ctx> = Router::new();
        let err = router.dispatch(Ctx, "missing", json!({})).await.unwrap_err();
        assert!(err.contains("unknown method `missing`"));
    }

    #[tokio::test]
    async fn invalid_params_returns_decode_error() {
        let mut router: Router<Ctx> = Router::new();
        router.on("echo", echo);
        let err =
            router.dispatch(Ctx, "echo", json!({ "value": "not-a-number" })).await.unwrap_err();
        assert!(err.starts_with("invalid params:"));
    }

    #[tokio::test]
    async fn handler_error_propagates_as_display_string() {
        async fn fail(_ctx: Ctx, _p: EchoParams) -> Result<EchoResult, String> {
            Err("boom".to_owned())
        }
        let mut router: Router<Ctx> = Router::new();
        router.on("fail", fail);
        let err = router.dispatch(Ctx, "fail", json!({ "value": 1 })).await.unwrap_err();
        assert_eq!(err, "boom");
    }
}
