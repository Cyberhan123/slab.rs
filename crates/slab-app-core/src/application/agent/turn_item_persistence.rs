//! Persists finalized harness `TurnItem` snapshots by consuming the
//! harness-protocol event stream.
//!
//! One observer task per thread (spawn-once — the agent `AgentCore` guards
//! with a `DashSet`). It subscribes to the [`AgentEventHub`] `EventMsg` stream
//! and on each [`EventMsg::ItemCompleted`] writes the carried `TurnItem` via
//! [`AgentStorePort::insert_turn_item`].
//!
//! slab-agent emits `ItemCompleted` directly for every finalized item
//! (assistant text, reasoning, tool calls), so a single per-thread observer
//! captures them all across every one of the thread's runs — there is no need
//! for per-run termination or terminal-status detection. Inserts are idempotent
//! (`INSERT OR IGNORE` on `(thread_id, id)`), so any overlap is harmless. The
//! live WS fan-out never writes the store, so there is no double-write.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use slab_agent::port::{AgentStorePort, TurnItemRecord};
use slab_proto::harness::event::EventMsg;
use tokio::sync::broadcast;

use crate::infra::agent::event_hub::AgentEventHub;

/// Spawn a background task that persists finalized `TurnItem`s for `thread_id`.
///
/// Intended to be called at most once per thread (the caller guards with a
/// `DashSet`). Subscribes to the harness-protocol (`EventMsg`) stream and, on
/// each `ItemCompleted`, writes the carried `TurnItem` via
/// [`AgentStorePort::insert_turn_item`]. slab-agent now emits `ItemCompleted`
/// directly for every finalized item (assistant text, reasoning, tool calls),
/// so this consumes `EventMsg` only — no projection layer.
///
/// One task per thread, one shared per-turn `seq` counter — never spawn a
/// second observer (two tasks would produce colliding `seq` values within a
/// turn). Inserts are idempotent (`INSERT OR IGNORE` on `(thread_id, id)`), so
/// any overlap is harmless. On broadcast lag it logs and continues (a lagged
/// turn may drop some items — acceptable degradation, not corruption); on
/// channel close it exits.
pub fn spawn_turn_item_persistence(
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
    thread_id: String,
) {
    tokio::spawn(async move {
        let subscription = events.subscribe_event_msgs(&thread_id);
        // Per-turn finalized-item counter; deterministic because each
        // `ItemCompleted` is processed exactly once by this single task.
        let mut seq_by_turn: HashMap<u32, u32> = HashMap::new();

        for envelope in &subscription.replay {
            process_msg(&store, &thread_id, &mut seq_by_turn, &envelope.msg).await;
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    process_msg(&store, &thread_id, &mut seq_by_turn, &envelope.msg).await;
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

/// Persist the `TurnItem` carried by an `ItemCompleted`, if any.
async fn process_msg(
    store: &Arc<dyn AgentStorePort>,
    thread_id: &str,
    seq_by_turn: &mut HashMap<u32, u32>,
    msg: &EventMsg,
) {
    let EventMsg::ItemCompleted(params) = msg else {
        return;
    };

    let turn_index = params.turn_id.parse::<u32>().ok().unwrap_or(0);
    let seq = seq_by_turn.entry(turn_index).or_default();
    let item_json = match serde_json::to_string(&params.item) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(thread_id, error = %error, "skip unserializable TurnItem");
            return;
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
