//! Persists the complete OpenAI-Responses-canonical `Response` for each agent
//! run, driven by the turn-event stream.
//!
//! The agent runs in a detached `tokio::spawn` task that outlives any HTTP/WS
//! connection, so persistence is transport-agnostic: this observer is wired as
//! an [`AgentNotifyPort`] alongside [`AgentEventHub`] (see
//! [`super::event_hub::CompositeNotifyPort`]). It buffers one run's envelopes
//! per thread and, on the run's terminal event (`ResponseCompleted` /
//! `ResponseFailed` / `ResponseCancelled`), assembles the canonical
//! [`slab_proto::openai::Response`] via
//! [`crate::application::agent::projection::openai_response::build_response`]
//! and stores it as JSON.
//!
//! Run segmentation: one run == one terminal event. `run_id` is generated here
//! (the wire `Response.id` is still the thread id); the stored `Response.id` is
//! this per-run id so multiple runs in a multi-turn thread have distinct rows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use slab_agent::port::{AgentNotifyPort, AgentStorePort, ThreadResponseRecord, ThreadStatus};
use slab_agent::{AgentEventKind, TurnEvent};
use slab_proto::openai::ResponseStatus;
use tracing::warn;

use super::event_hub::AgentEventEnvelope;
use crate::application::agent::projection::openai_response::{AdapterInput, build_response};

/// Hard cap on buffered envelopes per run. Runs are bounded by `max_turns`, so
/// this is a backstop against unbounded memory on pathological runs; when
/// exceeded the run is persisted with what was buffered and further events are
/// dropped (with a warning) so the process is never OOM-killed.
const MAX_ENVELOPES_PER_RUN: usize = 8192;

#[derive(Default)]
struct RunBuffer {
    envelopes: Vec<AgentEventEnvelope>,
    /// Unix seconds of the first envelope in the run (Response.created_at).
    created_at_unix: Option<f64>,
    turn_index_start: Option<u32>,
    /// `true` once the buffer hit [`MAX_ENVELOPES_PER_RUN`] (drop the rest).
    saturated: bool,
}

/// Notify-port observer that persists the complete per-run `Response` JSON.
pub struct ResponsePersistenceObserver {
    store: Arc<dyn AgentStorePort>,
    runs: Mutex<HashMap<String, RunBuffer>>,
}

impl ResponsePersistenceObserver {
    pub fn new(store: Arc<dyn AgentStorePort>) -> Self {
        Self { store, runs: Mutex::new(HashMap::new()) }
    }
}

#[async_trait]
impl AgentNotifyPort for ResponsePersistenceObserver {
    async fn on_status_change(&self, _thread_id: &str, _status: ThreadStatus) {
        // Status changes are not part of any Response's output[]; no-op.
    }

    async fn on_turn_event(&self, thread_id: &str, event: &TurnEvent) {
        let terminal = match event {
            TurnEvent::Response { event: AgentEventKind::ResponseCompleted { .. }, .. } => {
                Some(("completed", ResponseStatus::Completed))
            }
            TurnEvent::Response { event: AgentEventKind::ResponseFailed { .. }, .. } => {
                Some(("failed", ResponseStatus::Failed))
            }
            TurnEvent::Response { event: AgentEventKind::ResponseCancelled { .. }, .. } => {
                Some(("cancelled", ResponseStatus::Cancelled))
            }
            _ => None,
        };

        let persist = {
            let mut runs = self.runs.lock().expect("runs mutex poisoned");
            let buffer = runs.entry(thread_id.to_owned()).or_default();
            if buffer.created_at_unix.is_none() {
                buffer.created_at_unix = Some(Utc::now().timestamp() as f64);
            }
            if let TurnEvent::Response { turn_index: Some(idx), .. } = event {
                buffer.turn_index_start.get_or_insert(*idx);
            }
            if !buffer.saturated {
                if buffer.envelopes.len() >= MAX_ENVELOPES_PER_RUN {
                    buffer.saturated = true;
                    warn!(
                        thread_id,
                        max = MAX_ENVELOPES_PER_RUN,
                        "agent run exceeded envelope buffer cap; further events dropped from stored response"
                    );
                } else {
                    buffer.envelopes.push(AgentEventEnvelope {
                        id: buffer.envelopes.len() as u64,
                        event: event.clone(),
                    });
                }
            }
            match terminal {
                Some((status, response_status)) => {
                    let RunBuffer { envelopes, created_at_unix, turn_index_start, saturated: _ } =
                        std::mem::take(buffer);
                    runs.remove(thread_id);
                    Some((envelopes, status, response_status, created_at_unix, turn_index_start))
                }
                None => None,
            }
        };

        let Some((envelopes, status, response_status, created_at_unix, turn_index_start)) = persist
        else {
            return;
        };

        let created_at_unix = created_at_unix.unwrap_or_else(|| Utc::now().timestamp() as f64);
        let completed_at_unix = Utc::now().timestamp() as f64;
        let run_id = format!("resp_{}", uuid::Uuid::new_v4().simple());

        let mut response = build_response(AdapterInput {
            response_id: &run_id,
            model: "",
            created_at_unix,
            completed_at: Some(completed_at_unix),
            envelopes: &envelopes,
            ..Default::default()
        });
        response.status = Some(response_status);

        let response_json = match serde_json::to_string(&response) {
            Ok(json) => json,
            Err(error) => {
                warn!(%error, thread_id, "failed to serialize agent run response; not persisted");
                return;
            }
        };

        // session_id is denormalized onto the row for the session index; derive
        // it from the thread snapshot rather than threading spawn context through.
        let session_id = self
            .store
            .get_thread(thread_id)
            .await
            .ok()
            .flatten()
            .map(|snapshot| snapshot.session_id)
            .unwrap_or_default();

        let record = ThreadResponseRecord {
            run_id,
            thread_id: thread_id.to_owned(),
            session_id,
            turn_index_start: turn_index_start.unwrap_or(0),
            status: status.to_owned(),
            response_json,
            created_at: Utc::now().to_rfc3339(),
            completed_at: Some(Utc::now().to_rfc3339()),
        };
        if let Err(error) = self.store.insert_thread_response(&record).await {
            warn!(%error, thread_id, "failed to persist agent run response");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::db::repository::SqlxStore;
    use slab_agent::port::{ThreadSnapshot, ThreadStatus};

    fn text_done_event() -> TurnEvent {
        TurnEvent::Response {
            turn_index: Some(0),
            event: AgentEventKind::ResponseOutputTextDone {
                item_id: "msg_1".to_owned(),
                output_index: 0,
                content_index: 0,
                text: "Hello world".to_owned(),
                artifact_refs: Vec::new(),
                reason: None,
                phase: None,
            },
        }
    }

    fn completed_event() -> TurnEvent {
        TurnEvent::Response {
            turn_index: Some(0),
            event: AgentEventKind::ResponseCompleted {
                response: slab_agent::AgentResponseRef {
                    id: "thread-1".to_owned(),
                    status: ThreadStatus::Completed,
                },
            },
        }
    }

    async fn seeded_observer() -> (Arc<dyn AgentStorePort>, ResponsePersistenceObserver) {
        let sqlite = SqlxStore::connect("sqlite::memory:").await.expect("store");
        let now = "2026-01-01T00:00:00Z".to_owned();
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, created_at, updated_at) \
             VALUES ('session-1', '', ?1, ?1)",
        )
        .bind(&now)
        .execute(&sqlite.pool)
        .await
        .expect("session");
        sqlite
            .upsert_thread(&ThreadSnapshot {
                id: "thread-1".to_owned(),
                session_id: "session-1".to_owned(),
                parent_id: None,
                depth: 0,
                status: ThreadStatus::Running,
                role_name: None,
                config_json: "{}".to_owned(),
                completion_text: None,
                created_at: now.clone(),
                updated_at: now,
            })
            .await
            .expect("thread");
        let store: Arc<dyn AgentStorePort> = Arc::new(sqlite);
        let observer = ResponsePersistenceObserver::new(Arc::clone(&store));
        (store, observer)
    }

    #[tokio::test]
    async fn observer_persists_response_only_on_terminal_event() {
        let (store, observer) = seeded_observer().await;

        observer.on_turn_event("thread-1", &text_done_event()).await;
        // Non-terminal: nothing persisted yet.
        assert!(store.list_thread_responses("thread-1").await.unwrap().is_empty());

        observer.on_turn_event("thread-1", &completed_event()).await;

        let responses = store.list_thread_responses("thread-1").await.expect("list");
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].status, "completed");
        assert_eq!(responses[0].session_id, "session-1");
        assert_eq!(responses[0].thread_id, "thread-1");
        assert!(responses[0].response_json.contains("Hello world"));
        assert!(responses[0].response_json.contains("\"object\":\"response\""));
        assert!(responses[0].run_id.starts_with("resp_"));
    }

    #[tokio::test]
    async fn observer_segments_runs_by_terminal_event() {
        let (store, observer) = seeded_observer().await;

        // Run 1.
        observer.on_turn_event("thread-1", &text_done_event()).await;
        observer.on_turn_event("thread-1", &completed_event()).await;
        // Run 2 (a second run in the same thread).
        observer.on_turn_event("thread-1", &text_done_event()).await;
        observer.on_turn_event("thread-1", &completed_event()).await;

        let responses = store.list_thread_responses("thread-1").await.expect("list");
        assert_eq!(responses.len(), 2, "two runs produce two stored responses");
        assert_ne!(responses[0].run_id, responses[1].run_id);
    }
}
