//! Rollout event-source persistence observer.
//!
//! The per-thread background task that bridges the harness-protocol (`EventMsg`)
//! stream into the rollout JSONL true source (it superseded the old
//! SQL-writeback `turn_item_persistence` observer, which was removed in Slice 7
//! once the rollout adapter became the only `AgentStorePort` impl). One observer
//! task per thread (the agent `AgentCore` guards with a `DashSet`). It subscribes
//! to the [`AgentEventHub`] `EventMsg` stream and appends each event to the
//! rollout file in its [`RolloutItem`] form:
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
use tokio::sync::broadcast;

use crate::infra::agent::event_hub::AgentEventHub;

/// Spawn a background task that persists the rollout items for `thread_id`.
///
/// Intended to be called at most once per thread (the caller guards with a
/// `DashSet`). Subscribe to the harness-protocol stream, drain the replay buffer,
/// then loop on the live receiver. On broadcast lag it logs and continues (a
/// lagged turn may drop some events — acceptable degradation, not corruption);
/// on channel close it exits.
pub fn spawn_rollout_persistence(
    rollout: Arc<RolloutFileStore>,
    events: Arc<AgentEventHub>,
    thread_id: String,
    mode: EventPersistenceMode,
) {
    tokio::spawn(async move {
        let subscription = events.subscribe_event_msgs(&thread_id);
        // Current turn affiliation, tracked from turn-lifecycle events so that
        // a `ContextCompacted` event (which carries no `turn_id`) can stamp the
        // correct `turn_index` on the Compacted marker.
        let mut current_turn: u32 = 0;

        for envelope in &subscription.replay {
            current_turn =
                process_event_msg(&rollout, &thread_id, mode, current_turn, &envelope.msg).await;
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    current_turn =
                        process_event_msg(&rollout, &thread_id, mode, current_turn, &envelope.msg)
                            .await;
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(
                        thread_id = %thread_id,
                        missed,
                        "rollout persistence observer lagged; some events may be dropped",
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Append the rollout line(s) for one event, returning the updated turn index.
///
/// `current_turn` is advanced from turn-lifecycle events (`TurnStarted` /
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
async fn process_event_msg(
    rollout: &Arc<RolloutFileStore>,
    thread_id: &str,
    mode: EventPersistenceMode,
    mut current_turn: u32,
    msg: &EventMsg,
) -> u32 {
    // Track the active turn from lifecycle events first.
    current_turn = match msg {
        EventMsg::TurnStarted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(current_turn),
        EventMsg::TurnCompleted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(current_turn),
        EventMsg::TurnAborted(p) => p.turn.id.parse::<u32>().ok().unwrap_or(current_turn),
        _ => current_turn,
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
            // compaction writes its own Compacted line in Slice 6.
            let payload = CompactedPayload {
                thread_id: thread_id.to_owned(),
                compacted_messages: Vec::new(),
                removed_messages: params.removed_messages.unwrap_or(0),
                output_tokens: params.output_tokens.unwrap_or(0),
                status: params.status.clone().unwrap_or_else(|| "compacted".to_owned()),
                turn_index: current_turn,
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

    current_turn
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::port::AgentNotifyPort;
    use slab_agent::protocol::{
        ContextCompactedParams, ItemCompletedParams, Turn, TurnCompletedParams, TurnStartedParams,
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
                let lines = slab_agent_rollout::read_rollout_lines(&rollout.path_for("t"));
                if lines.iter().any(|l| matches!(l.item, RolloutItem::Compacted(_))) {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await;
        assertion.expect("observer did not persist compacted marker in time");

        let lines = slab_agent_rollout::read_rollout_lines(&rollout.path_for("t"));
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
                if slab_agent_rollout::read_rollout_lines(&rollout.path_for("t"))
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
                if slab_agent_rollout::read_rollout_lines(&rollout.path_for("t"))
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
