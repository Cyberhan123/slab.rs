//! `TurnItem` — re-exported from [`slab_agent::protocol`].
//!
//! The definition lives in `slab_agent::protocol::item`; this re-export keeps
//! the historical `slab_proto::harness::item::*` (and `slab_proto::harness::*`)
//! paths resolving unchanged. The on-the-wire format is byte-identical.

pub use slab_agent::protocol::{ReasoningText, TurnItem, UserMessageContent};
