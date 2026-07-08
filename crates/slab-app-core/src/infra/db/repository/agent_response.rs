//! REMOVED (2026-07-08): OpenAI-Responses-canonical `Response` JSON persistence.
//!
//! slab now stores only complete-format messages and turn state
//! (`AgentStorePort`); the per-run `response_json` store (`AgentResponseStore`,
//! `ThreadResponseRecord`, the `agent_thread_responses` table) is no longer
//! written or read. The SQL migration that created the table is left in place
//! (append-only) — the table simply goes unused. This file is kept as a
//! placeholder; it is not declared as a module in `super::mod`.
