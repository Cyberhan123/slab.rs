# 收缩 `slab-runtime-core` / `slab-runtime-macros` 暴露面的计划

## Summary
- 本次只修改 `crates/slab-runtime-core` 和 `crates/slab-runtime-macros`，不迁移 `bin/slab-runtime`、`bin/slab-server`、`crates/slab-app-core` 的调用代码。
- 目标是把 `slab-runtime-core` 收缩为 backend worker/thread runtime 底座，只保留 `bin/slab-runtime/src/infra/backends` 当前需要的协议、worker runner、注册入口、payload/stream/error 基础类型。
- 其它层继续引用 `Orchestrator`、`PipelineBuilder`、task 状态、domain/application 错误等 API 的地方会被有意打破，并作为后续迁移清单记录。

## Key Changes
- `slab-runtime-core::backend` should continue to expose the typed handler surface used by `#[backend_handler]`: `Input`, `Options`, `BroadcastSeq`, `ControlOpId`, route-table types, and the event/runtime/peer extractor helpers.
- `slab-runtime-macros` should keep `#[backend_handler]` stable while supporting `#[on_event]`, `#[on_runtime_control]`, `#[on_peer_control]`, and `#[on_control_lagged]` with typed extractors. Only event handlers are reply-producing; control handlers stay `()` / `Result<(), E>`.
- `slab-runtime-core` 公开面保留 `backend` facade，并确保现有 backend worker 可继续使用这些类型：`BackendRequest`、`BackendReply`、`BackendOp`、`RequestRoute`、`RuntimeControlSignal`、`PeerWorkerCommand`、`WorkerCommand`、`SyncMessage`、`DeploymentSnapshot`、`StreamChunk`、`StreamHandle`、`SharedIngressRx`、`RuntimeWorkerHandler`、`spawn_runtime_worker`、`spawn_workers`、`spawn_dedicated_workers`、`ResourceManager`、`ResourceManagerConfig`。
- 保留根级 `Payload` 和 `CoreError` re-export，作为 backend-facing 兼容入口；同时在 crate README 和代码注释中明确它们不是 runtime domain/application 通用错误或 DTO。
- 移除或不再公开 `scheduler` facade：`Orchestrator`、`PipelineBuilder`、`CpuStage`、`GpuStage`、`GpuStreamStage`、`Stage`、`TaskId`、`TaskStatus`、`StageStatus`、`TaskStatusView`、`ResultStorage` 不再属于 `slab-runtime-core` 的公共 API。
- 清理 `CoreError` 的职责边界：保留 backend/worker/runtime 底座仍需要的错误 variants，去掉或隐藏明显属于 runtime domain/application 的 variants，例如 model resolution、capability validation、task decode、task lifecycle 等。
- `slab-runtime-core` 删除未使用的 `slab-types` 依赖；保留 worker runtime 实际需要的 `tokio`、`async-trait`、`bytes`、`serde`、`serde_json`、`thiserror`、`tracing`。
- `slab-runtime-macros` 保持 `#[backend_handler]` 宏名和 generated path `::slab_runtime_core::backend::*` 不变，避免破坏现有 backend handler impl。
- `slab-runtime-macros` README 更新为“backend worker handler 宏”，不再描述为 `slab-runtime-core` 内部宏。

## Expected Breakage Outside Scope
- `bin/slab-runtime` bootstrap 会因为 `slab_runtime_core::scheduler::Orchestrator` 不再公开而失败。
- `bin/slab-runtime` domain 层会因为 `PipelineBuilder`、`CpuStage`、`TaskStatus`、`TaskStatusView`、`Orchestrator`、部分 `CoreError` variants 不再公开而失败。
- `bin/slab-runtime` application/API 层会因为继续把 `CoreError` 当 runtime application/domain error 使用而失败，尤其是 `CoreError -> tonic::Status` 映射。
- `bin/slab-server` 会因为 `ServerError::Runtime(#[from] slab_runtime_core::CoreError)` 和对 `CoreError` variants 的匹配不再成立而失败。
- `crates/slab-app-core` 会因为 `AppCoreError::Runtime(#[from] slab_runtime_core::CoreError)` 不再符合新边界而失败。
- 这些破坏不在本次修复；后续应在 `bin/slab-runtime` 内建立自己的 orchestrator/task/domain error，再让 server/app-core 依赖上层错误或 gRPC/proto 状态，而不是 backend worker core。

## Test Plan
- `cargo check -p slab-runtime-core` 必须通过。
- `cargo test -p slab-runtime-core` 必须通过，测试范围聚焦 backend protocol、worker runner、control bus、peer/self-echo filtering、dispatch 行为。
- `cargo test -p slab-runtime-macros` 必须通过，UI tests 更新为匹配新的 backend-only facade。
- `cargo check -p slab-runtime` 预期失败；记录失败符号和路径作为后续迁移输入。
- `cargo check -p slab-server` 和 `cargo check -p slab-app-core` 预期失败；记录它们对 `CoreError` 的残留依赖。

## Assumptions
- 本次允许 workspace 整体暂时不编译，只要求 `slab-runtime-core` 和 `slab-runtime-macros` 自身自洽。
- 本次不修改 `bin/slab-runtime/src/infra/backends`，因此必须保留它当前使用的 backend-facing 类型名和宏入口。
- Rust 外部 crate 无法按目录限制 public API 只给 `bin/slab-runtime/src/infra/backends` 使用；本次通过收缩 API 内容和文档边界达成语义限制，真正的路径级限制需要后续把底座移入 `bin/slab-runtime` 内部模块并使用 `pub(in crate::infra::backends)`。
