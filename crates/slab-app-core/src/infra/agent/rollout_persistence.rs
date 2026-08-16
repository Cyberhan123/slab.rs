//! Rollout event-source persistence observer.
//!
//! The per-thread background task that bridges the harness-protocol (`EventMsg`)
//! stream into the rollout JSONL true source (it superseded the old
//! SQL-writeback `turn_item_persistence` observer, which was removed
//! once the rollout adapter became the only `AgentStorePort` impl). One observer
//! task per thread (the agent `AgentCore` guards with a `DashSet`).
//!
//! The observer consumes a DEDICATED UNBOUNDED persistence
//! channel (`AgentEventHub::persistence_subscribe`), NOT the UI broadcast. The
//! bounded broadcast could `Lagged`-drop persistence-grade events under flood —
//! a silent conversation-data-loss false-green. The unbounded mpsc guarantees
//! delivery; there is NO `Lagged` branch in this loop. It subscribes to the
//! persistence channel and appends each event to the rollout file in its
//! [`RolloutItem`] form:
//!
//! - [`EventMsg::ItemCompleted`] → [`RolloutItem::TurnItem`] (full-fidelity UI
//!   artifact).
//! - [`EventMsg::ContextCompacted`] → [`RolloutItem::Compacted`] (the compaction
//!   marker; carries an empty baseline because the post-compaction summary
//!   arrives asynchronously in the next `TurnState.input_messages` — see the
//!   `RolloutStore::read_messages` replay rules). The `turn_index` stamp is
//!   tracked from turn-lifecycle events so a rollback that drops this turn also
//!   drops the marker (the H3 fix).
//! - any other event allowed by [`EventPersistenceMode`] →
//!   [`RolloutItem::EventMsg`].
//!
//! At every turn boundary (after the boundary flush) the observer has durable
//! data on disk. The cross-turn barrier in `fork_thread` / `compact_thread` /
//! `rollback_thread` / `restore_session` is the FIFO `Barrier` sentinel handled
//! in the `recv()` loop (flush + oneshot reply): when the observer reaches it,
//! every prior event on the unbounded mpsc has been appended + flushed, so the
//! caller may re-read the rollout with the complete history.
//!
//! Each append is fire-and-forget into the recorder actor; errors are warned,
//! not fatal — a transient flush failure must not kill the persistence task.
//!
//! Lives in `infra` (a background persistence task bridging the event stream to
//! the store) so the domain layer never backflows into `application`.

use std::sync::Arc;

use slab_agent::protocol::EventMsg;
use slab_agent_rollout::{
    CompactedPayload, EventPersistenceMode, RolloutFileStore, RolloutItem, RolloutStore,
};

use crate::infra::agent::event_hub::{AgentEventHub, PersistenceMessage};

/// Spawn a background task that persists the rollout items for `thread_id`.
///
/// Intended to be called at most once per thread (the caller guards with a
/// `DashSet`). Subscribe to the DEDICATED UNBOUNDED persistence channel, drain
/// the spawn-race replay snapshot, then loop on the live mpsc receiver. The mpsc
/// is unbounded ⇒ there is NO `Lagged` branch (the false-green class is gone).
/// On channel close (all senders dropped) it exits.
pub fn spawn_rollout_persistence(
    rollout: Arc<RolloutFileStore>,
    events: Arc<AgentEventHub>,
    thread_id: String,
    mode: EventPersistenceMode,
) {
    tokio::spawn(async move {
        let (replay_snap, mut receiver) = events.persistence_subscribe(&thread_id);
        // Current turn affiliation, tracked from turn-lifecycle events so that
        // a `ContextCompacted` event (which carries no `turn_id`) can stamp the
        // correct `turn_index` on the Compacted marker.
        let mut current_turn: u32 = 0;

        for msg in &replay_snap {
            process_event_msg(&rollout, &thread_id, mode, &mut current_turn, msg).await;
        }

        while let Some(message) = receiver.recv().await {
            match message {
                PersistenceMessage::Event(msg) => {
                    process_event_msg(&rollout, &thread_id, mode, &mut current_turn, &msg).await;
                }
                PersistenceMessage::Barrier(reply) => {
                    // Cross-turn barrier (D2): every prior event on this FIFO
                    // mpsc has been processed (appended to the recorder). Flush
                    // so the data is durable, THEN reply — releasing the barrier
                    // caller (fork/compact/rollback/restore) to re-read the
                    // rollout with the complete history.
                    if let Err(error) = rollout.flush(&thread_id).await {
                        tracing::warn!(
                            thread_id = %thread_id,
                            error = %error,
                            "barrier flush failed; releasing barrier with buffered data",
                        );
                    }
                    let _ = reply.send(());
                }
            }
        }
    });
}

/// Append the rollout line(s) for one event.
///
/// Advances `current_turn` in place from turn-lifecycle events (`TurnStarted` /
/// `TurnCompleted` / `TurnAborted`) before persistence so `ContextCompacted`
/// (which carries no `turn_id` of its own) stamps the correct turn.
///
/// At every turn boundary (`TurnCompleted` / `TurnAborted`) and after a
/// `ContextCompacted`, a fire-and-forget [`RolloutStore::flush`] materializes
/// the rollout file (the recorder only writes on `Persist`/`Shutdown`/`Truncate`
/// — without this flush a freshly-written thread's file is never materialized,
/// so a subsequent read falls through to an empty SQL fallback and the history
/// looks unreadable). The flush is best-effort: a transient failure is warned
/// and never kills the persistence task.
///
/// NOTE: the cross-turn durability barrier is the FIFO `Barrier`
/// sentinel handled in the `recv()` loop above (flush + oneshot reply), NOT a
/// durable-turn watch stamped here. This function appends lines and flushes at
/// boundaries; it does not return a boundary signal.
async fn process_event_msg(
    rollout: &Arc<RolloutFileStore>,
    thread_id: &str,
    mode: EventPersistenceMode,
    current_turn: &mut u32,
    msg: &EventMsg,
) {
    // Track the active turn from lifecycle events first.
    *current_turn = match msg {
        EventMsg::TurnStarted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(*current_turn),
        EventMsg::TurnCompleted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(*current_turn),
        EventMsg::TurnAborted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(*current_turn),
        _ => *current_turn,
    };

    // A turn boundary (or a compaction) is a flush point: materialize the file
    // so the next read sees the just-persisted items. F1.
    let flush_after = matches!(
        msg,
        EventMsg::TurnCompleted(_) | EventMsg::TurnAborted(_) | EventMsg::ContextCompacted(_)
    );

    match msg {
        EventMsg::ItemCompleted(params) => {
            if let Err(error) =
                rollout.append(thread_id, RolloutItem::TurnItem(params.item.clone())).await
            {
                tracing::warn!(thread_id, error = %error, "failed to persist turn item to rollout");
            }
        }
        EventMsg::MessageAppended(params) => {
            // Conversation message write path. Maps 1:1 to the
            // rollout TurnContext::MessageAppend line, preserving F3 (id /
            // created_at). Replaces the old slab-agent store-trait
            // `insert_thread_message` route.
            use slab_agent_rollout::TurnContextPayload;
            if let Err(error) = rollout
                .append(
                    thread_id,
                    RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                        turn_index: params.turn_index,
                        message: params.message.clone(),
                        id: Some(params.id.clone()),
                        created_at: Some(params.created_at.clone()),
                    }),
                )
                .await
            {
                tracing::warn!(thread_id, error = %error, "failed to persist message append to rollout");
            }
        }
        EventMsg::TurnStateChanged(params) => {
            // Turn-state write path. Maps 1:1 to the rollout
            // TurnContext::TurnState line, preserving F4 (started_at). The input
            // messages travel as a typed vec (NOT a json blob) so the F6
            // raw-blob recovery path is dead here — `input_messages_raw` is
            // always None. Replaces the old slab-agent store-trait
            // `upsert_turn_state` route.
            use slab_agent_rollout::TurnContextPayload;
            if let Err(error) = rollout
                .append(
                    thread_id,
                    RolloutItem::TurnContext(TurnContextPayload::TurnState {
                        turn_index: params.turn_index,
                        status: params.status.clone(),
                        input_messages: params.input_messages.clone(),
                        tool_specs_json: params.tool_specs_json.clone(),
                        llm_response_json: params.llm_response_json.clone(),
                        error: params.error.clone(),
                        completed_at: params.completed_at.clone(),
                        started_at: Some(params.started_at.clone()),
                        input_messages_raw: None,
                    }),
                )
                .await
            {
                tracing::warn!(thread_id, error = %error, "failed to persist turn state to rollout");
            }
        }
        EventMsg::ContextCompacted(params) => {
            // Field-shape note: `ContextCompactedParams` carries no
            // `compacted_messages` list and no `turn_id` (unlike the plan's
            // CompactedPayload). The summary arrives asynchronously in the next
            // `TurnState.input_messages`, so `compacted_messages` is empty here
            // (an empty baseline is the documented auto-compact semantics). The
            // turn affiliation is the `current_turn` tracked above. `status`
            // preserves the upstream value (`"compacted"` or `"skipped"` — the
            // skipped path must survive so `read_messages` can treat it as a
            // no-op; defaulting to `"compacted"` when absent). Manual
            // compaction writes its own Compacted line directly.
            let payload = CompactedPayload {
                thread_id: thread_id.to_owned(),
                compacted_messages: Vec::new(),
                removed_messages: params.removed_messages.unwrap_or(0),
                output_tokens: params.output_tokens.unwrap_or(0),
                status: params.status.clone().unwrap_or_else(|| "compacted".to_owned()),
                turn_index: *current_turn,
            };
            if let Err(error) = rollout.append(thread_id, RolloutItem::Compacted(payload)).await {
                tracing::warn!(thread_id, error = %error, "failed to persist compaction to rollout");
            }
        }
        // Any other event allowed by the policy becomes an EventMsg line. The
        // explicit arms above take precedence so ItemCompleted/ContextCompacted
        // are never double-written as EventMsg lines.
        other if mode.should_persist(other) => {
            if let Err(error) =
                rollout.append(thread_id, RolloutItem::EventMsg(other.clone())).await
            {
                tracing::warn!(thread_id, error = %error, "failed to persist event to rollout");
            }
        }
        _ => {}
    }

    if flush_after {
        // Fire-and-forget durability flush at the turn boundary. The recorder
        // is lazy (it only writes on Persist/Shutdown/Truncate); without this
        // flush the file stays un-materialized and the adapter's read gate
        // (file_exists) never sees it. (F1)
        if let Err(error) = rollout.flush(thread_id).await {
            tracing::warn!(
                thread_id,
                error = %error,
                "rollout turn-boundary flush failed; items remain buffered for next flush",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::port::AgentNotifyPort;
    use slab_agent::protocol::{
        ContextCompactedParams, ErrorEvent, ItemCompletedParams, MessageAppendedParams, Turn,
        TurnCompletedParams, TurnStartedParams, TurnStateChangedParams,
    };
    use slab_agent_rollout::{RolloutItem, SessionMeta, TurnContextPayload};

    fn assistant_item(id: &str, text: &str) -> slab_agent::protocol::TurnItem {
        slab_agent::protocol::TurnItem::AgentMessage { id: id.to_owned(), text: text.to_owned() }
    }

    fn user_msg(text: &str) -> slab_types::ConversationMessage {
        slab_types::ConversationMessage {
            role: "user".to_owned(),
            content: slab_types::ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn harness(dir: &tempfile::TempDir) -> (Arc<RolloutFileStore>, Arc<AgentEventHub>) {
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        // Prime a session so ItemCompleted appends don't auto-create a default
        // meta (keeps the assertions focused on the observer-written lines).
        rollout.create_session(SessionMeta {
            thread_id: "t".to_owned(),
            session_id: "s".to_owned(),
            parent_id: None,
            started_at: "x".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        let events = Arc::new(AgentEventHub::new());
        (rollout, events)
    }

    #[tokio::test]
    async fn item_completed_appends_turn_item() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        // Broadcast an ItemCompleted; the observer appends a TurnItem line.
        events
            .on_event_msg(
                "t",
                &EventMsg::ItemCompleted(ItemCompletedParams {
                    item: assistant_item("a1", "hi"),
                    thread_id: "t".to_owned(),
                    turn_id: "0".to_owned(),
                }),
            )
            .await;

        // Give the background task a moment to drain, then flush + read.
        rollout.flush("t").await.unwrap();
        let assertion = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if !rollout.read_turn_items("t").await.is_empty() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        assertion.expect("observer did not persist item in time");

        let items = rollout.read_turn_items("t").await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "a1");
    }

    #[tokio::test]
    async fn context_compacted_appends_compacted_marker_with_turn_index() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        // TurnStarted sets current_turn = 3, then ContextCompacted stamps it.
        events
            .on_event_msg(
                "t",
                &EventMsg::TurnStarted(TurnStartedParams {
                    thread_id: "t".to_owned(),
                    turn: Turn { id: "3".to_owned(), ..Default::default() },
                }),
            )
            .await;
        events
            .on_event_msg(
                "t",
                &EventMsg::ContextCompacted(ContextCompactedParams {
                    thread_id: "t".to_owned(),
                    status: Some("compacted".to_owned()),
                    removed_messages: Some(5),
                    output_tokens: Some(42),
                }),
            )
            .await;

        // Wait for the Compacted line to land.
        let assertion = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                let lines = slab_agent_rollout::read_rollout_lines(&rollout.resolve_path("t"));
                if lines.iter().any(|l| matches!(l.item, RolloutItem::Compacted(_))) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        assertion.expect("observer did not persist compacted marker in time");

        let lines = slab_agent_rollout::read_rollout_lines(&rollout.resolve_path("t"));
        let compacted = lines
            .iter()
            .find_map(|l| match &l.item {
                RolloutItem::Compacted(p) => Some(p.clone()),
                _ => None,
            })
            .expect("compacted marker present");
        assert_eq!(compacted.turn_index, 3, "turn_index tracked from TurnStarted");
        assert_eq!(compacted.removed_messages, 5);
        assert_eq!(compacted.output_tokens, 42);
        // F7a: status is preserved from upstream (not hardcoded "auto").
        assert_eq!(compacted.status, "compacted");
        assert!(compacted.compacted_messages.is_empty(), "auto-compact baseline is empty");
    }

    #[tokio::test]
    async fn lifecycle_event_appended_as_event_msg() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        events
            .on_event_msg(
                "t",
                &EventMsg::TurnCompleted(TurnCompletedParams {
                    thread_id: "t".to_owned(),
                    turn: Turn {
                        id: "1".to_owned(),
                        status: "completed".to_owned(),
                        ..Default::default()
                    },
                    usage: None,
                }),
            )
            .await;

        let assertion = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if !rollout.read_events("t").await.is_empty() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        assertion.expect("observer did not persist event in time");

        let evs = rollout.read_events("t").await;
        assert!(evs.iter().any(|e| matches!(e, EventMsg::TurnCompleted(_))));
    }

    // Regression guard: under the DEFAULT `EventPersistenceMode::Limited`,
    // `Error` (and `Warning`) events are persisted as `RolloutItem::EventMsg`
    // lines via the observer's `should_persist` fallback arm. `Error` is NOT in
    // the structural `is_persistence_grade` set, so this only works because
    // `on_event_msg` routes EVERY event to the dedicated persistence channel
    // (not just the structural subset). Routing only the structural variants
    // would silently drop Error/Warning from the rollout timeline under Limited
    // — the exact regression this test pins.
    //
    // Mutation that MUST fail: gate routing in `on_event_msg` on
    // `is_persistence_grade(msg)` again. `Error` is non-structural, so it is
    // never routed to the persistence channel → the observer never sees it →
    // `read_events` returns empty → the assertion fails.
    #[tokio::test]
    async fn limited_mode_persists_error_event_via_should_persist_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        events
            .on_event_msg("t", &EventMsg::Error(ErrorEvent::new("boom").with_code("turn_failed")))
            .await;

        let assertion = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if rollout.read_events("t").await.iter().any(|e| matches!(e, EventMsg::Error(_))) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        assertion.expect("Error event was not persisted under Limited mode");

        let evs = rollout.read_events("t").await;
        assert!(
            evs.iter().any(|e| matches!(e, EventMsg::Error(_))),
            "Limited mode persists Error via the should_persist fallback (non-structural event)"
        );
    }

    #[tokio::test]
    async fn compacted_marker_turn_gated_on_truncate() {
        // H3 regression: a Compacted marker whose turn is rolled back must be
        // dropped, otherwise read_messages resets to a summary of deleted
        // messages. Verify the observer stamps a real turn_index (not 0) so the
        // store's turn-gating kicks in.
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        // Turn 2 message + compaction at turn 2.
        rollout
            .append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: 0,
                    message: slab_types::ConversationMessage {
                        role: "user".to_owned(),
                        content: slab_types::ConversationMessageContent::Text("A".to_owned()),
                        name: None,
                        tool_call_id: None,
                        tool_calls: vec![],
                    },
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();
        events
            .on_event_msg(
                "t",
                &EventMsg::TurnStarted(TurnStartedParams {
                    thread_id: "t".to_owned(),
                    turn: Turn { id: "2".to_owned(), ..Default::default() },
                }),
            )
            .await;
        events
            .on_event_msg(
                "t",
                &EventMsg::ContextCompacted(ContextCompactedParams {
                    thread_id: "t".to_owned(),
                    status: None,
                    removed_messages: None,
                    output_tokens: None,
                }),
            )
            .await;

        // Wait for the marker, then roll back to turn 1 (drops turn 2+).
        let wait = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if slab_agent_rollout::read_rollout_lines(&rollout.resolve_path("t"))
                    .iter()
                    .any(|l| matches!(l.item, RolloutItem::Compacted(_)))
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        wait.expect("compacted marker not persisted");

        rollout.truncate_from_turn("t", 1).await.unwrap();

        let messages = rollout.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                slab_types::ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        // Compacted (turn 2) was dropped by the rollback → no reset; only A.
        assert_eq!(texts, vec!["A".to_owned()], "compacted marker rolled back, no reset");
    }

    // M5 (2d) — TurnItem turn attribution invariant.
    //
    // slab-agent GUARANTEES that the turn-N user message is persisted (a
    // MessageAppend routed through the store adapter → recorder) BEFORE the turn
    // loop emits any ItemCompleted for turn N: `control.send_input` builds the
    // messages vec with the new user content and passes `persist_from` into
    // `AgentThread::run`, which calls `persist_thread_message` for the user
    // message (thread.rs) BEFORE the `'turns` loop starts `execute_turn`. The
    // adapter writes that MessageAppend directly to the recorder; the observer
    // writes each TurnItem only after a broadcast hop. Both share ONE FIFO
    // recorder per thread, so MessageAppend(turn N) lands in the file before
    // TurnItem(turn N), and `read_turn_items`'s running-turn heuristic attributes
    // every item correctly.
    //
    // WITHIN a turn this is a synchronized guarantee (the user-message append is
    // awaited before execute_turn runs). ACROSS turns it is an assumption that
    // holds in practice, NOT a synchronized invariant: there is no per-thread
    // turn-boundary barrier fencing the observer's drain against the next turn's
    // send_input. The race is negligible because the observer has the whole
    // turn-teardown + client-roundtrip + send_input-setup window to drain one
    // event before the next turn's MessageAppend lands, but it is timing-based,
    // not barrier-enforced. This test reproduces the production ordering (for
    // each turn: write the user MessageAppend, emit ItemCompleted, and wait for
    // the observer to persist the item before advancing), which mirrors the
    // natural turn-serialization window.
    #[tokio::test]
    async fn turn_item_attribution_uses_prior_turn_context() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        for (turn, item_id) in [(0u32, "a0"), (1, "a1")] {
            // The adapter writes the turn-N user MessageAppend FIRST (direct to
            // the recorder, as slab-agent does before the turn runs).
            rollout
                .append(
                    "t",
                    RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                        turn_index: turn,
                        message: user_msg(&format!("u{turn}")),
                        id: None,
                        created_at: None,
                    }),
                )
                .await
                .unwrap();
            // Then slab-agent emits ItemCompleted (observer path, one broadcast
            // hop later) — still within the same turn.
            events
                .on_event_msg(
                    "t",
                    &EventMsg::ItemCompleted(ItemCompletedParams {
                        item: assistant_item(item_id, "r"),
                        thread_id: "t".to_owned(),
                        turn_id: turn.to_string(),
                    }),
                )
                .await;

            // Wait for THIS turn's item to land before starting the next turn —
            // mirrors production turn serialization (a turn's items are durable
            // before the next send_input runs).
            let wait = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    rollout.flush("t").await.unwrap();
                    if rollout.read_turn_items("t").await.iter().any(|i| i.id == item_id) {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await;
            wait.expect("observer did not persist the turn item in time");
        }

        let items = rollout.read_turn_items("t").await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "a0");
        assert_eq!(items[0].turn_index, 0, "M5: turn-0 item attributed to turn 0");
        assert_eq!(items[1].id, "a1");
        assert_eq!(items[1].turn_index, 1, "M5: turn-1 item attributed to turn 1");
    }

    // P6 — TurnState-anchored attribution.
    //
    // `read_turn_items` advances `current_turn` on ANY `TurnContext` line
    // (MessageAppend OR TurnState) with a new turn_index. `TurnStateChanged` is
    // the turn-state write path (replacing the slab-agent
    // store `upsert_turn_state`), so a `TurnStateChanged` alone must anchor
    // attribution for the items that follow — even with NO `MessageAppend` for
    // that turn. This test drives exactly that: emit `TurnStateChanged(2)`,
    // then an `ItemCompleted`, and assert the item lands at turn 2.
    //
    // Mutation that MUST fail: change `read_turn_items` to advance
    // `current_turn` on `TurnItem` instead of `TurnContext` (or skip the
    // TurnContext branch). Then `current_turn` stays 0 until the first
    // `TurnItem`, so the item is attributed to turn 0 (wrong) → the
    // `turn_index == 2` assertion fails. This is the attribution false-green
    // the TurnContext-advance rule exists to prevent.
    #[tokio::test]
    async fn turn_state_changed_alone_anchors_turn_item_attribution() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        // Seed a turn-0 baseline message so the rollout file is non-empty, then
        // emit TurnStateChanged(turn=2) — the new write path — and an item.
        rollout
            .append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: 0,
                    message: user_msg("baseline"),
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();
        events
            .on_event_msg(
                "t",
                &EventMsg::TurnStateChanged(TurnStateChangedParams {
                    thread_id: "t".to_owned(),
                    turn_index: 2,
                    status: "running".to_owned(),
                    input_messages: vec![user_msg("turn-2-input")],
                    tool_specs_json: None,
                    llm_response_json: None,
                    error: None,
                    started_at: "2026-01-01T00:00:00Z".to_owned(),
                    completed_at: None,
                }),
            )
            .await;
        events
            .on_event_msg(
                "t",
                &EventMsg::ItemCompleted(ItemCompletedParams {
                    item: assistant_item("a2", "r"),
                    thread_id: "t".to_owned(),
                    turn_id: "2".to_owned(),
                }),
            )
            .await;

        let wait = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if rollout.read_turn_items("t").await.iter().any(|i| i.id == "a2") {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        wait.expect("observer did not persist the turn item in time");

        let items = rollout.read_turn_items("t").await;
        let a2 = items.iter().find(|i| i.id == "a2").expect("item a2 present");
        assert_eq!(
            a2.turn_index, 2,
            "TurnStateChanged(2) alone must anchor attribution: item attributed to turn 2"
        );
    }

    // Test A (P7) — attribution under the FULLY event-driven write path,
    // INCLUDING turn 0 of a fresh thread (the SpawnRequest path).
    //
    // BOTH the message append AND the item completion are routed through
    // the same dedicated unbounded persistence mpsc (FIFO ⇒ file order). This
    // test drives a 2-turn conversation entirely via `on_event_msg` (no direct
    // rollout writes) — turn 0 of a FRESH thread (the M5 anchor: the user
    // MessageAppended lands before the turn's ItemCompleted) and turn 1 — and
    // asserts `read_turn_items` attributes every item to the correct turn AND
    // `read_messages` retains the user messages.
    //
    // Mutations that MUST fail:
    //  (a) make `read_turn_items` advance `current_turn` on `TurnItem` instead
    //      of `TurnContext` → the turn-1 user MessageAppended no longer advances
    //      the turn before the turn-1 item → misattribution (turn-1 item lands
    //      at turn 0) → the `a1.turn_index == 1` assertion fails.
    //  (b) revert the persistence channel to the bounded broadcast and flood
    //      300 persistence-grade events between the MessageAppended and the
    //      ItemCompleted → the broadcast `Lagged`-drops the MessageAppended →
    //      `read_messages` loses the user message (the no-Lag guarantee is
    //      pinned separately by `persistence_channel_delivers_all_events_under_flood_no_lag`).
    #[tokio::test]
    async fn event_driven_attribution_includes_turn_zero_of_fresh_thread() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        for (turn, item_id, user_text) in [(0u32, "a0", "u0"), (1, "a1", "u1")] {
            // The user MessageAppended (M5 anchor) — emitted BEFORE the turn's
            // ItemCompleted, exactly as slab-agent's `emit_new` loop does.
            events
                .on_event_msg(
                    "t",
                    &EventMsg::MessageAppended(MessageAppendedParams {
                        thread_id: "t".to_owned(),
                        turn_index: turn,
                        message: user_msg(user_text),
                        id: format!("m{turn}"),
                        created_at: "2026-01-01T00:00:00Z".to_owned(),
                    }),
                )
                .await;
            events
                .on_event_msg(
                    "t",
                    &EventMsg::ItemCompleted(ItemCompletedParams {
                        item: assistant_item(item_id, "r"),
                        thread_id: "t".to_owned(),
                        turn_id: turn.to_string(),
                    }),
                )
                .await;

            // Poll-until-present: the within-turn guarantee is FIFO (the observer
            // consumes the ordered mpsc), so each turn's item is durable before
            // the next turn begins.
            let wait = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    rollout.flush("t").await.unwrap();
                    if rollout.read_turn_items("t").await.iter().any(|i| i.id == item_id) {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await;
            wait.expect("observer did not persist the turn item in time");
        }

        let items = rollout.read_turn_items("t").await;
        assert_eq!(items.len(), 2);
        assert_eq!(
            items.iter().find(|i| i.id == "a0").unwrap().turn_index,
            0,
            "turn-0 item attributed to turn 0"
        );
        assert_eq!(
            items.iter().find(|i| i.id == "a1").unwrap().turn_index,
            1,
            "turn-1 item attributed to turn 1 (MessageAppended advanced the turn)"
        );

        // The user messages survived (no Lag drop on the persistence channel).
        let messages = rollout.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                slab_types::ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(texts.contains(&"u0".to_owned()), "turn-0 user message retained");
        assert!(texts.contains(&"u1".to_owned()), "turn-1 user message retained");
    }

    // 2e — auto-compaction via maybe_compact IS persisted by the single
    // observer+adapter chain (no second persistence path). The observer captures
    // ContextCompacted → Compacted line (empty baseline; the summary is produced
    // asynchronously). The adapter then writes the next TurnState carrying the
    // compacted input_messages (slab-agent's persist_turn_state at turn end, run
    // AFTER maybe_compact replaced the in-memory messages vec), which becomes the
    // post-compaction baseline on read. Verify the two-writer chain end-to-end.
    #[tokio::test]
    async fn auto_compact_persists_via_observer_then_adapter_turn_state() {
        let dir = tempfile::tempdir().unwrap();
        let (rollout, events) = harness(&dir);
        spawn_rollout_persistence(
            Arc::clone(&rollout),
            Arc::clone(&events),
            "t".to_owned(),
            EventPersistenceMode::Limited,
        );

        // Pre-compaction history (turn 0).
        rollout
            .append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: 0,
                    message: user_msg("old"),
                    id: None,
                    created_at: None,
                }),
            )
            .await
            .unwrap();

        // Auto-compact fires at turn 1 start: TurnStarted sets current_turn = 1,
        // then ContextCompacted → observer writes Compacted (empty baseline).
        events
            .on_event_msg(
                "t",
                &EventMsg::TurnStarted(TurnStartedParams {
                    thread_id: "t".to_owned(),
                    turn: Turn { id: "1".to_owned(), ..Default::default() },
                }),
            )
            .await;
        events
            .on_event_msg(
                "t",
                &EventMsg::ContextCompacted(ContextCompactedParams {
                    thread_id: "t".to_owned(),
                    status: Some("compacted".to_owned()),
                    removed_messages: Some(1),
                    output_tokens: Some(3),
                }),
            )
            .await;

        // Wait for the Compacted marker to land before writing the TurnState
        // (mirrors the file order: compaction at turn start, TurnState at turn end).
        let wait = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                rollout.flush("t").await.unwrap();
                if slab_agent_rollout::read_rollout_lines(&rollout.resolve_path("t"))
                    .iter()
                    .any(|l| matches!(l.item, RolloutItem::Compacted(_)))
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        wait.expect("auto-compact marker not persisted");

        // The adapter writes the turn-1 TurnState carrying the compacted baseline
        // (persist_turn_state at turn end, after maybe_compact replaced `messages`).
        rollout
            .append(
                "t",
                RolloutItem::TurnContext(TurnContextPayload::TurnState {
                    turn_index: 1,
                    status: "completed".to_owned(),
                    input_messages: vec![user_msg("summary")],
                    tool_specs_json: None,
                    llm_response_json: None,
                    error: None,
                    completed_at: None,
                    started_at: None,
                    input_messages_raw: None,
                }),
            )
            .await
            .unwrap();

        let messages = rollout.read_messages("t").await;
        let texts: Vec<String> = messages
            .iter()
            .map(|m| match &m.content {
                slab_types::ConversationMessageContent::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect();
        // The pre-compaction "old" message is gone; the compacted baseline
        // ("summary", from the post-compaction TurnState) is the new baseline.
        assert_eq!(
            texts,
            vec!["summary".to_owned()],
            "auto-compact baseline persisted via observer (Compacted) + adapter (TurnState)"
        );
    }
}
