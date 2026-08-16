//! REMOVED (2026-07-08): response_json persistence.
//!
//! The slab-owned control plane now stores only complete-format messages and
//! turn state (see `AgentStorePort`). The OpenAI-Responses-canonical `Response`
//! JSON is no longer persisted per run, so the `ResponsePersistenceObserver`
//! that assembled and stored it has been deleted. This file is kept as a
//! placeholder; it is not declared as a module in `super::mod`.
