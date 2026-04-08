# slab-runtime-core 重构指南 / slab-runtime-core Refactoring Guide

本文档记录了 `slab-runtime-core` 和 `slab-runtime-macros` 的重构历程、设计决策以及后续阶段指引。

---

## 已完成阶段 (Completed Phases)

### Phase 1: 删除 `TaskKind`，统一使用 `Capability`

**问题：** `task_kind.rs` 定义了 `TaskKind` 枚举，与 `model.rs` 中的 `Capability` 完全 1:1 重复，包括从 `Capability` 到 `TaskKind` 的转换以及反向查找方法 `task_kind()` 。

**改动：**
- 删除 `mod task_kind` 及 `task_kind.rs` 的编译入口（文件保留但不再被 `lib.rs` 引用）
- `dispatch/plan.rs`：所有 `TaskKind` 参数改为 `Capability`，`task_kind.capability()` 简化为直接使用 `capability`
- `dispatch/planner.rs`：`DriverResolver::resolve(task_kind)` → `resolve(capability)`
- `api/pipeline.rs`：所有 `TaskKind::X` 替换为 `Capability::X`；`ensure_loaded_for(task_kind)` → `ensure_loaded_for(capability)`；`spec.task_kind()` → `spec.capability`

**收益：**
- 消除了 42 行纯重复代码
- 调用路径更直观：`Capability` 贯穿整个 dispatch 层，无需中间转换

---

### Phase 2: 删除 `RuntimeError` 别名，统一使用 `CoreError`

**问题：** `internal/scheduler/types.rs` 定义了 `pub type RuntimeError = CoreError`，使代码读者误以为存在两个独立错误类型，增加了理解负担。

**改动：**
- 从 `types.rs` 删除 `RuntimeError` 类型别名
- 在 `stage.rs`、`kernel.rs`（已删除）、`pipeline.rs`（内部）、`orchestrator.rs`、`tests.rs`、`backend/admission.rs` 中将 `RuntimeError` 替换为 `CoreError`
- 保留 `types.rs` 中其他有实质意义的类型：`BackendLifecycleState`、`GlobalConsistencyState`、`GlobalOperationKind`

**收益：**
- 错误类型统一为 `CoreError`，全链路一致
- 减少概念混淆

---

### Phase 3: 删除 `ExecutionKernel`，方法合并入 `Orchestrator`

**问题：** `internal/scheduler/kernel.rs` 定义了 `ExecutionKernel` 结构体，它只是对 `Orchestrator` 的薄包装：所有方法均直接委托给 `Orchestrator`，唯一的增量价值是 `wait_terminal`/`wait_result`/`wait_stream` 等轮询等待方法。

**改动：**
- 将 `wait_terminal`、`wait_result`、`wait_stream` 方法及 `DEFAULT_WAIT_TIMEOUT`、`STREAM_INIT_TIMEOUT` 常量移至 `orchestrator.rs`
- `api/task.rs` 中 `TaskHandle` 的字段从 `kernel: ExecutionKernel` 改为 `orchestrator: Orchestrator`
- `api/runtime/registry.rs` 中移除 `Runtime::kernel()` 方法；`submit_plan` 改用 `runtime.orchestrator()`
- 从 `scheduler/mod.rs` 删除 `pub mod kernel`
- `kernel.rs` 内容替换为迁移说明注释（文件可安全删除，但工具限制无法直接删除）

**收益：**
- 减少一层包装抽象，调用链从 `TaskHandle → ExecutionKernel → Orchestrator` 简化为 `TaskHandle → Orchestrator`
- 职责更清晰：`Orchestrator` 既是调度入口，也是等待/结果提取的核心

---

### Phase 4: 消除 engine traits 的编译器警告

**问题：** `internal/engine/traits.rs` 中的 `ModelLoadConfig`、`ModelLoader`、`CausalLM` 三个 `pub(crate)` trait 从未被用作 trait object 或泛型约束，导致编译器产生 "never used" 警告。

**决策：保留 trait 定义**，这些 trait 代表推理框架的核心抽象契约（"推理框架可抽象性"）。未来可在需要多态分发时启用。

**改动：**
- 在 `traits.rs` 中为三个 trait 添加 `#[allow(dead_code)]`
- 同样在 `tensor.rs` 中为 `Tensor`/`TensorData` 相关未用成员添加 `#[allow(dead_code)]`，因为这些类型专门用于配合 trait 实现

---

## 架构现状 (Current Architecture)

### 调用路径（推理）

```
用户代码
  └── Pipeline::run_text_generation(request)
        └── ensure_loaded_for(Capability::TextGeneration, streaming)
              └── DriverResolver::resolve(spec, capability, streaming)
                    └── InvocationPlan::new(resolved, capability, ...)
        └── submit_plan(runtime, plan, codec)
              └── submit_invocation_plan → Orchestrator::submit(stages, payload)
              └── TaskHandle::new(orchestrator, task_id, codec)

用户代码
  └── TaskHandle::result()
        └── Orchestrator::wait_result(task_id, timeout)
              └── Orchestrator::wait_terminal → 轮询 get_status
              └── Orchestrator::get_result(task_id)
              └── codec.decode_result(payload)
```

### 关键模块职责

| 模块 | 职责 |
|------|------|
| `model.rs` | `ModelSpec`, `Capability`, `ModelFamily`, `ModelSource` — 模型描述 |
| `inference.rs` | 请求/响应类型 — `TextGenerationRequest`, `AudioTranscriptionRequest`, etc. |
| `api/pipeline.rs` | 高层 Pipeline API：加载、推理、任务提交 |
| `api/task.rs` | `TaskHandle<R,C>` — 已提交任务的句柄，含状态查询与结果获取 |
| `api/codec.rs` | Payload 编解码：请求序列化、响应反序列化 |
| `api/runtime/registry.rs` | `Runtime` — 持有 `Orchestrator`、`DriverResolver`，并通过 `RuntimeBuilder` 注入已解析的 backend registrations |
| `internal/dispatch/plan.rs` | `InvocationPlan`、`ResolvedDriver`、`op_name_for(Capability, streaming)` |
| `internal/dispatch/planner.rs` | `DriverResolver::resolve(spec, capability, streaming)` |
| `internal/scheduler/orchestrator.rs` | 调度核心：提交、执行、等待、取消、结果提取 |
| `internal/scheduler/storage.rs` | `ResultStorage` — 任务状态的线程安全存储 |
| `internal/scheduler/stage.rs` | `Stage` (Cpu/Gpu/GpuStream) — pipeline 阶段描述 |
| `internal/scheduler/pipeline.rs` | 内部 `PipelineBuilder` — 类型状态 builder |
| `internal/scheduler/backend/` | admission 控制、worker runner、protocol 类型 |
| `internal/engine/*/backend.rs` | 各后端 worker（llama/whisper/diffusion/onnx × ggml/candle） |

---

## 待完成阶段 (Pending Phases)

### Phase 5（可选）：压缩 `types.rs` 的 re-export 层

当前 `internal/scheduler/types.rs` 仍保留了对 `base::types::*` 的 re-export，各后端 worker 通过 `scheduler::types::Payload` 间接导入。可以改为直接从 `crate::base::types` 导入，彻底消除 re-export 中间层。

**影响范围：** 所有 `use crate::internal::scheduler::types::Payload;` 的文件（约 9 个）

**优先级：** 低（功能等价，仅减少一层路径）

---

### Phase 6（可选）：将 `ModelLoader`/`CausalLM` 实现激活为运行时多态

目前 GGML/Candle adapter 中的 `impl ModelLoader` 和 `impl CausalLM` 存在但未被使用。如果未来需要在运行时动态切换后端（e.g., `Box<dyn CausalLM>`），可以：

1. 删除各 backend worker 中对 engine 具体类型的直接调用
2. 改为通过 `dyn ModelLoader` + `dyn CausalLM` 进行多态分发
3. 更新 `dispatch/plan.rs` 中的 driver 解析逻辑以支持 trait object

**前提条件：** 确认多态分发的性能开销在推理路径上可接受。

---

### Phase 7（可选）：合并 `codec.rs` 中的 driver 字符串匹配为类型安全形式

`api/codec.rs` 中 `encode_load_payload` 函数通过 `match resolved.driver_id.as_str() { "ggml.llama" => ..., "candle.llama" => ... }` 手动映射 payload。这与 `DriverDescriptor::driver_id` 是一种隐式耦合。

可以引入一个 `PayloadEncoder` trait，让每个 backend 自己声明如何从 `ModelSpec` 构建 load payload，以此替换字符串匹配。

---

## 线程安全保证 (Thread Safety)

重构保持了原有的线程安全设计：

| 组件 | 机制 |
|------|------|
| `Orchestrator` | `Arc<RwLock<...>>` 内部状态 |
| `ResultStorage` | `Arc<RwLock<HashMap<TaskId, TaskRecord>>>` |
| `ResourceManager` | `Arc<RwLock<HashMap<...>>>` + `OwnedSemaphorePermit` |
| `TaskHandle<R,C>` | `Clone` 可跨线程持有，`codec` 为 `Arc<dyn TaskCodec>` |
| backend workers | `Mutex<mpsc::Receiver<BackendRequest>>` 竞争消费 |
| management 操作 | `OwnedRwLockWriteGuard` 独占锁，inference 持 read lock |

---

## 构建与测试 (Build & Test)

```sh
# 仅检查 slab-core（快速）
cargo check -p slab-core

# 运行 slab-core 所有测试
cargo test -p slab-core

# 检查 slab-runtime-macros
cargo check -p slab-runtime-macros
```

测试覆盖：40 个单元测试（scheduler + pipeline + dispatch + codec），全部通过。
