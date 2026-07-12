//! Persists finalized harness `TurnItem` snapshots by consuming the agent event
//! stream.
//!
//! One observer task per thread (spawn-once — the agent `AgentCore` guards
//! with a `DashSet`). It owns a private [`HarnessProjection`], subscribes to
//! [`AgentEventHub`], and on each [`EventMsg::ItemCompleted`] writes the carried
//! `TurnItem` via [`AgentStorePort::insert_turn_item`].
//!
//! `ItemCompleted` is always emitted by the projection (it is not gated by
//! `started_items`), so a single per-thread observer captures every finalized
//! item across all of the thread's runs — there is no need for per-run
//! termination or terminal-status detection. Inserts are idempotent
//! (`INSERT OR IGNORE` on `(thread_id, turn_index, seq)`), so any overlap is
//! harmless. The live WS fan-out never writes the store, so there is no
//! double-write; both consumers reuse the same pure projection.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use slab_agent::port::{AgentStorePort, TurnEvent, TurnItemRecord};
use slab_proto::harness::event::EventMsg;
use tokio::sync::broadcast;

use crate::application::agent::projection::harness::HarnessProjection;
use crate::infra::agent::event_hub::{AgentEventEnvelope, AgentEventHub};

/// Spawn a background task that persists finalized `TurnItem`s for `thread_id`.
///
/// Intended to be called at most once per thread (the caller guards with a
/// `DashSet`). The task processes the in-memory replay buffer first, then live
/// events, for the thread's lifetime. On broadcast lag it logs and continues (a
/// lagged turn may drop some items — acceptable degradation, not corruption);
/// on channel close it exits.
pub fn spawn_turn_item_persistence(
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
    thread_id: String,
) {
    tokio::spawn(async move {
        let subscription = events.subscribe_events(&thread_id);
        let mut proj = HarnessProjection::new();
        // Per-turn finalized-item counter; deterministic because events are
        // delivered in order and each is processed exactly once (single spawn).
        let mut seq_by_turn: HashMap<u32, u32> = HashMap::new();

        for envelope in &subscription.replay {
            process_envelope(&store, &thread_id, &mut proj, &mut seq_by_turn, envelope).await;
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    process_envelope(&store, &thread_id, &mut proj, &mut seq_by_turn, &envelope)
                        .await;
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(
                        thread_id = %thread_id,
                        missed,
                        "turn-item persistence observer lagged; some items may be dropped",
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Project one envelope and persist any `ItemCompleted` it yields.
async fn process_envelope(
    store: &Arc<dyn AgentStorePort>,
    thread_id: &str,
    proj: &mut HarnessProjection,
    seq_by_turn: &mut HashMap<u32, u32>,
    envelope: &AgentEventEnvelope,
) {
    // `turn_index` comes from the RAW envelope — the projection synthesizes
    // `turn_id` as the string `"current"` when the envelope lacks it, which
    // would not round-trip back to an index.
    let envelope_turn_index = match &envelope.event {
        TurnEvent::Response { turn_index, .. } => *turn_index,
    };

    for msg in proj.project(thread_id, envelope) {
        let EventMsg::ItemCompleted(params) = msg else { continue };

        let turn_index =
            envelope_turn_index.or_else(|| params.turn_id.parse::<u32>().ok()).unwrap_or(0);
        let seq = seq_by_turn.entry(turn_index).or_default();
        let item_json = match serde_json::to_string(&params.item) {
            Ok(json) => json,
            Err(error) => {
                tracing::warn!(thread_id, error = %error, "skip unserializable TurnItem");
                continue;
            }
        };
        let record = TurnItemRecord {
            id: params.item.id().to_owned(),
            thread_id: thread_id.to_owned(),
            turn_index,
            seq: *seq,
            item_json,
            created_at: Utc::now().to_rfc3339(),
        };
        *seq += 1;
        if let Err(error) = store.insert_turn_item(&record).await {
            tracing::warn!(thread_id, error = %error, "failed to persist turn item");
        }
    }
}
