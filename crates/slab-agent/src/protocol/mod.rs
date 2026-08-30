//! Slab-agent's own external harness protocol (the SQ/EQ-shaped surface).
//!
//! These semantic types — [`TurnItem`], [`EventMsg`], [`Turn`], [`Thread`] and
//! the notification param structs — used to live in `slab-proto::harness`. They
//! are relocated here so slab-agent owns its external protocol directly and
//! stays free of the wire-envelope crate. `slab-proto` keeps only the JSON-RPC
//! envelope (`ServerNotification`, method constants, request/response DTOs) and
//! re-exports these semantic types for backward compatibility.
//!
//! Serde attributes are byte-identical to the previous `slab-proto` definitions,
//! so the on-the-wire format is unchanged.

pub mod error;
pub mod event;
pub mod item;
pub mod notification;
pub mod thread;
pub mod turn;

pub use error::ErrorEvent;
pub use event::{Event, EventMsg, TurnAbortedParams};
pub use item::{ReasoningText, TurnItem, UserMessageContent};
pub use notification::{
    AgentMessageDeltaParams, BackgroundTaskUpdatedParams, CommandExecutionOutputDeltaParams,
    CommandExecutionRequestApprovalParams, ContextCompactedParams, ContextCompactingParams,
    FileChangeApprovalChange, FileChangeOutputDeltaParams, FileChangeRequestApprovalParams,
    ItemCompletedParams, ItemStartedParams, MessageAppendedParams, ReasoningSummaryTextDeltaParams,
    ReasoningTextDeltaParams, ThreadStatusChangedParams, TurnCompletedParams, TurnStartedParams,
    TurnStateChangedParams, TurnUsage,
};
pub use thread::{GitInfo, Thread};
pub use turn::{Turn, TurnError};
