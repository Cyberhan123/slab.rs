# Code Quality Review: Runtime Crates Post-Restructure

**Date:** 2026-04-20
**Scope:** `slab-proto`, `slab-runtime-core`, `slab-runtime-macros`, `slab-runtime`
**Methodology:** 5-agent parallel review team covering architecture, stability, performance, DX, and DDD design

---

## Executive Summary

| Crate | Architecture | Stability | Performance | DX & Style | Overall |
|---|---|---|---|---|---|
| `slab-proto` | Good | Good | Excellent | Good | **Good (75/100)** |
| `slab-runtime-core` | Good | Needs Improvement | Needs Improvement | Needs Improvement | **Needs Improvement** |
| `slab-runtime-macros` | Excellent | Good | Excellent | Needs Improvement | **Good (3.5/5)** |
| `slab-runtime` | Good | Needs Improvement | Needs Improvement | Needs Improvement | **Needs Improvement** |
| `slab-runtime` (DDD) | B+ | — | — | — | **Good (B+)** |

**Critical findings:** 3 race conditions / unsafe patterns, 60+ `unwrap()` calls in production paths, missing API documentation across all crates.

**Top-priority actions:**
1. Fix race condition in `ResourceManager` lock poisoning (`slab-runtime-core/admission.rs`)
2. Fix race condition in `Orchestrator` cancellation check (`slab-runtime/orchestrator.rs:128-132`)
3. Remove `unsafe impl Send + Sync` in GGML engine without proper synchronization guarantees

---

## Part 1: `crates/slab-proto`

### 1.1 Architecture Assessment

**Strengths:**
- Clean backend-scoped proto file organization: `ggml/` (3 files), `candle/` (2 files), `onnx.proto`, `common.proto`
- Shared `common.proto` provides reusable types (`StringList`, `BinaryPayload`, `Usage`, `RawImage`, `RawTensor`) without coupling
- Unified `package slab.ipc.v1;` namespace across all files
- `build.rs` correctly uses `protoc-bin-vendored` for reproducible builds

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| P-ARCH-1 | Major | `candle/transformers.proto:7-15` | `CandleTransformersService` mixes Llama, Whisper, and other models in one service — should be split per model type like GGML |
| P-ARCH-2 | Major | All proto files | No service-level versioning. Service names like `GgmlLlamaService` don't indicate version, making future breaking changes harder |
| P-ARCH-3 | Minor | All proto files | `common.proto` imports are clean, but no `reserved` field ranges declared for future compatibility |

### 1.2 Stability Assessment

**Strengths:**
- `build.rs` properly propagates errors — no `unwrap()` or `panic!` found
- Unsafe block in `build.rs:15-17` (protoc vendored env var) has safety comment

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| P-STB-1 | Minor | `build.rs:15-17` | Safety comment explains general intent but not why `set_var` is safe in the build script context (single-threaded) |
| P-STB-2 | Minor | All `.proto` | No reserved field numbers — field removal without `reserved` ranges risks wire-format collisions |

### 1.3 Performance Assessment

**Strengths:**
- Binary payloads use `bytes` type (zero-copy in prost)
- Streaming support for chat responses
- Appropriate scalar types (`uint64` for timestamps)

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| P-PERF-1 | Info | `build.rs:20` | `tonic_prost_build::configure()` doesn't enable lightweight mode — consider `.lightweight()` for smaller generated code |

### 1.4 DX & Style Assessment

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| P-DX-1 | Major | All `.proto` | No documentation comments in any proto file. Complex messages like `GgmlWhisperVadParams` lack field-level docs |
| P-DX-2 | Minor | All services | Mixed RPC naming styles: imperative ("Chat") vs descriptive ("RunText") — should standardize to one convention |

### 1.5 Proto Summary & Actions

| Priority | Action |
|---|---|
| 1 (Major) | Add documentation comments to all proto messages and fields |
| 2 (Major) | Split `CandleTransformersService` into per-model services |
| 3 (Minor) | Standardize RPC naming to imperative style |

---

## Part 2: `crates/slab-runtime-core`

### 2.1 Architecture Assessment

**Strengths:**
- Public API surface correctly exposes only `pub mod backend`, `CoreError`, and `Payload` — no scheduler internals leaked
- Clean `base/` vs `internal/` module separation
- `BackendRequest`/`BackendReply` provides typed request/response protocol with route dispatch

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| RC-ARCH-1 | Major | `internal/scheduler/backend/admission.rs` | `InferenceLease` and `ManagementLease` are public types but should remain private — they are implementation details of resource management |
| RC-ARCH-2 | Minor | `lib.rs` | `ResourceManagerConfig` not re-exported but essential for configuration — callers must construct it through less ergonomic paths |
| RC-ARCH-3 | Info | `internal/scheduler/backend/runner.rs` | No unified `Backend` trait exists — dispatch is via `RuntimeWorkerHandler` + `WorkerRouteTable`. Consider if a trait abstraction would improve extensibility |

### 2.2 Stability Assessment

**Strengths:**
- All flume channel `Disconnected` errors properly handled
- Cancellation via `watch` receivers in `BackendRequest` is correct
- `CoreError` covers all error paths with thiserror

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RC-STB-1** | **Critical** | `admission.rs:126, 134, 149` | **Inconsistent lock poisoning handling**: `register_backend` panics, `handle()` returns `CoreError::InternalPoisoned`, `backend_ids()` returns empty list. Pick one strategy (recover or panic) and apply consistently |
| RC-STB-2 | Major | `runner.rs:163-167` | `dispatch_backend_request` has unhandled case for unknown operation type — should return structured error |
| RC-STB-3 | Major | `runner.rs:287-306` | `spawn_dedicated_runtime_worker` runtime construction failure not gracefully handled |
| RC-STB-4 | Minor | `admission.rs:196-201` | Potential deadlock in `acquire_inference_lease` if lock acquire order changes across call sites |
| RC-STB-5 | Info | `admission.rs` | Only 2 tests for `ResourceManager` — critical component needs more coverage |

### 2.3 Performance Assessment

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RC-PERF-1** | **Critical** | `admission.rs:102-106` | Ingress queue is shared among all workers with no per-worker isolation — high-contention bottleneck under load |
| RC-PERF-2 | Major | `admission.rs` (entire) | Single `RwLock<HashMap>` for entire backend registry. Full map lock for every read/write operation. Consider `DashMap` or per-backend locks |
| RC-PERF-3 | Minor | `base/types.rs` | `Payload::json` creates new `serde_json::Value` on every response. `StreamChunk::Token(String)` allocates per chunk |
| RC-PERF-4 | Info | `admission.rs:198-199` | Two separate async operations for lease acquisition (`acquire_compute_permit` + lock read) — consider coalescing |

### 2.4 DX & Style Assessment

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RC-DX-1** | **Critical** | `base/error.rs`, `backend.rs` | Public APIs completely undocumented — no `///` comments on `CoreError` variants, `ResourceManager` methods, or `RuntimeWorkerHandler` trait |
| RC-DX-2 | Minor | `handler.rs` | Extractor functions have similar error handling patterns that could be consolidated |
| RC-DX-3 | Minor | Multiple files | `#[allow(dead_code)]` annotations suggest unused functionality that should be removed or documented |

### 2.5 Core Summary & Actions

| Priority | Action |
|---|---|
| 1 (Critical) | Fix inconsistent lock poisoning handling in `ResourceManager` — pick one strategy and apply uniformly |
| 2 (Critical) | Add `///` documentation to all public types: `CoreError` variants, `ResourceManager` methods, `RuntimeWorkerHandler` |
| 3 (Major) | Improve lock granularity — replace single `RwLock<HashMap>` with `DashMap` or per-backend locks |

---

## Part 3: `crates/slab-runtime-macros`

### 3.1 Architecture Assessment

**Strengths:**
- Correctly targets `::slab_runtime_core::backend::*` for generated code (lines 112, 121, 130, 294, 317, 324)
- Proper abstraction level — validates input, generates route tables, provides helper methods
- Clean separation of handler types (event, runtime control, peer control, lagged) with distinct validation logic

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| RM-ARCH-1 | Info | `lib.rs` | Good normalization functions (`normalize_event_path`, etc.) — path expansion is clean and consistent |

### 3.2 Stability Assessment

**Strengths:**
- No `unwrap()` or `panic!` in proc macro code — all parse errors use `syn::Error::new()` with appropriate spans
- Comprehensive validation: async requirement, method signatures, return types, duplicate handlers

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| RM-STB-1 | Minor | `lib.rs:646-651, 701-705, 770-774` | Early return on extraction failure without continuing to extract remaining arguments — intentional but could be surprising |

### 3.3 Performance Assessment

**Strengths:**
- Efficient token generation with `quote!` macro
- Minimal allocations (HashSet for duplicate tracking is appropriate)
- Simple normalization functions

### 3.4 DX & Style Assessment

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| RM-DX-1 | Major | `lib.rs:617, 680, 691, 759, 769` | Inconsistent generated method naming: `__backend_handler_*` prefix vs snake_case peer emitter methods |
| RM-DX-2 | Major | `lib.rs` | No inline documentation for generated functions — users cannot discover what `__backend_handler_match_*` does |
| RM-DX-3 | Minor | `lib.rs:461, 474, 503` | Magic string keys for duplicate detection use `quote!(#pattern).to_string()` — fragile pattern matching |

### 3.5 Macros Summary & Actions

| Priority | Action |
|---|---|
| 1 (Major) | Add documentation for all generated functions and naming conventions |
| 2 (Minor) | Standardize generated method naming — use consistent `__backend_handler_*` prefix |
| 3 (Minor) | Add usage example in README.md |

---

## Part 4: `bin/slab-runtime` — General Quality

### 4.1 Architecture Assessment

**Strengths:**
- Clean `api → application → domain ← infra` dependency flow — no upward dependencies found
- All handlers follow `pb -> dto -> application service -> dto -> pb` pattern
- GGML, Candle, and ONNX backends are properly isolated
- Bootstrap layer correctly wires dependencies

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RT-ARCH-1** | **Critical** | `infra/backends/ggml/llama/engine.rs:171-172` | `unsafe impl Send + Sync` declared without proper synchronization guarantees. Comment claims safety but `RwLock<std::sync::Mutex<SessionId>>` pattern is questionable |
| RT-ARCH-2 | Major | `bootstrap/server.rs` | Tightly couples IPC/TCP setup logic with server configuration — should be separated for testability |
| RT-ARCH-3 | Minor | `domain/runtime/storage.rs` | Atomic counter for task IDs without wraparound protection |

### 4.2 Stability Assessment

**Strengths:**
- No `panic!`/`unreachable!` in hot request paths
- Structured error hierarchy with thiserror
- Graceful shutdown with multiple termination sources

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RT-STB-1** | **Critical** | `domain/runtime/orchestrator.rs:128-132` | **Race condition**: task could start execution after cancellation check but before status update. Needs atomic state transition |
| RT-STB-2 | Major | `domain/runtime/storage.rs` | No task purging mechanism — memory leak in long-running processes |
| RT-STB-3 | Major | `domain/services/driver_runtime.rs:19` | `Arc<Mutex<bool>>` for loaded state is overly complex and introduces unnecessary lock contention |
| RT-STB-4 | Major | Multiple files | Lock poison errors terminate the process rather than attempting recovery |
| RT-STB-5 | Minor | `bootstrap/signals.rs:79` | Logging errors to `log!` but returning `()` — shutdown issues could be masked |

### 4.3 Performance Assessment

**Strengths:**
- Minimal DTOs without business logic
- Proper streaming for response types
- Good session management with snapshot reuse in GGML

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| RT-PERF-1 | Major | `infra/backends/*/worker.rs` | Multiple `tokio::task::block_in_place()` calls can starve async runtime under load |
| RT-PERF-2 | Major | `domain/services/driver_runtime.rs:49` | `Arc<str>` for `capability_id` where `&'static str` would suffice |
| RT-PERF-3 | Minor | `application/dtos/` | `Option<Vec<String>>` for optional fields causes unnecessary allocations |
| RT-PERF-4 | Minor | Multiple files | Hard-coded channel buffer sizes (e.g., `mpsc::channel(32)`) without configuration |

### 4.4 DX & Style Assessment

**Strengths:**
- Consistent naming across layers
- Excellent file organization with clear boundaries

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **RT-DX-1** | **Critical** | Multiple files | **60+ `unwrap()` calls** in production code paths. Every one is a potential panic. All must be replaced with proper error handling |
| RT-DX-2 | Major | Entire crate | No integration tests for end-to-end workflows |
| RT-DX-3 | Major | `infra/backends/` | Significant code duplication across GGML, Candle, and ONNX backend implementations |
| RT-DX-4 | Minor | Entire crate | No README or development guide |

### 4.5 Runtime Quality Summary & Actions

| Priority | Action |
|---|---|
| 1 (Critical) | Audit and replace all 60+ `unwrap()` calls with proper error propagation |
| 2 (Critical) | Fix race condition in `Orchestrator::execute` (cancellation check vs status update) |
| 3 (Major) | Add integration tests for end-to-end inference workflows |

---

## Part 5: `bin/slab-runtime` — DDD Design Review

### 5.1 Anti-Corruption Layer & Bounded Context

**Score: High (Good)**

| Check | Result | Evidence |
|---|---|---|
| Domain layer references proto types? | PASS | No `slab_proto` imports in `domain/` files |
| ACL conversion location | PASS | All `pb <-> dto` conversions in `application/dtos/` and `api/handlers/` |
| Cross-aggregate state mutation | PASS | Aggregates don't directly modify each other's state |
| Dependency inversion | PASS | Domain defines `TaskCodec` trait; infra implements it |

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| DDD-ACL-1 | Major | `domain/models/` | `backend_payload.rs` and `enabled_backends.rs` are cross-layer types that arguably belong outside the pure domain model |
| DDD-ACL-2 | Minor | `domain/ → slab-runtime-core` | Bidirectional dependency through shared kernel should be documented explicitly |

### 5.2 Aggregate Integrity

**Score: Medium (Needs Improvement)**

| Check | Result | Evidence |
|---|---|---|
| Modifications through aggregate root? | PASS | All task modifications go through `Orchestrator` |
| Small aggregate principle? | FAIL | `Orchestrator` is a god-object handling 5+ responsibilities |
| Invariant enforcement? | PARTIAL | `TaskStatus::is_terminal()` validates transitions, but logic is scattered |

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| **DDD-AGG-1** | **Major** | `domain/runtime/orchestrator.rs` | `Orchestrator` violates SRP — handles task submission (218-238), execution (112-216), backend management (247-269), and result storage (279-369). Split into `TaskManager` and `ResourceManager` aggregates |
| DDD-AGG-2 | Minor | `domain/runtime/storage.rs:12-17` | `TaskRecord` contains both state and execution data, violating aggregate cohesion |
| DDD-AGG-3 | Minor | `domain/runtime/` | Task cancellation logic duplicated in orchestrator (127-132) and storage |

### 5.3 Anemic Domain Model Check

**Score: Medium (Mixed)**

| Check | Result | Evidence |
|---|---|---|
| Setter injection? | PASS | No public `set_xxx()` methods on domain entities |
| Value object immutability? | PASS | `EnabledBackends`, contracts are properly immutable |
| Domain services carry entity logic? | PARTIAL | Services are thin facades over infrastructure |

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| DDD-ANM-1 | Major | `domain/services/ggml_llama_service.rs:27-51` | Domain services are data containers with validation — they delegate to `DriverRuntime` rather than containing real business logic |
| DDD-ANM-2 | Major | `infra/backends/` | Model loading and inference logic lives in infrastructure instead of domain services. Domain services act as thin facades |
| DDD-ANM-3 | Minor | `domain/runtime/types.rs:7-16` | `TaskStatus` could encapsulate state transition rules rather than relying on external checks |
| DDD-ANM-4 | Minor | `domain/services/helpers.rs` | Procedural helper functions rather than domain-method patterns |

### 5.4 Ubiquitous Language

**Score: High (Good)**

| Check | Result | Evidence |
|---|---|---|
| Consistent terminology? | PASS | "orchestrator", "pipeline", "stage", "backend", "inference" used consistently |
| Service ID naming? | PASS | Consistent patterns: `ggml.llama`, `candle.whisper` |
| Context-appropriate? | PASS | Naming fits "runtime worker" bounded context |

**Findings:**

| ID | Severity | File | Finding |
|---|---|---|---|
| DDD-LANG-1 | Minor | Cross-cutting | "Payload" overloaded — refers to both transport data and domain structure |
| DDD-LANG-2 | Minor | Cross-cutting | "Stage" used differently in pipeline context vs GPU/CPU stage context |

### 5.5 Layered Architecture Diagram

```
┌──────────────────────────────────────────────────────┐
│                    API Layer                          │
│  src/api/handlers/                                   │
│  gRPC handlers: pb → dto conversion only             │
└──────────────────────┬───────────────────────────────┘
                       │ DTOs
                       ▼
┌──────────────────────────────────────────────────────┐
│                Application Layer                     │
│  src/application/{dtos, services}/                   │
│  Use case orchestration, proto ↔ domain mapping      │
└──────────────────────┬───────────────────────────────┘
                       │ domain types
                       ▼
┌──────────────────────────────────────────────────────┐
│                  Domain Layer                        │
│  src/domain/{models, runtime, services}/             │
│  Aggregate roots: Orchestrator, Task                 │
│  Value objects: EnabledBackends, Contracts           │
│  Domain services: ExecutionHub, DriverRuntime        │
│  Interfaces: TaskCodec                               │
│  ⚠ NO proto/infra dependencies                       │
└──────────────────────┬───────────────────────────────┘
                       │ implements interfaces
                       ▼
┌──────────────────────────────────────────────────────┐
│              Infrastructure Layer                    │
│  src/infra/{backends, config}/                       │
│  GGML / Candle / ONNX concrete implementations      │
└──────────────────────┬───────────────────────────────┘
                       │ wires everything
                       ▼
┌──────────────────────────────────────────────────────┐
│                Bootstrap Layer                       │
│  src/bootstrap/                                      │
│  CLI, server startup, signals, telemetry            │
└──────────────────────────────────────────────────────┘

        All layers ← slab-runtime-core (shared kernel)
```

### 5.6 DDD Summary & Actions

| Priority | Action |
|---|---|
| 1 (Major) | Split `Orchestrator` into `TaskManager` (lifecycle) and `ResourceManager` (backend management) aggregates |
| 2 (Major) | Enrich domain services with actual business logic — move validation and orchestration out of infrastructure |
| 3 (Medium) | Strengthen `TaskStatus` value object with encapsulated state transition rules |

---

## Consolidated Finding Matrix

### Critical (Must Fix)

| ID | Crate | File | Finding |
|---|---|---|---|
| RC-STB-1 | runtime-core | `admission.rs:126,134,149` | Inconsistent lock poisoning — panic vs empty vs error |
| RC-PERF-1 | runtime-core | `admission.rs:102-106` | Shared ingress queue — high-contention bottleneck |
| RT-ARCH-1 | runtime | `ggml/llama/engine.rs:171-172` | Unsafe `Send + Sync` without synchronization guarantees |
| RT-STB-1 | runtime | `orchestrator.rs:128-132` | Race condition: cancellation check vs execution start |
| RT-DX-1 | runtime | Multiple files | 60+ `unwrap()` calls in production paths |
| RC-DX-1 | runtime-core | Public API surface | Zero documentation on public types |

### Major (Should Fix)

| ID | Crate | File | Finding |
|---|---|---|---|
| P-ARCH-1 | proto | `candle/transformers.proto` | Mixed-model service should be split |
| P-DX-1 | proto | All `.proto` | No documentation comments |
| RC-ARCH-1 | runtime-core | `admission.rs` | `InferenceLease`/`ManagementLease` should be private |
| RC-STB-2 | runtime-core | `runner.rs:163-167` | Unknown operation type not handled |
| RC-PERF-2 | runtime-core | `admission.rs` | Single `RwLock<HashMap>` bottleneck |
| RT-STB-2 | runtime | `storage.rs` | No task purging — memory leak |
| RT-PERF-1 | runtime | `infra/backends/*/worker.rs` | `block_in_place()` can starve runtime |
| RT-DX-2 | runtime | Entire crate | No integration tests |
| RT-DX-3 | runtime | `infra/backends/` | Code duplication across backends |
| DDD-AGG-1 | runtime | `orchestrator.rs` | God-object orchestrator violates SRP |
| DDD-ANM-1 | runtime | `domain/services/` | Domain services are thin facades, not rich behavior |

### Minor / Info (Nice to Have)

| ID | Crate | Finding |
|---|---|---|
| P-ARCH-3 | proto | No reserved field ranges |
| P-DX-2 | proto | Mixed RPC naming styles |
| RC-STB-4 | runtime-core | Potential deadlock if lock order changes |
| RM-DX-1 | macros | Inconsistent generated method naming |
| DDD-LANG-1 | runtime | "Payload" term overloaded |
| RT-PERF-3 | runtime | `Option<Vec<String>>` unnecessary allocations |

---

## Recommended Remediation Plan

### Phase 1 — Critical Fixes (Week 1)
1. Fix `ResourceManager` lock poisoning strategy — pick recovery or panic consistently
2. Fix `Orchestrator` race condition — use atomic state transitions (e.g., `compare_exchange`)
3. Audit and replace all `unwrap()` calls with `?` or explicit error handling
4. Remove `unsafe impl Send + Sync` from GGML engine or add proper synchronization
5. Add `///` documentation to all public types in `slab-runtime-core`

### Phase 2 — Architecture Improvements (Week 2-3)
1. Split `CandleTransformersService` into per-model proto services
2. Split `Orchestrator` into `TaskManager` + `ResourceManager` aggregates
3. Replace single `RwLock<HashMap>` with `DashMap` or per-backend locks
4. Add per-worker ingress queues to eliminate shared queue contention
5. Enrich domain services with actual business logic

### Phase 3 — Quality & DX (Week 4)
1. Add proto file documentation comments
2. Add integration tests for end-to-end inference workflows
3. Reduce code duplication across backend implementations
4. Standardize generated macro method naming
5. Add task purging mechanism in storage
6. Address `block_in_place()` usage in async workers

---

## Appendix: Review Methodology

This review was conducted by a 5-agent parallel team:
- **proto-reviewer**: Proto contract architecture, stability, performance, DX
- **core-reviewer**: Runtime core concurrency safety, API surface, error handling
- **macros-reviewer**: Proc-macro correctness, error reporting, generated code quality
- **runtime-ddd-reviewer**: DDD design (ACL, aggregates, domain model, ubiquitous language)
- **runtime-quality-reviewer**: General architecture, stability, performance, DX

Each agent read all relevant source files in their assigned crate and produced findings with severity levels, file paths, and line numbers.
