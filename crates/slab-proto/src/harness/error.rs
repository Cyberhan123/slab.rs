//! Error / warning payloads — re-exported from [`slab_agent::protocol`].
//!
//! Definitions live in `slab_agent::protocol::{error, turn}` (`TurnError` rides
//! with `Turn`). This re-export keeps the historical `slab_proto::harness::*`
//! paths resolving unchanged.

pub use slab_agent::protocol::{ErrorEvent, TurnError, WarningEvent};
