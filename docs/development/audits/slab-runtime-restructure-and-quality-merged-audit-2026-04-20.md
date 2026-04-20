# 合并审计：`slab-runtime` 重构与代码质量复核（2026-04-20）

> 本报告合并并纠偏以下两份现有审计：
>
> - `docs/development/audits/code-quality-review-2026-04-20.md`
> - `docs/development/audits/slab-runtime-restructure-audit-2026-04-20.md`

## 范围与方法

- 范围：`crates/slab-proto`、`crates/slab-runtime-core`、`crates/slab-runtime-macros`、`bin/slab-runtime`，以及它们与 `crates/slab-app-core` / `bin/slab-server` 的当前集成边界。
- 目标：校正旧报告中的误判和过时结论，补齐当前代码库里更关键但未被完整覆盖的问题。
- 本次实际执行的验证：
  - `cargo check -p slab-proto`
  - `cargo check -p slab-runtime-core`
  - `cargo check -p slab-runtime-macros`
  - `cargo check -p slab-runtime`
  - `cargo check -p slab-app-core`
  - `cargo check -p slab-server`
  - `cargo test -p slab-runtime-core`
  - `cargo test -p slab-runtime-macros`
  - `cargo test -p slab-runtime --lib`

## 执行摘要

- `slab-proto`、`slab-runtime-core`、`slab-runtime-macros`、`slab-runtime` 当前都能独立通过 `cargo check`。
- `slab-runtime-core`、`slab-runtime-macros`、`slab-runtime --lib` 的现有测试也都通过，说明 runtime 内核侧已经明显比旧报告描述的更稳定。
- 当前真正阻断主链路的不是 runtime-core 内部 API 文档或“60+ unwrap”，而是 `crates/slab-app-core` 仍停留在旧 proto / 旧 gRPC 客户端模型上，导致 `cargo check -p slab-app-core` 失败 48 个错误，并进一步拖垮 `cargo check -p slab-server`。
- 旧两份报告有价值，但都存在“把计划假设当成当前事实”或“把低置信度判断写成既成事实”的问题；如果继续直接沿用，会误导优先级排序。

## 当前验证矩阵

| 项目 | 结果 | 说明 |
|---|---|---|
| `cargo check -p slab-proto` | PASS | 当前 proto 结构和生成流程可编译 |
| `cargo check -p slab-runtime-core` | PASS | 当前 backend facade / runner / admission 可编译 |
| `cargo check -p slab-runtime-macros` | PASS | 宏 crate 当前可编译 |
| `cargo check -p slab-runtime` | PASS | runtime 主体当前已能通过，不再符合旧计划中的“预期失败” |
| `cargo check -p slab-app-core` | FAIL | 48 个错误，核心原因是仍引用旧 proto API |
| `cargo check -p slab-server` | FAIL | 主要被 `slab-app-core` 的编译失败传导阻断 |
| `cargo test -p slab-runtime-core` | PASS | 9/9 通过 |
| `cargo test -p slab-runtime-macros` | PASS | UI 测试通过 |
| `cargo test -p slab-runtime --lib` | PASS | 36 通过，1 ignored |

## 对原两份报告的纠偏

| 原报告结论 | 判定 | 修正说明 | 依据 |
|---|---|---|---|
| `slab-runtime-core` 没有 re-export `ResourceManagerConfig` | 错误 | `ResourceManagerConfig` 已通过 backend facade 导出 | `crates/slab-runtime-core/src/backend.rs:2` |
| `dispatch_backend_request` 对 unknown op 未处理 | 错误 | 当前实现会回写 `BackendReply::error("unknown op: ...")` | `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:154`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:163` |
| `slab-runtime` “没有 README / 开发指南” | 错误 | README 已存在，但内容有文档漂移 | `bin/slab-runtime/README.md:1`, `bin/slab-runtime/README.md:12` |
| “60+ unwrap 在生产路径中” | 不可复现 | 当前树上对这几个 crate 的 `unwrap/expect` 文本命中是 45 处；其中大量在 `#[cfg(test)]` 模块。仍有少量生产路径上的 `expect`，但旧报告的规模和严重度都被高估了 | `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:357`, `bin/slab-runtime/src/infra/backends/ggml/llama/engine.rs:1149`, `bin/slab-runtime/src/infra/backends/onnx/engine.rs:187`, `bin/slab-runtime/src/infra/backends/ggml/llama/engine.rs:802`, `bin/slab-runtime/src/infra/backends/ggml/llama/engine.rs:820` |
| `slab-runtime` 仍应按计划“预期编译失败” | 已过时 | 当前 `cargo check -p slab-runtime` 已通过，说明实现已超出旧计划假设 | `docs/development/planning/slab-runtime-core-2026-04-19.md:31` |
| `slab-server` 仍直接引用旧 proto 客户端 / `slab_proto::convert` | 定位不准 | 旧引用主要残留在 `crates/slab-app-core`；`bin/slab-server/src` 本身没有这些旧符号 | `crates/slab-app-core/src/infra/rpc/client.rs:70`, `crates/slab-app-core/src/domain/services/chat/local.rs:7`, `bin/slab-server/Cargo.toml:18` |
| “`cargo test` 尚未执行” | 已过时 | 本次复核已补跑 `slab-runtime-core`、`slab-runtime-macros`、`slab-runtime --lib` 测试并通过 | 本次命令执行结果 |

## 当前确认的问题清单

### P0 / 阻断主链路

| ID | 严重级别 | 问题 | 影响 | 依据 |
|---|---|---|---|---|
| MRG-01 | Critical | `crates/slab-app-core` 仍依赖旧 proto 布局、旧 gRPC client 名称和已删除的 `slab_proto::convert` | `slab-app-core` 无法编译，`slab-server` 也因此无法通过 | `crates/slab-app-core/src/infra/rpc/client.rs:70`, `crates/slab-app-core/src/infra/rpc/client.rs:192`, `crates/slab-app-core/src/infra/rpc/client.rs:271`, `crates/slab-app-core/src/domain/services/audio.rs:42`, `crates/slab-app-core/src/domain/services/chat/local.rs:7`, `crates/slab-app-core/src/domain/services/image.rs:4`, `crates/slab-app-core/src/domain/services/video.rs:3`, `crates/slab-app-core/src/model_auto_unload.rs:6` |

### P1 / 应优先修复

| ID | 严重级别 | 问题 | 影响 | 依据 |
|---|---|---|---|---|
| MRG-02 | Major | `ResourceManager::register_backend` 不返回 `Result`，而 worker 线程 / 专用 runtime 启动失败只做日志记录 | backend 可能在注册表里“存在”，但实际上没有 worker 消费请求，最终表现为超时或假性可用 | `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:98`, `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:110`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:275`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:286`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:306`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:348` |
| MRG-03 | Major | `ResourceManager` 对 poisoned lock 的处理策略不一致：有时 panic，有时返回错误，有时静默降级为空列表 | 同一类损坏在不同调用路径上表现不一致，增加故障定位成本 | `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:110`, `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:121`, `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:131`, `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:143`, `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:145` |
| MRG-04 | Major | `DriverRuntime` 的 `loaded: Arc<Mutex<bool>>` 只表达布尔态，`unload()` 未在整个卸载期间持锁；同时 application 层会把 service clone 出去后再异步执行 | 可能出现“旧 service 仍在提交请求，但新/旧 deployment 正在切换”的窗口，特别是 load/unload 与请求并发时 | `bin/slab-runtime/src/domain/services/driver_runtime.rs:53`, `bin/slab-runtime/src/domain/services/driver_runtime.rs:60`, `bin/slab-runtime/src/domain/services/driver_runtime.rs:219`, `bin/slab-runtime/src/domain/services/driver_runtime.rs:227`, `bin/slab-runtime/src/application/services/mod.rs:61`, `bin/slab-runtime/src/application/services/mod.rs:71`, `bin/slab-runtime/src/application/services/ggml_llama_service.rs:27`, `bin/slab-runtime/src/application/services/ggml_llama_service.rs:46` |
| MRG-05 | Major | 取消语义仍然是“检查一次 + 协作式下传”，`cancel_and_purge()` 还会立即删除 task record | 取消可能发生在 pre-stage 检查之后、backend 真正处理之前；被 purge 后，调用方拿不到稳定的终态 | `bin/slab-runtime/src/domain/runtime/orchestrator.rs:126`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:134`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:240`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:244`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:306`, `bin/slab-runtime/src/domain/runtime/stage.rs:70`, `bin/slab-runtime/src/domain/runtime/stage.rs:116`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:152` |
| MRG-06 | Major | ONNX 对外暴露 `onnx.text` 和 `onnx.embedding` 两种 capability，但内部只有一个共享 deployment 槽位，且两者都绑定到同一个 backend id `onnx` | 加载 embedding 会挤掉 text，加载 text 也会挤掉 embedding；这与 capability 暴露给上层的直觉不一致 | `bin/slab-runtime/src/infra/backends/onnx/mod.rs:19`, `bin/slab-runtime/src/infra/backends/onnx/mod.rs:31`, `bin/slab-runtime/src/application/services/onnx_service.rs:77`, `bin/slab-runtime/src/application/services/onnx_service.rs:127`, `bin/slab-runtime/src/application/services/onnx_service.rs:153`, `bin/slab-runtime/src/domain/services/onnx_text_service.rs:43`, `bin/slab-runtime/src/domain/services/onnx_embedding_service.rs:51` |

### P2 / 需要补齐但不阻断

| ID | 严重级别 | 问题 | 影响 | 依据 |
|---|---|---|---|---|
| MRG-07 | Medium | 旧报告把 ingress 竞争描述成“所有 worker 全局共享队列”过于宽泛；实际问题是“每个 backend 只有一条同时承载 inference 与 management 的 ingress queue” | backlog 下 `load/unload` 无法优先，甚至会因为队列已满直接失败 | `crates/slab-runtime-core/src/internal/scheduler/backend/admission.rs:102`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:49`, `bin/slab-runtime/src/domain/runtime/stage.rs:71`, `bin/slab-runtime/src/domain/runtime/stage.rs:117` |
| MRG-08 | Medium | `ResultStorage` 只在显式 `take_result()` / `take_stream()` / `purge_task()` 时清理；终态任务没有 TTL / retention 策略 | 长时间运行后，成功或失败任务会持续占用内存 | `bin/slab-runtime/src/domain/runtime/storage.rs:21`, `bin/slab-runtime/src/domain/runtime/storage.rs:35`, `bin/slab-runtime/src/domain/runtime/storage.rs:83`, `bin/slab-runtime/src/domain/runtime/storage.rs:98`, `bin/slab-runtime/src/domain/runtime/orchestrator.rs:214` |
| MRG-09 | Medium | GGML 三个 engine 仍依赖手写 `unsafe impl Send/Sync`，虽然已有注释说明，但缺少专门的并发安全验证 | 这是需要持续跟踪的安全债务；旧报告把它写成“已证实的致命错误”过重，但完全忽略也不合理 | `bin/slab-runtime/src/infra/backends/ggml/llama/engine.rs:147`, `bin/slab-runtime/src/infra/backends/ggml/llama/engine.rs:171`, `bin/slab-runtime/src/infra/backends/ggml/whisper/engine.rs:64`, `bin/slab-runtime/src/infra/backends/ggml/whisper/engine.rs:85`, `bin/slab-runtime/src/infra/backends/ggml/diffusion/engine.rs:54`, `bin/slab-runtime/src/infra/backends/ggml/diffusion/engine.rs:75` |
| MRG-10 | Medium | `bin/slab-runtime/README.md` 已存在，但仍写着 `config/`, `context/`, `launch.rs` 旧布局，和当前 `bootstrap/api/application/domain/infra` 实现不一致 | 会误导后续维护者，也会让后续审计文档继续引用旧结构 | `bin/slab-runtime/README.md:12`, `bin/slab-runtime/src/bootstrap/mod.rs:1`, `bin/slab-runtime/src/api/handlers/mod.rs:1` |

## 本次未支持旧报告的结论

以下结论在当前代码树里要么证据不足，要么优先级被旧报告明显抬高，本报告不沿用原结论：

- “`CandleTransformersService` 必须拆成多个 proto service”：当前它在 application 层确实聚合了 llama / whisper 两条独立槽位，不足以直接证明这是结构性错误。见 `bin/slab-runtime/src/application/services/candle_transformers_service.rs:16`。
- “public API 几乎没有文档”：`slab-runtime-core` 的根导出、error、handler、runner 目前已有相当数量 `///` 注释，不符合“几乎没有”的描述。见 `crates/slab-runtime-core/src/lib.rs:11`, `crates/slab-runtime-core/src/base/error.rs:3`, `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:13`。
- “`spawn_dedicated_runtime_worker` 没有处理 runtime 构造失败”：当前已经记录错误并返回；真实问题是失败没有向注册阶段传播。见 `crates/slab-runtime-core/src/internal/scheduler/backend/runner.rs:287`。

## 修复优先级建议

### 第一阶段：先恢复主链路

1. 完成 `crates/slab-app-core` 对新 proto 的迁移：
   - 移除 `slab_proto::convert` 依赖。
   - 用 backend-scoped client 替换旧 `llama_service_client` / `whisper_service_client` / `diffusion_service_client`。
   - 用新的 `Ggml*` / `Candle*` / `Onnx*` request/response DTO 重写 `infra/rpc/client.rs` 与本地服务适配层。
2. 在这个阶段结束前，`cargo check -p slab-app-core` 与 `cargo check -p slab-server` 都应恢复为 PASS。

### 第二阶段：修 runtime 生命周期可靠性

1. 让 backend 注册成为 fail-fast：
   - `register_backend` 返回 `Result`.
   - worker/thread/runtime 启动失败必须阻断 `build_grpc_service()`。
2. 把 `DriverRuntime` 的 bool 状态升级为显式生命周期状态机：
   - `Unloaded`
   - `Loading`
   - `Loaded`
   - `Unloading`
3. 把 cancel 与 purge 拆开：
   - 先保留 task record 到终态。
   - 再由 TTL / 显式 purge 做清理。

### 第三阶段：补合同与文档

1. 决定 ONNX 到底是：
   - 一个 backend + 两个互斥 mode，还是
   - 两个独立 deployment / worker。
2. 为 `ResourceManager` 明确单一的 poisoned-lock 策略。
3. 更新 `bin/slab-runtime/README.md`，并保持与 `AGENTS.md` / `CLAUDE.md` / `.github/copilot-instructions.md` 一致。

## 结论

- 旧两份报告里，真正仍然成立的高优先级问题主要有三类：runtime/app-core 边界断裂、worker 生命周期可靠性不足、取消/清理语义不完整。
- 相比之下，`slab-runtime-core` 文档缺失、`slab-runtime` 无 README、以及“60+ unwrap”这些说法已经不符合当前代码树。
- 因此，当前最合理的修复顺序不是继续打磨 runtime-core 表层代码风格，而是先把 `slab-app-core` 的 proto 迁移补齐，恢复 `slab-server` 主链路，然后再收口 runtime 生命周期与文档债务。
