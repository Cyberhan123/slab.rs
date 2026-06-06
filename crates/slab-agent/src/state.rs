//! Explicit state machines for agent threads and tool calls.

use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use slab_types::agent::{AgentThreadStatus, ToolCallStatus};

use crate::error::AgentError;

pub(crate) struct ThreadStateMachine {
    status: Mutex<AgentThreadStatus>,
    status_tx: watch::Sender<AgentThreadStatus>,
}

impl ThreadStateMachine {
    pub(crate) fn new() -> (Arc<Self>, watch::Receiver<AgentThreadStatus>) {
        let initial = AgentThreadStatus::Pending;
        let (status_tx, status_rx) = watch::channel(initial);
        (Arc::new(Self { status: Mutex::new(initial), status_tx }), status_rx)
    }

    pub(crate) fn status(&self) -> AgentThreadStatus {
        *self.status.lock().expect("thread state mutex poisoned")
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<AgentThreadStatus> {
        self.status_tx.subscribe()
    }

    pub(crate) fn transition(&self, next: AgentThreadStatus) -> Result<(), AgentError> {
        let mut status = self.status.lock().expect("thread state mutex poisoned");
        let current = *status;
        if current == next {
            return Ok(());
        }
        if !is_valid_thread_transition(current, next) {
            return Err(AgentError::InvalidStateTransition {
                entity: "thread",
                from: current.to_string(),
                to: next.to_string(),
            });
        }
        *status = next;
        let _ = self.status_tx.send(next);
        Ok(())
    }
}

fn is_valid_thread_transition(current: AgentThreadStatus, next: AgentThreadStatus) -> bool {
    match current {
        AgentThreadStatus::Pending => matches!(
            next,
            AgentThreadStatus::Running
                | AgentThreadStatus::Interrupting
                | AgentThreadStatus::Shutdown
        ),
        AgentThreadStatus::Running => matches!(
            next,
            AgentThreadStatus::Interrupting
                | AgentThreadStatus::Completed
                | AgentThreadStatus::Errored
                | AgentThreadStatus::Shutdown
        ),
        AgentThreadStatus::Interrupting => {
            matches!(next, AgentThreadStatus::Interrupted | AgentThreadStatus::Shutdown)
        }
        AgentThreadStatus::Interrupted
        | AgentThreadStatus::Completed
        | AgentThreadStatus::Errored
        | AgentThreadStatus::Shutdown => false,
    }
}

pub(crate) struct ToolCallStateMachine {
    status: ToolCallStatus,
}

impl ToolCallStateMachine {
    pub(crate) fn new(status: ToolCallStatus) -> Self {
        Self { status }
    }

    pub(crate) fn status(&self) -> ToolCallStatus {
        self.status
    }

    pub(crate) fn transition(
        &mut self,
        next: ToolCallStatus,
    ) -> Result<ToolCallStatus, AgentError> {
        let current = self.status;
        if current == next {
            return Ok(next);
        }
        if !is_valid_tool_transition(current, next) {
            return Err(AgentError::InvalidStateTransition {
                entity: "tool_call",
                from: current.to_string(),
                to: next.to_string(),
            });
        }
        self.status = next;
        Ok(next)
    }
}

fn is_valid_tool_transition(current: ToolCallStatus, next: ToolCallStatus) -> bool {
    match current {
        ToolCallStatus::Pending => matches!(next, ToolCallStatus::Running | ToolCallStatus::Failed),
        ToolCallStatus::Running => {
            matches!(next, ToolCallStatus::Completed | ToolCallStatus::Failed)
        }
        ToolCallStatus::Completed | ToolCallStatus::Failed => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_state_rejects_completed_to_interrupted() {
        let (state, _status_rx) = ThreadStateMachine::new();

        state.transition(AgentThreadStatus::Running).expect("running");
        state.transition(AgentThreadStatus::Completed).expect("completed");

        let err = state
            .transition(AgentThreadStatus::Interrupted)
            .expect_err("completed thread should not become interrupted");
        assert!(matches!(err, AgentError::InvalidStateTransition { entity: "thread", .. }));
    }

    #[test]
    fn thread_state_rejects_shutdown_to_running() {
        let (state, _status_rx) = ThreadStateMachine::new();

        state.transition(AgentThreadStatus::Shutdown).expect("shutdown");

        let err = state
            .transition(AgentThreadStatus::Running)
            .expect_err("shutdown thread should not restart");
        assert!(matches!(err, AgentError::InvalidStateTransition { entity: "thread", .. }));
    }

    #[test]
    fn tool_state_rejects_pending_to_completed() {
        let mut state = ToolCallStateMachine::new(ToolCallStatus::Pending);

        let err = state
            .transition(ToolCallStatus::Completed)
            .expect_err("pending tool call should not complete without running");
        assert!(matches!(err, AgentError::InvalidStateTransition { entity: "tool_call", .. }));
    }

    #[test]
    fn tool_state_allows_approval_lifecycle() {
        let mut state = ToolCallStateMachine::new(ToolCallStatus::Pending);

        assert_eq!(
            state.transition(ToolCallStatus::Running).expect("running"),
            ToolCallStatus::Running
        );
        assert_eq!(
            state.transition(ToolCallStatus::Completed).expect("completed"),
            ToolCallStatus::Completed
        );
    }
}
