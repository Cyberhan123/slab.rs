/**
 * Harness protocol contract for the `/v1/agents/harness` WebSocket JSON-RPC
 * control plane — single entry for consumers (`@slab/api/harness`).
 *
 * Everything here is GENERATED from the authoritative Rust contract
 * (`crates/slab-proto` + `crates/slab-agent` + `crates/slab-jsonrpc` +
 * `crates/slab-types`) by `bun run gen:harness`; runtime client code lives in
 * `@slab/core/harness`. Wire fields are camelCase; optional fields are
 * omitted on the wire (not `null`).
 */

export * from "./constants"
export * from "./generated/index"
