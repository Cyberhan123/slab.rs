//! `slab-agent-rollout` — append-only JSONL event-source for slab agent sessions.
//!
//! This crate is the **L1 true source**: each thread owns one
//! `<app_home>/sessions/<thread_id>.rollout.jsonl` file containing a flat
//! sequence of [`RolloutLine`]s. It depends *upward* on `slab-agent` (for
//! [`TurnItem`] / [`EventMsg`]) and `slab-types` (for [`ConversationMessage`]);
//! `slab-agent` never depends on it, keeping the agent core pure.
//!
//! Layout:
//! - [`item`] — the wire types ([`RolloutItem`], [`RolloutLine`], [`SessionMeta`],
//!   [`CompactedPayload`], [`TurnContextPayload`]).
//! - [`policy`] — [`EventPersistenceMode`] filtering of [`EventMsg`]s.
//! - [`projection`] — [`TurnItem`] ↔ [`ConversationMessage`] projection.
//! - [`writer`] — [`JsonlWriter`] + atomic file replacement.
//! - [`reader`] — fault-tolerant line reader ([`read_rollout_lines`]).
//! - [`recorder`] — single-writer actor ([`RolloutRecorderHandle`]).
//! - [`store`] — [`RolloutStore`] trait + [`RolloutFileStore`].
//!
//! [`TurnItem`]: slab_agent::protocol::TurnItem
//! [`EventMsg`]: slab_agent::protocol::EventMsg
//! [`ConversationMessage`]: slab_types::ConversationMessage

#![deny(rust_2018_idioms)]

pub mod error;
pub mod item;
pub mod policy;
pub mod projection;
pub mod reader;
pub mod recorder;
pub mod store;
pub mod writer;

pub use error::{Result, RolloutError};
pub use item::{CompactedPayload, RolloutItem, RolloutLine, SessionMeta, TurnContextPayload};
pub use policy::EventPersistenceMode;
pub use projection::{
    conversation_message_to_turn_item, is_tool_result, turn_item_to_conversation_message,
};
pub use reader::{RolloutReader, read_rollout_lines};
pub use recorder::{RolloutCmd, RolloutRecorderHandle, RolloutRecorderParams};
pub use store::{RolloutFileStore, RolloutStore};
pub use writer::JsonlWriter;
