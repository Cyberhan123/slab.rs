//! Harness request host — owns the per-connection [`HarnessSession`] and the
//! [`Router`] that dispatches typed method handlers.
//!
//! Implements [`RequestHandler`] so [`slab_jsonrpc::ws::serve_websocket`] can
//! drive it unchanged: `initialize` is handled inline (handshake gate), every
//! other method is delegated to the router. The `-32000` error code is applied
//! by `serve_websocket` to any `Err(String)` this returns.
//!
//! Registration maps each method to a transformer that absorbs cross-cutting
//! concerns (see [`super::transform`]):
//! - **default** typed handler: `thread/start`, `turn/start`, `thread/list`,
//!   `model/list`, `workspace/migrate`.
//! - **thread_op** (resolves binding, injects real_id): `turn/interrupt`,
//!   `approval/resolve`, `shutdown`, `thread/archive`, `thread/rollback`.
//! - **establish_op** (binds + fans out centrally): `thread/fork`,
//!   `thread/resume`.

use std::sync::Arc;

use serde_json::Value;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::HarnessService;
use slab_jsonrpc::host::RequestHandler;
use slab_jsonrpc::notifier::Notifier;
use slab_jsonrpc::router::Router;
use slab_proto::harness::messages::{InitializeResult, ServerCapabilities, ServerInfo};
use slab_proto::harness::method;

use super::body;
use super::session::HarnessSession;
use super::transform;

/// Per-connection harness host: session state + method router.
pub(crate) struct HarnessHost {
    session: HarnessSession,
    router: Router<HarnessSession>,
}

impl HarnessHost {
    pub(crate) fn new(
        session_id: String,
        state: Arc<AppState>,
        service: HarnessService,
        notifier: Notifier,
    ) -> Self {
        let session = HarnessSession::new(session_id, state, service, notifier);
        let mut router: Router<HarnessSession> = Router::new();
        // default typed handlers
        router.on(method::THREAD_START, body::thread_start);
        router.on(method::TURN_START, body::turn_start);
        router.on(method::THREAD_LIST, body::thread_list);
        router.on(method::MODEL_LIST, body::model_list);
        router.on(method::SKILLS_LIST, body::skills_list);
        router.on(method::WORKSPACE_MIGRATE, body::workspace_migrate);
        // thread_op: resolve binding, inject real_id
        router.on(method::TURN_INTERRUPT, transform::thread_op(body::turn_interrupt));
        router.on(method::APPROVAL_RESOLVE, transform::thread_op(body::approval_resolve));
        router.on(method::SHUTDOWN, transform::thread_op(body::shutdown));
        router.on(method::THREAD_ARCHIVE, transform::thread_op(body::thread_archive));
        router.on(method::THREAD_ROLLBACK, transform::thread_op(body::thread_rollback));
        // establish_op: bind + fan-out centrally
        router.on_erased(method::THREAD_FORK, transform::establish_op(body::thread_fork));
        router.on_erased(method::THREAD_RESUME, transform::establish_op(body::thread_resume));
        Self { session, router }
    }
}

#[async_trait::async_trait]
impl RequestHandler for HarnessHost {
    async fn handle_request(&self, method: String, params: Value) -> Result<Value, String> {
        // initialize handshake gate: every method except `initialize` requires
        // the connection to have completed the handshake first.
        if method.as_str() != method::INITIALIZE && !self.session.is_initialized() {
            return Err("harness socket not initialized: send `initialize` first".to_owned());
        }

        if method.as_str() == method::INITIALIZE {
            self.session.mark_initialized();
            return serde_json::to_value(InitializeResult {
                server_info: Some(ServerInfo {
                    name: "slab-server".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                }),
                protocol_version: Some("1.0".to_owned()),
                capabilities: Some(ServerCapabilities::default()),
            })
            .map_err(|e| e.to_string());
        }

        self.router.dispatch(self.session.clone(), &method, params).await
    }
}
