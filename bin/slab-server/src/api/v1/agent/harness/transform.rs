//! Cross-cutting handler transformers for the harness method router.
//!
//! [`thread_op`] resolves the harness thread id → real slab thread id and
//! injects it into the body, so the "operate on an existing thread" handlers
//! can't forget binding resolution.
//!
//! [`establish_op`] wraps a body that produces a freshly-established thread
//! ([`Established`]) and runs the post-await side effects (`bind` +
//! `spawn_event_fanout`) centrally, so `thread/fork` and `thread/resume` share
//! one establishment path.
//!
//! Both produce values satisfying [`slab_jsonrpc::router::ErasedHandler`],
//! registered via `Router::on` / `Router::on_erased`.

use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use slab_jsonrpc::router::ErasedHandler;
use slab_proto::harness::messages::{
    ApprovalResolveParams, ShutdownParams, ThreadArchiveParams, ThreadCompactStartParams,
    ThreadRollbackParams, TurnInterruptParams,
};

use super::session::HarnessSession;

/// Params carrying a harness `thread_id` to resolve against the binding table.
pub(crate) trait ThreadReferenced {
    fn thread_id(&self) -> &str;
}

impl ThreadReferenced for TurnInterruptParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}
impl ThreadReferenced for ApprovalResolveParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}
impl ThreadReferenced for ShutdownParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}
impl ThreadReferenced for ThreadArchiveParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}
impl ThreadReferenced for ThreadRollbackParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}
impl ThreadReferenced for ThreadCompactStartParams {
    fn thread_id(&self) -> &str {
        &self.thread_id
    }
}

/// Wrap a body `Fn(HarnessSession, real_id: String, P)` so the caller writes
/// only the operate-on-existing-thread logic; binding resolution happens here.
pub(crate) fn thread_op<B, P, Fut>(
    body: B,
) -> impl Fn(HarnessSession, P) -> Fut + Send + Sync + 'static
where
    B: Fn(HarnessSession, String, P) -> Fut + Send + Sync + 'static,
    P: ThreadReferenced + Send + 'static,
    Fut: std::future::Future + Send + 'static,
{
    move |session: HarnessSession, params: P| {
        let real_id = session.real_id_for(params.thread_id());
        body(session, real_id, params)
    }
}

/// Outcome of an `establish_op` body: a newly-created/replayed thread plus the
/// typed wire result. The adapter binds it and starts the fan-out.
pub(crate) struct Established<R> {
    pub(crate) harness_id: String,
    pub(crate) real_id: String,
    pub(crate) result: R,
}

/// Adapter that runs an establish body, then binds + fans out centrally.
pub(crate) struct EstablishAdapter<B, P, R, Fut> {
    body: B,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn(HarnessSession, P) -> (R, Fut)>,
}

#[async_trait::async_trait]
impl<B, P, R, Fut> ErasedHandler<HarnessSession> for EstablishAdapter<B, P, R, Fut>
where
    B: Fn(HarnessSession, P) -> Fut + Send + Sync + 'static,
    P: DeserializeOwned + Send + 'static,
    R: Serialize + Send + 'static,
    Fut: std::future::Future<Output = Result<Established<R>, String>> + Send + 'static,
{
    async fn handle(&self, session: HarnessSession, params: Value) -> Result<Value, String> {
        let parsed: P =
            serde_json::from_value(params).map_err(|e| format!("invalid params: {e}"))?;
        let established = (self.body)(session.clone(), parsed).await?;
        session.bind(&established.harness_id, established.real_id.clone());
        session.spawn_event_fanout(established.real_id, established.harness_id);
        serde_json::to_value(established.result).map_err(|e| e.to_string())
    }
}

/// Wrap an establish body so `bind` + `spawn_event_fanout` run centrally after
/// it produces an [`Established`]. Returns a custom [`ErasedHandler`] (the side
/// effects can't be expressed as a plain `Fn` returning a single future).
pub(crate) fn establish_op<B, P, R, Fut>(body: B) -> EstablishAdapter<B, P, R, Fut>
where
    B: Fn(HarnessSession, P) -> Fut + Send + Sync + 'static,
    P: DeserializeOwned + Send + 'static,
    R: Serialize + Send + 'static,
    Fut: std::future::Future<Output = Result<Established<R>, String>> + Send + 'static,
{
    EstablishAdapter { body, _phantom: PhantomData }
}
