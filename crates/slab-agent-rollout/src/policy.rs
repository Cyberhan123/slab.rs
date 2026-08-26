//! Event persistence policy — decides which [`EventMsg`]s are written to disk.
//!
//! Rollout files capture every [`TurnItem`](slab_agent::protocol::TurnItem) and
//! every [`crate::TurnContextPayload`], but only a *filtered* subset of
//! [`EventMsg`]s. [`EventPersistenceMode::Limited`] (the default) keeps the
//! turn-lifecycle and error events needed to reconstruct the session
//! timeline; [`EventPersistenceMode::Extended`] additionally keeps the streaming
//! deltas and approval requests for high-fidelity debugging.

use slab_agent::protocol::EventMsg;

/// How aggressively [`EventMsg`]s are persisted to the rollout file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventPersistenceMode {
    /// Persist only turn lifecycle + error + compaction events.
    ///
    /// Drops the high-frequency `*Delta` and `*RequestApproval` variants.
    #[default]
    Limited,
    /// Persist every [`EventMsg`] variant.
    Extended,
}

impl EventPersistenceMode {
    /// Returns `true` when `msg` should be appended under this mode.
    ///
    /// `EventMsg` is `#[non_exhaustive]`, so the `Limited` match keeps a `_`
    /// wildcard arm that returns `false` for any future variant — new event
    /// kinds default to *not persisted* until explicitly allow-listed here.
    pub fn should_persist(&self, msg: &EventMsg) -> bool {
        match self {
            Self::Extended => true,
            Self::Limited => matches!(
                msg,
                EventMsg::Error(_)
                    | EventMsg::TurnStarted(_)
                    | EventMsg::TurnCompleted(_)
                    | EventMsg::TurnAborted(_)
                    | EventMsg::ContextCompacting(_)
                    | EventMsg::ContextCompacted(_)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::protocol::notification::*;

    fn delta_event() -> EventMsg {
        EventMsg::AgentMessageDelta(AgentMessageDeltaParams {
            thread_id: "t".to_owned(),
            turn_id: "tu".to_owned(),
            item_id: "i".to_owned(),
            delta: "x".to_owned(),
        })
    }

    fn turn_completed_event() -> EventMsg {
        EventMsg::TurnCompleted(TurnCompletedParams {
            thread_id: "t".to_owned(),
            turn: slab_agent::protocol::Turn {
                id: "tu".to_owned(),
                status: "completed".to_owned(),
                ..Default::default()
            },
            usage: None,
            reason: None,
        })
    }

    #[test]
    fn limited_keeps_lifecycle_drops_deltas() {
        let limited = EventPersistenceMode::Limited;
        assert!(limited.should_persist(&turn_completed_event()));
        assert!(!limited.should_persist(&delta_event()));
    }

    #[test]
    fn extended_keeps_everything() {
        let extended = EventPersistenceMode::Extended;
        assert!(extended.should_persist(&delta_event()));
        assert!(extended.should_persist(&turn_completed_event()));
    }

    #[test]
    fn default_is_limited() {
        assert_eq!(EventPersistenceMode::default(), EventPersistenceMode::Limited);
    }
}
