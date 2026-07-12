//! `Event` / `EventMsg` — re-exported from [`slab_agent::protocol`].
//!
//! Definitions live in `slab_agent::protocol::event`. The `into_notification`
//! lift (which returns the wire [`crate::harness::ServerNotification`]) moved
//! to the server-side projection (`event_msg_to_notification`) so the semantic
//! types here stay free of the wire-envelope coupling.

pub use slab_agent::protocol::{Event, EventMsg, TurnAbortedParams};
