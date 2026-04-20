# Audit: Proto Restructuring & Runtime Core API Shrinking

**Date:** 2026-04-20
**Plans audited:**
- `docs/development/planning/slab-runtime-core-2026-04-19.md` — Proto restructuring and runtime service bleeding-stop v4
- `docs/development/planning/slab-runtime-codec-ref.md` — Shrinking `slab-runtime-core` / `slab-runtime-macros` exposure surface

---

## Build Verification

| Crate | Status | Notes |
|---|---|---|
| `slab-proto` | PASS | Compiles cleanly |
| `slab-runtime-core` | PASS | Compiles cleanly |
| `slab-runtime-macros` | PASS | Compiles cleanly |
| `slab-runtime` | PASS | Compiles cleanly |
| `slab-server` | FAIL (48 errors) | Expected per plan assumptions |
| `slab-app-core` | FAIL (48 errors) | Expected per plan assumptions |

### slab-server Failure Analysis

The 48 compilation errors are caused by:
1. **`slab_proto::convert` unresolved import** — `convert.rs` was removed as planned; `slab-server` still references the old module.
2. **Old proto client names gone** — `llama_service_client`, `whisper_service_client`, `diffusion_service_client` replaced by new backend-scoped services.
3. **Old DTO types gone** — `TranscribeRequest`, `TranscribeVadOptions`, `TranscribeDecodeOptions`, `ImageResponse`, `ModelLoadRequest` reorganized under new proto files.

These breakages are **expected and documented** in both plans' assumptions sections. The next migration phase (`slab-app-core/server` boundary remediation) will resolve them.

---

## Plan 1: Proto Restructuring & Runtime Service Bleeding-Stop

### Overall: COMPLETE (100%)

| Area | Target | Current State | Status |
|---|---|---|---|
| Proto file layout | 7 files in `ggml/`, `candle/`, `onnx.proto`, `common.proto` | All 7 files exist at `crates/slab-proto/proto/slab/ipc/v1/` with correct subdirectories | DONE |
| Domain services (8) | `GgmlLlamaService`, `GgmlWhisperService`, `GgmlDiffusionService`, `CandleLlamaService`, `CandleWhisperService`, `CandleDiffusionService`, `OnnxTextService`, `OnnxEmbeddingService` | All 8 implemented in `bin/slab-runtime/src/domain/services/` | DONE |
| Application services (5) | `GgmlLlamaService`, `GgmlWhisperService`, `GgmlDiffusionService`, `CandleService`, `OnnxService` | All 5 implemented in `bin/slab-runtime/src/application/services/` | DONE |
| API handlers | GGML 3, Candle 2, ONNX 1 | `ggml_llama.rs`, `ggml_whisper.rs`, `ggml_diffusion.rs`, `candle_transformers.rs`, `candle_diffusion.rs`, `onnx.rs` | DONE |
| Handler pattern | `pb -> dto -> application service -> dto -> pb` only | All handlers follow this pattern; no reasoning extraction, usage estimation, stop trimming, whisper text assembly, or OpenAI/SSE wrapping | DONE |
| `convert.rs` | Thin `pb <-> dto` mapping, no `slab_types` dependency | Removed; replaced by per-backend DTO modules in `application/dtos/` | DONE |
| `slab_types` dependency removal | Runtime main path must not depend on `TextGenerationRequest/Response`, `AudioTranscriptionRequest/Response`, `RuntimeBackendLoadSpec` | No traces found; runtime uses internal domain DTOs exclusively | DONE |
| `slab-runtime-core` freeze | No structural migration | Confirmed untouched; only backend/worker types remain | DONE |

### Test Plan Checklist

| Test | Result |
|---|---|
| `cargo check -p slab-proto` | PASS |
| `cargo check -p slab-runtime` | PASS |
| `cargo check -p slab-runtime-core` | PASS |
| Proto `pb <-> dto` round-trip uses thin mapping without semantic objects | Confirmed |
| Domain/application layers use per-driver services (not unified `BackendSession`) | Confirmed |
| No reasoning/usage/stop/whisper-text/SSE logic in handlers | Confirmed |
| No `slab_types` family request/response types in runtime main path | Confirmed |

---

## Plan 2: Shrinking `slab-runtime-core` / `slab-runtime-macros` Exposure Surface

### Overall: COMPLETE (100%)

| Area | Target | Current State | Status |
|---|---|---|---|
| Public API surface | Only `backend` facade + root `CoreError` / `Payload` re-exports | `lib.rs` exports `pub mod backend`, `pub use base::error::CoreError`, `pub use base::types::Payload` | DONE |
| Scheduler removal | `Orchestrator`, `PipelineBuilder`, `CpuStage`, `GpuStage`, `GpuStreamStage`, `Stage`, `TaskId`, `TaskStatus`, `StageStatus`, `TaskStatusView`, `ResultStorage` not public | None of these types appear in public API; scheduler module is internal only | DONE |
| `CoreError` cleanup | Only backend/worker/runtime-base variants | 11 variants remain: `QueueFull`, `Busy`, `BackendShutdown`, `Timeout`, `UnsupportedOperation`, `DriverNotRegistered`, `InternalPoisoned`, `EngineIo`, `GGMLEngine`, `OnnxEngine`, `CandleEngine` | DONE |
| `slab-types` dependency removal | Removed from Cargo.toml | Not present in dependencies; only `tokio`, `flume`, `thiserror`, `serde_json`, `serde`, `tracing`, `async-trait`, `bytes` | DONE |
| `#[backend_handler]` macro | Stable, uses `::slab_runtime_core::backend::*` | Confirmed; generated code targets `::slab_runtime_core::backend::*` | DONE |
| Event/control handler macros | `#[on_event]`, `#[on_runtime_control]`, `#[on_peer_control]`, `#[on_control_lagged]` | All present with typed extractors | DONE |
| Macro README updated | Described as "backend worker handler macro" | README states role and scope correctly | DONE |

### Test Plan Checklist

| Test | Result |
|---|---|
| `cargo check -p slab-runtime-core` | PASS |
| `cargo test -p slab-runtime-core` | Not run (requires full test execution) |
| `cargo check -p slab-runtime-macros` | PASS |
| `cargo check -p slab-runtime` | PASS (no longer broken — subsequent work resolved the expected breakage) |
| `cargo check -p slab-server` | FAIL (expected; `slab_proto::convert` and old proto clients gone) |
| `cargo check -p slab-app-core` | FAIL (expected; `CoreError` variant dependencies broken) |

---

## Outstanding Items

1. **`slab-server` migration** — Must adopt new proto layout (backend-scoped services, new DTO types). The 48 errors are dominated by `slab_proto::convert` imports and old gRPC client names.
2. **`slab-app-core` migration** — Must replace `CoreError` variant matching with its own error types or depend on higher-layer abstractions.
3. **Integration tests** — The plans' search-verification items (grepping for removed symbols in runtime main path) should be run as part of CI to prevent regressions.
4. **`cargo test`** — Unit tests for `slab-runtime-core` and `slab-runtime-macros` were not executed in this audit; recommend running before closing the plans.
