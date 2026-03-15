/// Base domain layer: core data structures and unified error type.
///
/// These types are dependency-free with respect to the scheduler and engine
/// layers, making them safe to reference from any module within `slab-core`.
pub mod error;
pub mod types;
