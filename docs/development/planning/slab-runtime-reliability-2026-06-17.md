# 跨进程可靠性、冗余治理与安全收敛专项设计 (2026-06-17)

> **文档定位**：本规划书基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §5 的跨模块冗余（R1/R5/R6）、§5.2 的逻辑死角与边界隐患（G1/G2/G3/G6/G7/G8/G9），以及 §2.3 F-Stack-3 的 gRPC 错误跨边界结构损失（general case），在 `slab.rs` 的跨进程层（JSON-RPC 插件宿主、gRPC runtime 网关、进程监督器）上落地一套**路径单一真源、宿主骨架去重、pending-map 有界、task 可监督、错误跨边界保结构**的契约级设计。
>
> **方法**：首席架构师主导，所有 path:line 证据由主审计员在 2026-06-18（审计发布次日）**直接读源码重新核实**——审计原文行号有 +1 ~ +9 行漂移，且发现两处需纠错的事实（见 §1.3 与对应条款）。本文引用**当前工作树行号**。
>
> **读者**：实现该规范的工程师与审计员。本文为**契约级设计**，非概念稿。
>
> **闭环定位**：本计划是审计 §6 行动表的**收口计划**（Track C：跨进程可靠性、冗余治理与安全收敛）。它覆盖 §6 中归属 Track C 的全部 P0/P1/P2 条款；与三份姊妹计划（[slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md) / 存储-契约计划 / PMID-设置计划）一起构成 §6 全表的闭环（见 §8）。

> **实施闭环（2026-06-19）**：Track C 已按当前代码完成落地。实现时修正两处计划语义：模型 `model_path`/`model_cache_dir` 只要求合法绝对路径，不纳入插件 root containment；JSON-RPC 宿主管道落在既有 `crates/slab-jsonrpc::host`，不新增 `slab-jsonrpc-host` workspace crate。G8 拆分项中，OpenAI URL 测试 fixture 已去重，runtime launch bind/base port 已补 `runtime.launch.{server,desktop}.{bind_host,base_port}` PMID 与 settings schema。model-pack 的 engine exhaustion 已消费本计划的 general envelope：`runtime_code = "runtime_engine_exhausted"`，`detail = { model_id, attempts[] }`，不再保留 `data.rollback` 特化生产信封。

---

## 1. 背景与目标

### 1.1 现状与痛点

`slab.rs` 的跨进程层是整个桌面 AI 运行时的"管道工"——它把 `slab-server` HTTP 网关、`slab-runtime` 推理 worker、`slab-js-runtime`/`slab-python-runtime` 插件宿主、`process_supervisor` 子进程监督器粘合在一起。审计 §5 + §2.3 暴露这一层有**两个 HIGH 级可靠性/安全风险**与若干 MED/LOW 缺口：

**🔴 风险一：路径包含校验三套实现语义不一致（R1，安全相关）**

跨"模型路径校验"与"插件资源路径校验"两条敏感链路存在**三套独立实现**，且语义不一致：

- [catalog.rs:572-588](../../../crates/slab-app-core/src/domain/services/model/catalog.rs#L572-L588) `validate_path`：纯词法判定，`Path::components().any(|c| c == ParentDir)`，**完全不 canonicalize**。symlink 逃逸、绝对分量嵌入（`/etc/passwd`）、`root/./evil` 均能绕过——是典型的越界（path-traversal）绕过形态。
- [package.rs:203-221](../../../crates/slab-app-core/src/domain/services/plugin/package.rs#L203-L221) `ensure_path_within`：第三套，`root.canonicalize()?` 后对 `path` 调 `canonicalize_existing_or_nearest(&path)` 再 `starts_with`——`canonicalize_existing_or_nearest` 对不存在路径会回退到父目录规范化，**存在 TOCTOU 与未完全规范化**的窗口。
- [registry.rs:148-156](../../../crates/slab-plugin/src/registry.rs#L148-L156) `is_path_within_root`（被 [plugin/assets.rs:40](../../../crates/slab-app-core/src/domain/services/plugin/assets.rs#L40) 等使用）：**两侧 canonicalize**，是三者中最严谨的实现。

> **审计纠错（2026-06-18 核实）**：审计原文（§5.1 R1）对第三套 `is_path_within_root` 仅含糊描述为"canonicalize 式"。经直接读源码，**该实现实际上对 root 与 path 都 canonicalize，是三者中最正确的**——故本规范选择它作为统一真源的语义基线，**保留其语义、迁移到 `slab_utils::path`、并替换另两套**。

**🔴 风险二：JSON-RPC pending-map 无界泄漏（G1，HIGH 可靠性）**

[bin/slab-js-runtime/src/api/jsonrpc/mod.rs:84-95](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs#L84-L95)（python 端逐字节同构）的 `request()` 实现：

```rust
async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
    let id = Value::String(format!("host-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
    let key = id_key(&id);
    let (tx, rx) = oneshot::channel();
    self.pending.lock().await.insert(key.clone(), tx);   // :88 插入
    self.send_serialized(&request(id, method, params));
    match rx.await {                                      // :91 无 timeout
        Ok(result) => result,
        Err(_) => Err(format!("host request `{method}` response channel closed")),
    }
}
```

只有当匹配响应到达时（`resolve_response` :43-56）才删 key。host 进程崩溃/挂起/协议 bug 不回 `host-N` 时，`oneshot::Sender` + key **永久残留**于 `Mutex<HashMap>`：长跑 runtime 缓慢内存泄漏 + 锁竞争加剧。`request()` 在 `RecvError` 返错但**不删 key**。**无超时、无上限、无清理任务**——四个性质同时缺失。

**🟠 缺口簇（MED，跨进程生命周期治理缺失）**

- **G2**：[mod.rs:170-175](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs#L170-L175)（py :184-196）每请求 `tokio::spawn`，JoinHandle 丢弃——无 `JoinSet`、无并发上限、无 panic 捕获。`server.handle_request` panic 时 `host.send_response` 不执行，对端永久等待。
- **G3**：[process_supervisor.rs](../../../crates/slab-app-core/src/infra/process_supervisor.rs) 的 `SupervisedStdioProcess`（struct :38-42）spawn **四**个 fire-and-forget task（writer :78、stderr :99、stdout :109、wait :119），靠 `AtomicBool alive` + `exit_handler` 维持，**无 `Drop` impl**——handle 被 drop 时四 task 不 abort，继续向已死子进程写 captured stdin/stdout。
- **G6**：[rpc/client.rs:214](../../../crates/slab-app-core/src/infra/rpc/client.rs#L214)、[:440](../../../crates/slab-app-core/src/infra/rpc/client.rs#L440)、[:542](../../../crates/slab-app-core/src/infra/rpc/client.rs#L542) 三处重试循环末尾 `unreachable!("... retry loop should always return")`——当前确不可达（`1..=N` 末轮命中 return），但**重构脆弱**（`continue`→fall-through 或排他界 `1..N` 即变可达 → 生产 panic）；[config_document.rs:524](../../../crates/slab-app-core/src/domain/services/model/config_document.rs#L524) `unreachable!("json payload should have been normalized to an object")` 承重于上游 normalizer。
- **G7**：[workspace/handler.rs:534](../../../bin/slab-server/src/api/v1/workspace/handler.rs#L534) `root.canonicalize().ok()` 失败 → 静默回退默认 settings 路径（:536-538），配置错的 root 与"用默认"不可区分、无日志。
- **G8**：[pmid_service.rs:29-31](../../../crates/slab-config/src/pmid_service.rs#L29-L31) 的 `DEFAULT_SERVER_RUNTIME_BASE_PORT`/`DEFAULT_DESKTOP_RUNTIME_BASE_PORT` 是 const 但非可配 PMID。

> **审计纠错（2026-06-18 核实）**：审计 §5.2 G8 原文称"硬编码 OpenAI base URL ×3，应为单一 const 或 PMID"，并引 `pmid_service.rs:1685,1723,1777`。经直接读源码，**这三处全部位于 `#[cfg(test)]` 测试代码**（`load_from_path_supports_current_settings_document` / `secret_setting_views_redact_literal_secret_values` / `update_setting_preserves_redacted_secret_placeholders` 三个测试函数的 `ProviderRegistryEntry` fixture），**非生产路径硬编码**。生产路径的 OpenAI base URL 由操作员 `chat.providers[]` 注册项（PMID 可编辑）注入。故本规范把 G8 的"OpenAI base URL ×3"部分**降级为测试去重**（抽 `test_fixtures::openai_provider()` 单一构造器），仅保留"运行期端口常量"的 PMID 化为生产改动。

- **G9**：[model_packs/mod.rs:307](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L307) `read_persisted_model_config_from_pack_bytes` 当 manifest SHA 与存储不符时返 `Ok(None)`；caller [:128](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L128) 继续构建无 `selected_download_source`/`local_path`/`status` 的命令——**静默把已下载模型重置为 `not_downloaded`**，无 warn。

**🟠 缺口簇（MED，错误结构与冗余）**

- **R5**：[bin/slab-js-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs)（357 行）与 [bin/slab-python-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-python-runtime/src/api/jsonrpc/mod.rs)（361 行）的 `JsonRpcRuntimeHost` 结构、`PendingMap`、`resolve_response`/`send_response`/`send_notification`/`send_serialized`、`impl RuntimeHost`、`drain_outbound` **逐字节同**（仅 parse 循环因宿主语言协议略异）；`drain_outbound` 在两文件 :181-196 / :209-224 是字符级相同函数。两者已 `use crate::slab_jsonrpc` 但**仅用于信封原语**，宿主态管道零共享，~150 行重复。
- **R6**：[bin/slab-runtime/src/api/handlers/](../../../bin/slab-runtime/src/api/handlers/) 六个 handler 文件（candle_diffusion / candle_transformers / ggml_diffusion / ggml_llama / ggml_whisper / onnx）共 **73** 处 `map_err(application_to_status|proto_to_status)?`（grep 核实；算上完整 handler 模式约 ×80）。映射函数本身已在 [handlers/mod.rs:36-82](../../../bin/slab-runtime/src/api/handlers/mod.rs#L36-L82) 正确集中（`runtime_to_status` 把 `QueueFull`→`resource_exhausted`、`BackendShutdown`→`unavailable`、`UnsupportedOperation`→`unimplemented`、`DriverNotRegistered`→`failed_precondition`），故**非逻辑重复而是样板**——无 `forward<Req,Resp,S>` 泛型。
- **F-Stack-3（general）/ P1-10**：`CoreError` 变体（`QueueFull{queue,capacity}` / `Busy{backend_id}` / `BackendShutdown` / `UnsupportedOperation{backend,op}` / `DriverNotRegistered{driver_id}`）在 [slab-runtime-core/src/base/error.rs:10-54](../../../crates/slab-runtime-core/src/base/error.rs#L10-L54) 结构良好；[bin/slab-server/src/error.rs:212-251](../../../bin/slab-server/src/error.rs#L212-L251) 的 `ServerError::Runtime(e)` 映射把上述变体**压平为固定人类可读字符串**（"inference backend is busy" 等），原始 `queue`/`capacity`/`backend_id`/`op`/`driver_id` detail **只进日志、不发往客户端**——HTTP 体只剩 `code: 5000 (RUNTIME_ERROR)` + 一句话 message，前端无字段可分支。`map_runtime_error` fallback（[runtime_gateway.rs:229](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L229)）对未识别 tonic status 一律 `Internal("grpc {action} failed: {error:#}")` → HTTP 500，仅 `action` 字符串进日志。

### 1.2 目标

| 目标 | 衡量标准 |
|------|----------|
| **G-SAFE 路径单一真源** | 插件包/资源的 root containment 只有一处实现 `slab_utils::path::ensure_within_root(root, path)`，两侧 canonicalize；`slab-plugin::is_path_within_root` 与 `plugin/package::ensure_path_within` 仅保留 thin wrapper/错误映射。模型 `model_path`/`model_cache_dir` 是任意绝对路径，`catalog.rs::validate_path` 只做绝对路径校验，不强行归入 root containment。对齐 R1 / P0-1。 |
| **G-BOUND pending-map 有界** | `request()` 的 `rx.await` 包 `tokio::time::timeout`，超时/err 删 key；`PendingMap` 加 `cap`，达上限拒绝新请求而非无限增长。对齐 G1 / P0-6。 |
| **G-SUPER task 可监督** | per-request 用 `JoinSet` + 并发上限 + panic 兜底响应；`SupervisedStdioProcess` 加 `Drop` impl abort 四 task。对齐 G2 / G3 / P2-2。 |
| **G-CODE 错误跨边界保结构** | `CoreError` 变体保 machine-readable `code`（snake_case 字符串）穿过 HTTP 边界进响应体 `data`；统一 agent 协议 `Error{code,message,i18n}` 入口。对齐 F-Stack-3 general / P1-10。 |
| **G-DEDUP 宿主骨架去重** | `JsonRpcRuntimeHost`/`drain_outbound`/`serve_reader` 抽入既有 `slab_jsonrpc::host`，参数化 `RequestHandler` trait，消除 ~150 行重复 + 补 dispatch 测试。调用方仍负责进程生命周期与业务派发。对齐 R5 / P1-7。 |
| **G-FORWARD gRPC 样板消除** | `forward<Req,Resp,S>(req, decode, svc, encode)` 泛型消除 ×80 `map_err` 样板。对齐 R6。 |
| **G-INVARIANT 承重不变式用类型表达** | 三处 `unreachable!()` 改 `return Err("exhausted retries")` 或注释证明不变式；`config_document` 那处注释 normalizer 不变式。对齐 G6 / P2-4。 |
| **G-OBS 静默行为可见** | G7 canonicalize 失败 `warn!` + 区分"用默认"；G9 manifest SHA 不匹配 `warn!`；G8 端口常量升 PMID、测试 OpenAI fixture 抽单一构造器。对齐 G7/G8/G9。 |

### 1.3 非目标（指向姊妹计划）

- **不**改 `model_pack` 配置体系（Schema/variant/engine chain）——见 [slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md)。该计划 Phase 2 的 engine exhaustion 已迁入本规范 §4 general envelope：生产路径使用 `RuntimeFailure` + `data.runtime_code = "runtime_engine_exhausted"` + `detail = { model_id, attempts[] }`，不再新增特化 `ServerError` 或 `data.rollback` 信封。
- **不**改 DB 契约、迁移、`tasks.result_data` 信封、CHECK 约束——属存储-契约计划（覆盖 T1/T2/T3/D1-D13/P0-2/P0-3/P0-5/P0-7/P1-5/P1-6/P1-8 部分/P2-5/P2-6/P2-7）。
- **不**改 PMID 双源桥、logging 级联、热重载拓宽、`parse_env` 诊断——属 PMID-设置计划（覆盖 PMID-F1/F2/F3/F4/F5/F6/F7/F8/F9/F10/F11/F-Stack-1/P1-1/P1-2/P1-3/P1-4/P2-1 配置部分/P2-3/P2-8/P2-9 `$config`→`$document` 部分）。
- **不**改 `slab-runtime-core` 内部 backend 加载实现、`slab-hub` 多源路由——分别属 model_pack Phase 2/3 与存储契约。
- **不**改数据路径 `.ok()`/`unwrap_or_default()` 的存储层 `warn!`（G4 data 部分 → 存储计划）；但 `app_config.rs::parse_env`（G4 config 部分 / PMID-F7）→ PMID 计划。本规范仅负责 G6 + 跨进程可靠性原则。
- **不**改认证交接、前端 token 注入（F-Stack-2 → PMID 计划）；不改选择→加载两步顺序（F-Stack-4 → model_pack）。

---

## 2. 架构设计原则

### 2.1 单一真源（Single Source of Truth）

> 审计 §5.1 R1 的"三套路径校验语义不一致"是本规范第一驱动力。同型决策只允许一处实现。

**P1. 路径包含校验只有一处。** `slab_utils::path::ensure_within_root(root, path)` 是仓库唯一的"判断 path 是否在 root 内"实现，适用于插件包/资源边界。`package.rs::ensure_path_within` 与 `slab-plugin::is_path_within_root` 仅作为兼容 wrapper/错误映射存在。`catalog.rs::validate_path` 不属于 root containment：模型 `model_path`/`model_cache_dir` 允许任意绝对路径，因此只调用绝对路径输入校验 helper。

**P2. JSON-RPC 宿主态只有一处。** `JsonRpcRuntimeHost`/`PendingMap`/`drain_outbound`/`serve_reader` 只存在于既有 `slab_jsonrpc::host`，不新增 workspace crate。js-runtime 与 python-runtime 各自的 `jsonrpc/mod.rs` 只保留**宿主语言协议差异**（python 的 `plugin.call` 派发分支、js 的 `handle_request` 调用形态），其余全部 import 共享宿主；进程生命周期、ready payload、业务派发仍归各调用方。

### 2.2 两侧 canonicalize（Defense-in-Depth Path Validation）

**P3. 校验前先解析。** 路径校验在 `starts_with` 比较前，root 与 path 都必须经 `std::fs::canonicalize`（或对不存在路径的 `canonicalize_existing_or_nearest` 等价物）解析为绝对、规范化、symlink 展开的形式。仅 canonicalize root 而 path 不 canonicalize（[package.rs:203-221](../../../crates/slab-app-core/src/domain/services/plugin/package.rs#L203-L221) 当前形态）或完全不 canonicalize（[catalog.rs:572-588](../../../crates/slab-app-core/src/domain/services/model/catalog.rs#L572-L588)）都属漏洞。

> **失败语义**：canonicalize 失败（路径不存在 / 权限拒绝）必须**传播为错误**，不得 `.ok()` 吞掉（对齐 G7：当前 `workspace_info` 把失败静默当"无 overlay"）。

### 2.3 有界即正确（Boundedness Is Correctness）

> 审计 §5.2 G1 的"pending-map 无界泄漏"是本规范第二驱动力。任何无界增长的数据结构在长跑 runtime 中都是缓慢故障。

**P4. 每个等待都有时限。** `oneshot::Receiver::await` 必须包 `tokio::time::timeout`；超时即清理（删 key）+ 返 typed error。永不"等永远"。

**P5. 每个集合都有上限。** `PendingMap` 加 `cap`（默认 256，可配），达上限拒绝新请求返 `busy` 而非无限增长。同型原则适用于 `JoinSet` 并发上限。

### 2.4 监督即回收（Supervision Means Reclamation）

> 审计 §5.2 G2/G3 的"fire-and-forget task 无 Drop abort"是本规范第三驱动力。spawn 一个 task 而不在所有者 drop 时 abort，等同于把生命周期管理外包给 GC——Rust 没有 GC。

**P6. 每个 spawn 都有所有者。** per-request spawn 进 `JoinSet`（所有者 = host 实例）；`SupervisedStdioProcess` 持有四 task 的 `JoinSet`/`AbortHandle`，`Drop` impl 全部 abort。无 JoinHandle 丢弃。

**P7. panic 不静默死。** per-request task 的闭包用 `AssertUnwindSafe` + `catch_unwind`（或等价的 `tokio::spawn` + 监控 `JoinError::is_panic`）兜底，panic 时仍发 `error` 响应给对端，不永久挂起。

### 2.5 错误跨边界保结构（Errors Preserve Structure Across Boundaries）

> 审计 §2.3 F-Stack-3 的"CoreError 变体被压平为不透明字符串"是本规范第四驱动力。机器可读的变体信息必须在 HTTP 体里存活，前端才能分支。

**P8. 每个变体有 machine-readable code。** `CoreError` 变体映成稳定 snake_case code（`runtime_queue_full` / `runtime_busy` / `runtime_backend_shutdown` / `runtime_unsupported_operation` / `runtime_driver_not_registered` / ...），跨 tonic → `AppCoreError` → `ServerError` → HTTP body 不丢失。code 是契约（写入 openapi），人类可读 message 是 i18n 资源。

**P9. 一套错误信封贯穿 HTTP + agent。** HTTP `ErrorResponse{code,data,message,i18n}` 与 agent 协议 `AgentResponsesServerMessage::Error{code,message,i18n}` 共享同一 `code` 词表（§4）。`ServerError::Runtime(_)` 不再折叠为单一 `"internal_error"`——携带具体 runtime code。

### 2.6 承重不变式用类型而非 unreachable! 表达（Type-Level Invariants）

> 审计 §5.2 G6 的"`unreachable!()` 承重且脆弱"是本规范第五驱动力。

**P10. 不变式要么类型证明，要么显式 return。** 重试循环末尾的"理应不可达"改 `return Err("exhausted retries")`（编译器强制所有路径返值）；若确需 `unreachable!()`，必须在该处注释证明不变式（哪个 loop bound、哪个 normalizer 保证不可达），并加 debug_assert 配对。`config_document.rs:524` 那处的"已 normalize 为 object"由 normalizer 函数签名/类型保证（§3.7）。

---

## 3. 核心设计（Per-Finding / Per-Cluster）

### 3.1 R1 / P0-1 — `slab_utils::path::ensure_within_root` 单一真源（安全）

**根因**：[catalog.rs:572-588](../../../crates/slab-app-core/src/domain/services/model/catalog.rs#L572-L588) 与 [package.rs:203-221](../../../crates/slab-app-core/src/domain/services/plugin/package.rs#L203-L221) 是两套语义不一致的"路径包含校验"，前者不 canonicalize、后者只 canonicalize root；第三套 [registry.rs:148-156](../../../crates/slab-plugin/src/registry.rs#L148-L156) 两侧 canonicalize 但活在 slab-plugin crate，业务核心无法依赖（会形成 slab-app-core → slab-plugin 反向依赖，违反 [AGENTS.md:28](../../../AGENTS.md#L28) 边界）。

**修复**：在 [crates/slab-utils/src/path/mod.rs](../../../crates/slab-utils/src/path/mod.rs)（已有 `normalize_relative_path`）新增 `ensure_within_root`，**两侧 canonicalize**，作为仓库唯一真源：

```rust
// crates/slab-utils/src/path/mod.rs
//
// 校验 `path` 解析后位于 `root` 之内。两侧均 canonicalize。
// 语义基线取自 slab-plugin::registry::is_path_within_root（三者中最严谨）。
//
// 返回：
//   Ok(canonicalized_path) — path 在 root 内，返回规范化绝对路径供调用方复用
//   Err(EnsureWithinRootError) — 越界 / canonicalize 失败 / path 不存在
//
// 错误传播而非 .ok() 吞掉（对齐 G7 原则）。

#[derive(Debug, thiserror::Error)]
pub enum EnsureWithinRootError {
    #[error("failed to canonicalize root {root}: {source}")]
    RootCanonicalize { root: PathBuf, source: std::io::Error },
    #[error("failed to canonicalize path {path}: {source}")]
    PathCanonicalize { path: PathBuf, source: std::io::Error },
    #[error("path {path} escapes root {root}")]
    EscapesRoot { path: PathBuf, root: PathBuf },
}

pub fn ensure_within_root(
    root: &Path,
    path: &Path,
) -> Result<PathBuf, EnsureWithinRootError> {
    let canonical_root = root.canonicalize().map_err(|e| {
        EnsureWithinRootError::RootCanonicalize { root: root.to_path_buf(), source: e }
    })?;
    // path 可能尚不存在（如待创建文件）；调用方据场景选 strict 或 best-effort 变体
    let canonical_path = path.canonicalize().map_err(|e| {
        EnsureWithinRootError::PathCanonicalize { path: path.to_path_buf(), source: e }
    })?;
    if canonical_path.starts_with(&canonical_root) {
        Ok(canonical_path)
    } else {
        Err(EnsureWithinRootError::EscapesRoot {
            path: canonical_path,
            root: canonical_root,
        })
    }
}

// 对"待创建文件"场景：尽力规范化到最近存在的祖先，再 starts_with。
// 用于 package.rs 的 plugin asset 解析（资产可能尚未落盘）。
pub fn ensure_within_root_or_nearest(
    root: &Path,
    path: &Path,
) -> Result<PathBuf, EnsureWithinRootError> { /* canonicalize_existing_or_nearest + starts_with */ }
```

**调用点替换**：

| 原实现 | 替换为 | 备注 |
|--------|--------|------|
| [catalog.rs:572-588](../../../crates/slab-app-core/src/domain/services/model/catalog.rs#L572-L588) `validate_path` | 改调 `slab_utils::path::validate_absolute_path(label, path)` 抛 `BadRequest` | 模型 `model_path`/`model_cache_dir` 是任意绝对路径，不使用 root containment。这样避免把模型缓存目录错误限制到插件/资源 root 下 |
| [package.rs:203-221](../../../crates/slab-app-core/src/domain/services/plugin/package.rs#L203-L221) `ensure_path_within` | 保留为 app-core 错误映射 wrapper；内部改调 `ensure_within_root_or_nearest(root, path)?` | plugin asset 可能未落盘，用 `_or_nearest` 变体 |
| [registry.rs:148-156](../../../crates/slab-plugin/src/registry.rs#L148-L156) `is_path_within_root` | **保留为 thin wrapper** `pub fn is_path_within_root(root, path) -> bool { slab_utils::path::ensure_within_root(root, path).is_ok() }`（保 bool API 兼容现有 [plugin/assets.rs:40](../../../crates/slab-app-core/src/domain/services/plugin/assets.rs#L40) 等调用点） | 避免一次性改全部调用点；wrapper 内委托单一真源 |

**调用点新增（G7 同步闭合）**：[workspace/handler.rs:534](../../../bin/slab-server/src/api/v1/workspace/handler.rs#L534) 的 `root.canonicalize().ok()` 改为：

```rust
let configured_root = match config.workspace_root.as_deref() {
    Some(root) => match slab_utils::path::ensure_within_root(root, root) {
        Ok(canonical) => Some(canonical),
        Err(e) => {
            warn!(error = %e, root = %root.display(), "workspace_root canonicalize failed; falling back to default settings path");
            None
        }
    },
    None => None,
};
```

> **G7 闭合**：canonicalize 失败从静默变 `warn!`，配置错误可见。语义不变（仍回退默认），但运维可观测。

**安全测试（前移）**：在 `slab-utils` 加 `path` 模块负向测试：① symlink 逃逸（`root/link → /etc`，校验 `root/link/passwd` 拒绝）；② 绝对分量嵌入（`/etc/passwd`）；③ `..` 遍历（`root/../../etc/passwd`）；④ 大小写/分隔符规范化（Windows `Root\\..\\evil`）；⑤ 不存在路径（strict 拒绝 / `_or_nearest` 容忍）。

---

### 3.2 R5 / P1-7 — 共享 JSON-RPC 宿主入 `slab_jsonrpc`

**根因**：[slab-jsonrpc/src/lib.rs](../../../crates/slab-jsonrpc/src/lib.rs)（257 行）当前**仅**含 JSON-RPC 2.0 信封原语（`request`/`response`/`notification` 构造与解析）。js-runtime（357 行）与 python-runtime（361 行）各自重复实现了完整的**宿主态管道**：`JsonRpcRuntimeHost` 结构 + `PendingMap` + `resolve_response` + `send_response`/`send_notification`/`send_serialized` + `impl RuntimeHost` + `drain_outbound`，这些在两文件里逐字节相同。

**修复**：把宿主态管道抽入既有 `slab_jsonrpc::host`。不新增 `slab-jsonrpc-host` workspace crate；该模块只提供通用管道、pending 管理、reader/writer loop 与并发保护，进程生命周期和业务派发仍由调用方实现。

**抽出的 API 形态**：

```rust
// crates/slab-jsonrpc/src/host.rs （新文件，pub mod host;）

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::io::AsyncWrite;
use tokio::time::timeout;

pub type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

/// 入站请求派发 trait。宿主语言（js/python）各自实现。
#[async_trait::async_trait]
pub trait RequestHandler: Send + Sync + Clone + 'static {
    /// 处理一个入站 JSON-RPC 请求，返回 Ok(value) 或 Err(error_string)。
    async fn handle_request(&self, method: &str, params: Value) -> Result<Value, String>;
}

#[derive(Clone)]
pub struct JsonRpcRuntimeHost<H: RequestHandler> {
    outbound: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
    /// §3.3 G1: pending-map cap，达上限拒绝新请求
    pending_cap: usize,
    /// §3.3 G1: 单请求超时（秒）
    request_timeout: std::time::Duration,
    _handler: std::marker::PhantomData<H>,
}

impl<H: RequestHandler> JsonRpcRuntimeHost<H> {
    pub fn new(
        outbound: mpsc::UnboundedSender<String>,
        pending_cap: usize,
        request_timeout: std::time::Duration,
    ) -> Self { /* ... */ }

    /// G1 闭合：rx.await 包 timeout，超时/err 删 key（见 §3.3）
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> { /* ... */ }

    pub fn resolve_response(&self, response: Value) { /* 同现有 */ }
    pub fn send_response(&self, id: Value, result: Result<Value, String>) { /* 同现有 */ }
    pub fn send_notification(&self, method: &str, params: Value) { /* 同现有 */ }
    pub fn send_serialized(&self, line: &str) { /* 同现有 */ }
}

/// G2 闭合：per-request JoinSet + 并发上限 + panic 兜底（见 §3.4）
pub async fn serve_reader<R, H>(
    mut reader: R,
    outbound: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    handler: H,
    concurrency_limit: usize,
) where
    R: tokio::io::AsyncBufRead + Unpin,
    H: RequestHandler,
{ /* 共享 parse 循环 + JoinSet 监督 */ }

pub async fn drain_outbound<W>(mut outbound: mpsc::UnboundedReceiver<String>, mut writer: W)
where
    W: AsyncWrite + Unpin,
{ /* 同现有两份逐字节同实现 */ }
```

**js-runtime / python-runtime 改造**：

- 删除各自 `JsonRpcRuntimeHost` / `PendingMap` / `resolve_response` / `send_*` / `drain_outbound` / `impl RuntimeHost` 的重复定义，改 `use slab_jsonrpc::host::{JsonRpcRuntimeHost, RequestHandler, serve_reader, drain_outbound};`。
- 各自仅保留**宿主语言协议差异**：
  - **js-runtime**：实现 `impl RequestHandler for JsHandler`（包 `server.handle_request`）。
  - **python-runtime**：实现 `impl RequestHandler for PyHandler`（包 `plugin.call` 派发分支 [:184-196](../../../bin/slab-python-runtime/src/api/jsonrpc/mod.rs#L184-L196)）。
- 预计 js-runtime mod.rs 从 357 行降到 ~120 行，python 从 361 行降到 ~150 行（差值即 plugin.call 分支）；**净消除 ~150 行重复**。

**dispatch 测试（顺带补）**：审计 R5 提"顺带补 dispatch 测试"——在 `slab_jsonrpc::host` 加：
- 正向：发送 `host-0` 请求 → mock handler 返 value → `resolve_response` 触发 → `request()` 收到 value。
- 超时：mock handler 永不响应 → `request()` 在 `request_timeout` 后返 `Err("host request timeout")` + key 从 pending-map 删除（§3.3 闭合）。
- panic：mock handler panic → `serve_reader` 的 JoinSet 捕获 → 对端收到 `error` 响应（§3.4 闭合）。
- 并发上限：mock handler 慢 → 第 N+1 个请求被 `concurrency_limit` 拒绝返 `busy`。

---

### 3.3 G1 / P0-6 — pending-map 超时清理 + cap

**根因**：[bin/slab-js-runtime/src/api/jsonrpc/mod.rs:84-95](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs#L84-L95) 的 `request()`，`rx.await` 无 timeout、`RecvError` 不删 key、HashMap 无上限。

**修复**：在 §3.2 抽出的 `JsonRpcRuntimeHost::request` 内：

```rust
pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
    let id = Value::String(format!("host-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
    let key = id_key(&id);
    let (tx, rx) = oneshot::channel();

    // G1 修复 (c)：达 cap 拒绝新请求
    {
        let mut pending = self.pending.lock().await;
        if pending.len() >= self.pending_cap {
            return Err(format!(
                "host request `{method}` rejected: pending-map at capacity ({})",
                self.pending_cap
            ));
        }
        pending.insert(key.clone(), tx);
    }
    self.send_serialized(&request(id, method, params));

    // G1 修复 (a)+(b)：rx.await 包 timeout，超时/err 删 key
    let outcome = timeout(self.request_timeout, rx).await;
    // 无论 Ok/Err/超时，key 都不再需要——发送端要么已消费要么已死
    self.pending.lock().await.remove(&key);
    match outcome {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(format!("host request `{method}` response channel closed")),
        Err(_) => Err(format!(
            "host request `{method}` timed out after {:?}",
            self.request_timeout
        )),
    }
}
```

**配置**（参数化，不硬编码）：
- `pending_cap`：默认 `256`（足够覆盖正常并发；异常积压时 fail-fast 而非 OOM）。
- `request_timeout`：默认 `30s`（覆盖 plugin 冷启动；过短会误杀慢插件）。
- 两值由 js-runtime / python-runtime 的 main.rs 构造 `JsonRpcRuntimeHost` 时传入，将来可由 PMID `runtime.jsonrpc.*` 暴露（PMID 计划负责）。

> **为什么 `remove(&key)` 放在所有路径**：超时与正常响应竞态时，`resolve_response` 可能已消费 `tx` 并删过 key——再 `remove` 是 no-op，安全。关键是**永不漏删**。

**回归测试**：见 §3.2 dispatch 测试"超时"分支。

---

### 3.4 G2 — per-request JoinSet + 并发上限 + panic 兜底

**根因**：[bin/slab-js-runtime/src/api/jsonrpc/mod.rs:170-175](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs#L170-L175)（py :184-196）`tokio::spawn(async move { ...; host.send_response(id, result); })`，JoinHandle 丢弃，无监督、无 panic 兜底。

**修复**：在 §3.2 抽出的 `serve_reader` 内用 `JoinSet` + `Semaphore`（并发上限）+ panic 监控：

```rust
use tokio::task::JoinSet;
use tokio::sync::Semaphore;
use std::sync::Arc;

pub async fn serve_reader<R, H>(/* ... */, concurrency_limit: usize)
where R: AsyncBufRead + Unpin, H: RequestHandler,
{
    let mut lines = tokio_buf_reader_lines(reader);
    let mut join_set: JoinSet<()> = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let handler = handler.clone();
    let host = JsonRpcRuntimeHost::clone(/* outbound, pending, ... */);

    while let Some(line) = lines.next_line().await.transpose() {
        let Ok(line) = line else { continue; };
        let incoming = match parse_incoming(&line) { Ok(v) => v, Err(_) => continue };
        let Some(id) = incoming.id.clone() else { continue; };

        // G2 修复：并发上限
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let host = host.clone();
        let handler = handler.clone();

        // G2 修复：JoinSet 监督
        join_set.spawn(async move {
            let _permit = permit; // 持有至 task 结束
            // G2 修复：panic 兜底
            let result = std::panic::AssertUnwindSafe(handler.handle_request(&incoming.method, incoming.params))
                .catch_unwind()
                .await
                .map_err(|panic_payload| {
                    let msg = panic_payload
                        .downcast_ref::<String>().map(|s| s.as_str())
                        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                        .unwrap_or("plugin handler panic");
                    format!("plugin handler panicked: {msg}")
                })
                // catch_unwind 返 Result<Result<Value,String>, Box<dyn Any>>，展平
                .and_then(|inner| inner);
            host.send_response(id, result);
        });

        // 回收已完成 task（避免 JoinSet 无限增长）
        while join_set.len() > concurrency_limit * 2 {
            let _ = join_set.join_next().await;
        }
    }
}
```

> **panic 兜底细节**：`catch_unwind` 在 async 上下文需 `futures::FutureExt::catch_unwind` + `AssertUnwindSafe`。`send_response` 即便在 panic 后也必须执行——这是修复的核心：对端不再永久等待。注意 `send_response(id, result)` 是 clone-able 的 host 方法，闭包持有 clone，不受原 host drop 影响。

> **`AssertUnwindSafe` 安全性论证**：`RequestHandler` 要求 `Send + Sync + 'static`，`handle_request` 内部状态由实现保证 unwind 安全（plugin runtime 应自身处理 panic；我们兜底的是"未捕获的 panic"，把 message 序列化进 error 响应）。若 plugin runtime 自身需要 unwind 安全保证，应在 handler 实现内部用 `spawn` + 监控 `JoinError::is_panic`（等价语义）。

**配置**：`concurrency_limit` 默认 `16`（覆盖典型插件并发；过大会退化回 G2 原状，过小会串行化热路径）。js-runtime / python-runtime 各自 main.rs 传入。

---

### 3.5 G3 / P2-2 — `SupervisedStdioProcess` Drop abort 四 task

**根因**：[process_supervisor.rs](../../../crates/slab-app-core/src/infra/process_supervisor.rs) 的 `SupervisedStdioProcess`（struct :38-42）spawn 四个 task（writer :78、stderr :99、stdout :109、wait :119），靠 `AtomicBool alive` + `exit_handler` 维持，**无 `Drop` impl**——handle drop 时四 task 继续 capture stdin/stdout 写向已死子进程。

**修复**：给 `SupervisedStdioProcess` 加四 task 的 `AbortHandle`（或把它们收进一个 `JoinSet`）+ `Drop` impl：

```rust
pub(crate) struct SupervisedStdioProcess {
    label: Arc<str>,
    stdin: mpsc::UnboundedSender<String>,
    alive: Arc<AtomicBool>,
    // G3 修复：持有四 task 的 abort handle
    writer_abort: tokio::task::AbortHandle,
    stderr_abort: tokio::task::AbortHandle,
    stdout_abort: tokio::task::AbortHandle,
    wait_abort: tokio::task::AbortHandle,
}

impl Drop for SupervisedStdioProcess {
    fn drop(&mut self) {
        // G3 修复：handle drop 即 abort 四 task，停止向子进程写
        self.writer_abort.abort();
        self.stderr_abort.abort();
        self.stdout_abort.abort();
        self.wait_abort.abort();
        // alive 标志置 false，让 wait task 的 exit_handler 知道这是主动回收
        self.alive.store(false, Ordering::SeqCst);
    }
}
```

**spawn 改造**：把现有 `tokio::spawn(async move { ... })` 改：

```rust
let writer_handle = tokio::spawn(async move { /* writer 逻辑 */ });
let writer_abort = writer_handle.abort_handle();
// 持有 writer_handle 进 JoinSet 或显式管理（避免 JoinHandle drop 即不 abort）
// 由于 tokio 的 spawn 返 JoinHandle，drop JoinHandle 不 abort task——故必须显式 abort
```

> **审计边界（已核实，不重 flag）**：审计 §5 G3 括号明确核实 [workspace/mod.rs](../../../bin/slab-server/src/api/v1/workspace/mod.rs) 的 spawn 在 await 路径（**非**无监督）、[slab-agent/src/control.rs:488](../../../crates/slab-agent/src/control.rs#L488) 存 `abort_handle` 且 TOCTOU 安全自移除（**正确**）。本规范仅修 `process_supervisor` 的 Drop gap。

**测试**：`process_supervisor` 测试加：spawn 子进程 → drop `SupervisedStdioProcess` → 断言四 task 不再向 closed stdin pipe 写（无 panic、无 stderr noise）；`alive` 标志为 false；后续子进程退出被正常 reap（不僵尸）。

---

### 3.6 R6 — gRPC handler `forward<Req,Resp,S>` 泛型消除 ×80 样板

**根因**：[bin/slab-runtime/src/api/handlers/](../../../bin/slab-runtime/src/api/handlers/) 六个 handler 文件（candle_diffusion / candle_transformers / ggml_diffusion / ggml_llama / ggml_whisper / onnx）共 **73** 处 `map_err(application_to_status|proto_to_status)?`，模式高度一致：

```rust
// 现有 ggml_llama.rs:18-35 的 chat handler 样板
let request_id = extract_request_id(request.metadata());
tracing::Span::current().record("request_id", &request_id);
let dto = dto::decode_ggml_llama_chat_request(&request.into_inner()).map_err(proto_to_status)?;
let response = self.application
    .ggml_llama().map_err(application_to_status)?
    .chat(dto).await.map_err(application_to_status)?;
Ok(Response::new(dto::encode_ggml_llama_chat_response(&response)))
```

映射函数本身已在 [handlers/mod.rs:36-82](../../../bin/slab-runtime/src/api/handlers/mod.rs#L36-L82) 正确集中（`runtime_to_status` 把 `CoreError::QueueFull`→`Status::resource_exhausted` 等，**保结构良好**——注意：structure loss 发生在 HTTP 边界 [bin/slab-server/src/error.rs:212-251](../../../bin/slab-server/src/error.rs#L212-L251)，不在 slab-runtime；§3.8 单独处理）。

**修复**：在 `handlers/mod.rs` 新增泛型 `forward`：

```rust
// bin/slab-runtime/src/api/handlers/mod.rs

use tonic::{Request, Response, Status};

/// G-FORWARD: 消除 handler 样板。统一 request_id 提取 + tracing span record +
/// decode/proto_to_status + service/application_to_status + encode。
///
/// 泛型：
///   P  - protobuf request 类型
///   Q  - protobuf response 类型
///   D  - decode 出的 DTO 类型
///   R  - service 返回的 domain 类型
///   S  - service future
///   F  - decode 函数 P -> Result<D, ProtoConversionError>
///   G  - encode 函数 R -> Q
pub(super) async fn forward<P, Q, D, R, F, G, Svc, Fut>(
    request: Request<P>,
    decode: F,
    encode: G,
    service: Svc,
) -> Result<Response<Q>, Status>
where
    P: Send + 'static,
    F: FnOnce(P) -> Result<D, dto::ProtoConversionError>,
    G: FnOnce(&R) -> Q,
    Svc: FnOnce(D) -> Fut,
    Fut: std::future::Future<Output = Result<R, RuntimeApplicationError>>,
{
    let request_id = extract_request_id(request.metadata());
    tracing::Span::current().record("request_id", &request_id);

    let dto = decode(request.into_inner()).map_err(proto_to_status)?;
    let response = service(dto).await.map_err(application_to_status)?;
    Ok(Response::new(encode(&response)))
}
```

**调用点改造**（以 ggml_llama.rs:18-35 为例）：

```rust
#[instrument(skip_all, fields(request_id, backend = "ggml.llama"))]
async fn chat(
    &self,
    request: Request<pb::GgmlLlamaChatRequest>,
) -> Result<Response<pb::GgmlLlamaChatResponse>, Status> {
    let app = self.application.clone();
    forward(
        request,
        dto::decode_ggml_llama_chat_request,
        dto::encode_ggml_llama_chat_response,
        |dto| async move {
            app.ggml_llama()?.chat(dto).await
        },
    )
    .await
}
```

每个 handler 从 ~17 行降到 ~10 行，**消除全部 73 处 `map_err`**（集中在 `forward` 内两处）。预计 6 个 handler 文件总行数从 ~650 降到 ~400。

> **是否引入 macro**：本规范倾向**泛型函数而非 macro**——泛型有类型检查、IDE 友好；macro 在 Rust 生态对 IDE/编译错误更不友好。仅当 `service` 闭包捕获所有权复杂时（如 `app.ggml_llama()?` 的两阶段错误）才考虑 macro；目前泛型 + async closure 足够。

> **回归保证**：`forward` 内的 `map_err(application_to_status|proto_to_status)` 与现有逐字节同语义；新增测试断言每个改造后的 handler 对同输入返同 `Status::code()` + 同 message。

---

### 3.7 G6 / P2-4 — `unreachable!()` 改 `return Err` 或注释证明

**根因**：四处 `unreachable!()` 承重且重构脆弱：

| 位置 | 当前代码 | 风险 |
|------|---------|------|
| [rpc/client.rs:214](../../../crates/slab-app-core/src/infra/rpc/client.rs#L214) | `call_initial_response_with_retry` 末尾 `unreachable!("unary gRPC retry loop should always return")` | `for attempt in 1..=N` 末轮命中 return，**当前不可达**；但若有人改 `continue`→fall-through 或 `1..N` 排他界即 panic 生产 |
| [rpc/client.rs:440](../../../crates/slab-app-core/src/infra/rpc/client.rs#L440) | `load_model` 同型 | 同型脆弱 |
| [rpc/client.rs:542](../../../crates/slab-app-core/src/infra/rpc/client.rs#L542) | `unload_model` 同型 | 同型脆弱 |
| [config_document.rs:524](../../../crates/slab-app-core/src/domain/services/model/config_document.rs#L524) | `ensure_json_object` 末尾 `unreachable!("json payload should have been normalized to an object")` | 承重于函数顶部 `if !value.is_object() { *value = Value::Object(Map::new()); }` normalizer |

**修复**：

**(a) 三处 retry loop 末尾** 改 `return Err`（编译器强制所有路径返值，重构不破坏）：

```rust
// rpc/client.rs:214 原 unreachable! 改为：
return Err(Status::unknown("unary gRPC retry loop exhausted without returning"));
// 注：实际不可达（for 1..=N 末轮必命中 return），但用 return Err 兜底，
// 使未来若有人改 loop bound 为排他界，得到 typed gRPC error 而非生产 panic。
// 等价于 §2.6 P10：不变式用类型（返值）而非 panic 表达。
```

> 注：`call_initial_response_with_retry` 当前返 `Result<Status, Status>` 或类似；具体返值类型据实现填——核心是 `return Err(typed_status)` 而非 `unreachable!()`。`load_model`/`unload_model` 返 `anyhow::Error` 路径同理改 `return Err(anyhow::anyhow!("... retry loop exhausted"))`。

**(b) `ensure_json_object`** 改注释 + 类型证明 + `debug_assert`（保留 unreachable 语义但显式证明不变式）：

```rust
// config_document.rs:524 区域
pub(super) fn ensure_json_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    // 不变式证明：上方 if 已保证 value 是 Object（要么原本是、要么刚被替换为空 Object）。
    // Value 是 #[non_exhaustive] 闭集（Null/Bool/Num/Str/Arr/Obj），is_object() 仅对 Object 真；
    // 故此处 match 的 _ 分支理论上不可达。
    // 不用 unreachable!() 而是 debug_assert + 显式 panic message——生产中若 serde_json
    // 引入新变体（极不可能）得到清晰诊断而非模糊 panic。
    match value {
        Value::Object(map) => map,
        other => {
            debug_assert!(
                false,
                "ensure_json_object invariant violated: value is {other:?} after normalization"
            );
            // 退化路径：再次替换为空 Object（永不执行，但保函数返值类型）
            *other = Value::Object(Map::new());
            match other {
                Value::Object(map) => map,
                _ => unreachable!("replaced with Object above"),
            }
        }
    }
}
```

> **审计纠错（2026-06-18 核实）**：审计 §5.2 G6 引 `crates/slab-config/src/config_document.rs:543`，但**实际文件路径是 `crates/slab-app-core/src/domain/services/model/config_document.rs:524`**（不是 slab-config crate），行号 524 而非 543。本规范已用正确路径。

---

### 3.8 F-Stack-3 general / P1-10 — gRPC 错误跨 HTTP 边界保结构

> 这是本规范最复杂的条款，单列 §4 详述错误信封统一模型。本节给出修复落地点。

**根因**：[bin/slab-server/src/error.rs:212-251](../../../bin/slab-server/src/error.rs#L212-L251) 的 `ServerError::Runtime(e)` 映射把 `CoreError` 变体压平为固定字符串，detail（`queue`/`capacity`/`backend_id`/`op`/`driver_id`）丢失；`map_runtime_error` fallback（[runtime_gateway.rs:229](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L229)）对未识别 tonic status 一律 `Internal("grpc {action} failed")` → HTTP 500。

**修复**：

**(a) `CoreError` 加 machine-readable code**：在 [slab-runtime-core/src/base/error.rs:10-54](../../../crates/slab-runtime-core/src/base/error.rs#L10-L54) 给每个变体加方法 `code()`：

```rust
impl CoreError {
    /// 跨 tonic → AppCoreError → ServerError → HTTP body 保结构的稳定 code。
    /// 写入 openapi，是机器可读契约。
    pub fn code(&self) -> &'static str {
        match self {
            Self::QueueFull { .. } => "runtime_queue_full",
            Self::Busy { .. } => "runtime_busy",
            Self::BackendShutdown => "runtime_backend_shutdown",
            Self::Timeout => "runtime_timeout",
            Self::UnsupportedOperation { .. } => "runtime_unsupported_operation",
            Self::DriverNotRegistered { .. } => "runtime_driver_not_registered",
            Self::InternalPoisoned { .. } => "runtime_internal_poisoned",
            Self::EngineIo(_) => "runtime_engine_io",
            Self::GGMLEngine { .. } => "runtime_ggml_engine",
            Self::OnnxEngine(_) => "runtime_onnx_engine",
            Self::CandleEngine { .. } => "runtime_candle_engine",
            // model_pack Phase 2 引入的变体挂同一个 code 命名空间
            // Self::ModelNotLoaded => "runtime_model_not_loaded", 等
        }
    }

    /// 把变体的 detail 字段序列化为 JSON 对象（machine-readable，进 HTTP body `data`）
    pub fn detail(&self) -> Option<serde_json::Value> {
        match self {
            Self::QueueFull { queue, capacity } => Some(serde_json::json!({
                "queue": queue, "capacity": capacity,
            })),
            Self::Busy { backend_id } => Some(serde_json::json!({ "backend_id": backend_id })),
            Self::UnsupportedOperation { backend, op } => Some(serde_json::json!({
                "backend": backend, "op": op,
            })),
            Self::DriverNotRegistered { driver_id } => Some(serde_json::json!({
                "driver_id": driver_id,
            })),
            // 无字段的变体返 None
            _ => None,
        }
    }
}
```

**(b) `ServerError::Runtime(e)` 不再折叠**：改 [bin/slab-server/src/error.rs:212-251](../../../bin/slab-server/src/error.rs#L212-L251)：

```rust
ServerError::Runtime(e) => {
    error!(error = %e, "AI runtime error");
    let code = e.code();                                       // machine-readable
    let detail = e.detail();                                   // structured
    let (status_code, i18n_key) = runtime_status_and_i18n(&e);// 见下表
    let message = e.to_string();                               // 人类可读，不再压平
    (
        status_code,
        error_codes::RUNTIME_ERROR,                            // 外层 u16 不变（兼容）
        detail,                                                // 进 body.data
        format!("{code}: {message}"),                          // body.message 带 code 前缀
        message_i18n(i18n_key),
    )
}
```

`runtime_status_and_i18n` 映射（HTTP status + i18n key）：

| `CoreError` 变体 | HTTP status | i18n key | 备注 |
|-----------------|-------------|----------|------|
| `QueueFull` / `Busy` | 503 Service Unavailable | `server.errors.runtime.busy` | 与 `BackendNotReady` 同档（资源耗尽），但 code 区分 |
| `BackendShutdown` | 503 | `server.errors.runtime.unavailable` | |
| `Timeout` | 504 Gateway Timeout | `server.errors.runtime.timeout` | |
| `UnsupportedOperation` | 400 Bad Request | `server.errors.runtime.unsupportedOperation` | 配置 bug，重试无益 |
| `DriverNotRegistered` | 503 | `server.errors.runtime.driverNotRegistered` | 缺编译 → 下一引擎（model_pack Phase 2 回退消费此 code） |
| `EngineIo` / `GGMLEngine` / `OnnxEngine` / `CandleEngine` | 500 | `server.errors.runtime.engine` | 引擎内部错（OOM 类经 `RuntimeMemoryPressure` 走另一路） |
| `InternalPoisoned` | 500 | `server.errors.runtime.internal` | |

**(c) `ErrorResponse.data` 携带 `{code, detail}`**：当前 [bin/slab-server/src/error.rs:26-32](../../../bin/slab-server/src/error.rs#L26-L32) 的 `ErrorResponse{code,data,message,i18n}` 已有 `data: Option<AppCoreErrorData>`——扩 `AppCoreErrorData` 增 `runtime_code: Option<&'static str>` 字段（或把整个 `{code, detail}` 塞进现有 data JSON）。HTTP body 形如：

```jsonc
{
  "code": 5000,                                  // 外层稳定 u16（兼容现有前端）
  "message": "runtime_queue_full: queue full: ggml.llama (capacity 64)",
  "data": {
    "runtime_code": "runtime_queue_full",        // 新：machine-readable
    "detail": { "queue": "ggml.llama", "capacity": 64 }
  },
  "i18n": { "message": { "key": "server.errors.runtime.busy", "params": {} } }
}
```

**(d) `RuntimeMemoryPressure` 不再映 `BackendNotReady`**：[error.rs:294-296](../../../bin/slab-server/src/error.rs#L294-L296) 当前把 OOM 类故障与"还在加载"合并。改为独立 `ServerError::RuntimeMemoryPressure` 变体（或 `ServerError::Runtime(_)` 路径下 `runtime_memory_pressure` code）——OOM 与"还在加载"前端处理不同（OOM 应提示"显存不足/换引擎"，加载中应轮询）。**注意**：model_pack Phase 2 的 engine exhaustion 已接入同一 code 命名空间，使用 `runtime_engine_exhausted` + `detail = { model_id, attempts[] }`，见 §4。

**(e) `map_runtime_error` fallback 保 gRPC status code**：[runtime_gateway.rs:229](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L229) 当前一律 `Internal("grpc {action} failed: {error:#}")`。改为保留 tonic `Code`（`Status::code()`）映 `runtime_grpc_<snake_case_code>`（如 `runtime_grpc_unavailable` / `runtime_grpc_deadline_exceeded`），仍进 `AppCoreError::Internal` 路径但 message 带 code 前缀，且 `data.runtime_code = "runtime_grpc_<code>"`。**未识别的 tonic status 不再被压平为不透明 500**。

**(f) agent 协议统一入口**：[agent/handler.rs:468](../../../bin/slab-server/src/api/v1/agent/handler.rs#L468) 的 `server_error_message` 调 `ServerError::agent_code_message()`（[error.rs:96-144](../../../bin/slab-server/src/error.rs#L96-L144)）；当前 `ServerError::Runtime(_)` arm 折叠为 `"internal_error"`。改为对 `ServerError::Runtime(e)` arm 提取 `e.code()` 作为 agent `code`（如 `"runtime_queue_full"`），与 HTTP body 共享 code 词表。`AgentResponsesServerMessage::Error{code,message,i18n,..}` 的 `code` 字段由此成为机器可读契约。

> **向后兼容**：外层 `code: u16`（4000/4004/4009/5000/...）保持不变，前端现有按 u16 分支的逻辑不受影响。新增的 `data.runtime_code`（snake_case 字符串）是**加性**字段，前端可渐进迁移到按 `runtime_code` 分支（更精确）。

---

### 3.9 G8 / P2-9 — OpenAI base URL 测试去重 + 端口常量 PMID 化

> **审计纠错（2026-06-18 核实）**：审计 §5.2 G8 原文称三处 `https://api.openai.com/v1` 硬编码（引 `pmid_service.rs:1685,1723,1777`）。**经直接读源码，这三处全部位于 `#[cfg(test)]` 测试代码**（`load_from_path_supports_current_settings_document` / `secret_setting_views_redact_literal_secret_values` / `update_setting_preserves_redacted_secret_placeholders` 三个测试函数的 `ProviderRegistryEntry` fixture），**非生产路径硬编码**。生产路径的 OpenAI base URL 由操作员 `chat.providers[]` 注册项（PMID 可编辑）经 [chat/cloud.rs](../../../crates/slab-app-core/src/domain/services/chat/cloud.rs) 注入。故本规范的"OpenAI base URL"部分降级为**测试 fixture 去重**，生产路径无改动。

**修复（测试部分）**：在 `crates/slab-config/src/pmid_service.rs` 的 `#[cfg(test)] mod tests` 内加 `test_fixtures::openai_provider(api_key: &str) -> ProviderRegistryEntry` 单一构造器，三个测试调用它：

```rust
#[cfg(test)]
mod test_fixtures {
    use super::*;
    pub(super) fn openai_provider(api_key: &str) -> ProviderRegistryEntry {
        ProviderRegistryEntry {
            id: "openai-main".to_owned(),
            family: ProviderFamily::OpenaiCompatible,
            display_name: "OpenAI".to_owned(),
            api_base: "https://api.openai.com/v1".to_owned(),  // 单一硬编码点
            auth: ProviderAuthConfig {
                api_key: Some(api_key.to_owned()),
                api_key_env: None,
            },
            defaults: ProviderDefaultsConfig::default(),
        }
    }
}

// 三个测试改：
// document.providers.registry.push(ProviderRegistryEntry { ... "https://api.openai.com/v1" ... });
// → document.providers.registry.push(test_fixtures::openai_provider("sk-test"));
```

**修复（端口常量 PMID 化，生产）**：[pmid_service.rs](../../../crates/slab-config/src/pmid_service.rs) 原 `DEFAULT_SERVER_RUNTIME_BASE_PORT = 3001` / `DEFAULT_DESKTOP_RUNTIME_BASE_PORT = 50051` 是 const 但**不可配**。本次按最小设置闭环新增 `runtime.launch.server.bind_host` / `runtime.launch.server.base_port` / `runtime.launch.desktop.bind_host` / `runtime.launch.desktop.base_port`，默认分别为 `127.0.0.1:3001` 与 `127.0.0.1:50051`，并把 `base_port` 约束为 `1..=65535`。`load_config` 用这些设置填充 `LaunchConfig.profiles`；settings document schema 已通过 `bun run gen:schemas` 更新。

> **`$config`→`$document` 部分**：审计 P2-9 还含 model_pack 的 `$config`→`$document` 命名去歧义——该条**归 model_pack 计划**（其 Phase 1 已含 `$config`→`$ref` alias 化，语义等价）。本规范不重复。

---

### 3.10 G9 — `manifest_sha256` 不匹配 warn

**根因**：[model_packs/mod.rs:307](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L307) `read_persisted_model_config_from_pack_bytes` 当 manifest SHA 与存储不符时返 `Ok(None)`；caller [:128](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L128) 静默构建无投影态命令。

**修复**：在 SHA 不匹配分支加 `warn!`：

```rust
// model_packs/mod.rs:307 区域
if verify_sha256_hex_expected(&actual_manifest_sha256, manifest_sha256).is_err() {
    warn!(
        expected = manifest_sha256,
        actual = %actual_manifest_sha256,
        "manifest SHA mismatch; discarding persisted model projection state \
         (selected_download_source/local_path/status will be reset to defaults)"
    );
    return Ok(None);
}
```

**caller 不改**：[:128](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L128) 的 `if let Some(config) = ... { apply_persisted_projection_state(...) }` 语义正确（无投影态时用 manifest 默认）。仅观测性补强——运维知道投影态被丢弃。

---

## 4. 错误信封统一模型（Cross-Boundary Error Envelope）

> 本节是 §3.8 的展开，回答："如何让 `CoreError` 变体、HTTP `ErrorResponse`、agent 协议 `Error` 三者在 code 上收敛？model_pack Phase 2 的 engine exhaustion 如何消费同一信封？"

### 4.1 三层错误的当前形态

| 层 | 类型 | 当前 code 形态 | 当前结构损失 |
|----|------|--------------|-------------|
| runtime（最里） | `CoreError`（[slab-runtime-core/src/base/error.rs:10-54](../../../crates/slab-runtime-core/src/base/error.rs#L10-L54)） | 变体名（Rust enum），结构化字段（`QueueFull{queue,capacity}`） | **无**——结构良好 |
| runtime→HTTP 网关 | tonic `Status`（[handlers/mod.rs:36-82](../../../bin/slab-runtime/src/api/handlers/mod.rs#L36-L82) `runtime_to_status`） | tonic `Code`（`ResourceExhausted`/`Unavailable`/`Unimplemented`/...）+ 字符串 message | message 是 `format_error_chain` 人类可读串，**变体 detail 进 message 字符串**（可解析但不结构化） |
| 网关→HTTP（slab-server） | `AppCoreError`（[rpc/runtime_gateway.rs:216-230](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L216-L230) `map_runtime_error`）+ `ServerError`（[error.rs:212-251](../../../bin/slab-server/src/error.rs#L212-L251)） | u16 `error_codes`（5000/5003/...）+ 字符串 message + i18n key | **变体 detail 丢失**（压平为 "inference backend is busy"）；未识别 tonic status 全部 → 500 + "grpc {action} failed" |
| HTTP→agent（WS/SSE） | `AgentResponsesServerMessage::Error{code,message,i18n}`（[agent/handler.rs:468](../../../bin/slab-server/src/api/v1/agent/handler.rs#L468)） | 字符串 code（`"internal_error"` / `"backend_not_ready"` / ...） | `ServerError::Runtime(_)` 折叠为 `"internal_error"` |

### 4.2 统一信封：`runtime_code` snake_case 字符串

本规范引入 **`runtime_code: &'static str`**（snake_case）作为贯穿三层的**机器可读契约**：

```
CoreError::QueueFull{queue,capacity}
   │
   │  CoreError::code() → "runtime_queue_full"
   │  CoreError::detail() → {"queue": ..., "capacity": ...}
   ▼
tonic Status (slab-runtime handler)
   │  Status::code() = ResourceExhausted, message = "queue full: ggml.llama (capacity 64)"
   │  ★ runtime_code 不在 tonic Status 内传播（tonic metadata 可加但非必须）——
   │    由 AppCoreError 侧从 Status::code() + message 反推，或更优：CoreError 在
   │    slab-runtime-core 序列化进 Status metadata（见 §4.4 决策）
   ▼
AppCoreError (slab-app-core rpc_gateway)
   │  新增 AppCoreError::RuntimeStructured { code: &'static str, detail: Value, message: String }
   │  取代 RuntimeMemoryPressure/BackendNotReady 等多变体为一信封 + code 判别
   │  （或并存：保现有变体 + 新增 code 携带）
   ▼
ServerError (slab-server error.rs)
   │  ErrorResponse { code: 5000 (u16 不变), data: { runtime_code: "runtime_queue_full", detail: {...} }, message: "runtime_queue_full: queue full: ...", i18n }
   ▼
HTTP body  +  agent Error{code: "runtime_queue_full", message, i18n}
```

### 4.3 变体 → code → HTTP → i18n 对照表

| `CoreError` 变体 | `runtime_code` | HTTP status | i18n key | model_pack Phase 2 消费？ |
|-----------------|----------------|-------------|----------|---------------------------|
| `QueueFull{queue,capacity}` | `runtime_queue_full` | 503 | `server.errors.runtime.busy` | 否（瞬时） |
| `Busy{backend_id}` | `runtime_busy` | 503 | `server.errors.runtime.busy` | 否 |
| `BackendShutdown` | `runtime_backend_shutdown` | 503 | `server.errors.runtime.unavailable` | 否 |
| `Timeout` | `runtime_timeout` | 504 | `server.errors.runtime.timeout` | 否 |
| `UnsupportedOperation{backend,op}` | `runtime_unsupported_operation` | 400 | `server.errors.runtime.unsupportedOperation` | 否（配置 bug，**不重试**——§4.5 model_pack §4.4 表） |
| `DriverNotRegistered{driver_id}` | `runtime_driver_not_registered` | 503 | `server.errors.runtime.driverNotRegistered` | **是**（缺编译 → 触发跨引擎回退） |
| `EngineIo` / `GGMLEngine` / `OnnxEngine` / `CandleEngine` | `runtime_engine_io` / `runtime_ggml_engine` / `runtime_onnx_engine` / `runtime_candle_engine` | 500 | `server.errors.runtime.engine` | 部分（OOM 经 `RuntimeMemoryPressure` 走下条） |
| （新）`RuntimeMemoryPressure` | `runtime_memory_pressure` | 503 | `server.errors.runtime.memoryPressure` | **是**（OOM → 触发跨引擎回退） |
| （model_pack 已消费）engine fallback exhausted | `runtime_engine_exhausted` | 500（外层 code 5000） | `server.errors.runtime.engineExhausted` | **是**（本信封的 model-pack 消费者） |
| （gRPC fallback）未识别 tonic status | `runtime_grpc_<snake_code>` | 500/503 据 tonic code | `server.errors.runtime.grpc` | 否 |

### 4.4 决策：`runtime_code` 跨 tonic 边界如何传播

三个选项：

| 方案 | 形态 | 优点 | 缺点 |
|------|------|------|------|
| **A. tonic metadata** | slab-runtime 在 `runtime_to_status` 内把 `runtime_code` 塞进 `Status::metadata()`（custom header `x-runtime-code`） | 保结构最完整；网关直接读 metadata | tonic metadata 跨网络序列化，需 ASCII；对已部署 runtime 有线协议变更 |
| **B. message 前缀** | `format_error_chain` 改为 `format!("{code}: {msg}")`，网关侧解析前缀 | 零 wire 协议变更 | 字符串解析脆弱（message 含 `:` 时歧义） |
| **C. CoreError 序列化进 detail**（**采纳**） | slab-runtime 在 `runtime_to_status` 把 `CoreError::code()` + `detail()` 序列化为 JSON 塞进 `Status::source()` 链或 tonic `Status::metadata()` 的 `x-runtime-detail`（JSON ASCII） | 保结构；解析在网关侧（同一进程树，JSON 解析稳定） | 网关需解析 JSON（已有 serde，成本零） |

**采纳方案 C 的简化形态**：`runtime_to_status` 在 `Status` 的 metadata 加 `x-runtime-code: runtime_queue_full`（单 ASCII 串，无 JSON 解析负担）+ 现有 message 字符串保 `format_error_chain`。`CoreError::detail()` 字段不跨 tonic——**detail 在网关侧从 `AppCoreError` 重建**（因 `CoreError` 在 slab-app-core 的 `rpc/client.rs` 反序列化点已可得原变体，detail 直接从变体取，不需跨网络）。

> **关键洞察**：`slab-runtime-core::CoreError` 在 slab-app-core 的 gRPC 客户端（[rpc/client.rs](../../../crates/slab-app-core/src/infra/rpc/client.rs)）反序列化点已可得完整变体（若 tonic status 经 `runtime_to_status` 反向映射回 CoreError——当前未做，是 §4.5 的小改动）。故 `runtime_code` + `detail` 在 slab-app-core 侧本地可得，**不需跨 tonic wire**——wire 只需保 `runtime_code` ASCII 让网关 fast-path 分支。

### 4.5 model_pack engine exhaustion 闭环

[slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md) Phase 2 的 engine exhaustion 已按当前代码消费本规范的 general envelope：

- `crates/slab-app-core/src/domain/services/model/runtime.rs` 在候选引擎全部耗尽时返回 `AppCoreError::RuntimeFailure`。
- `AppCoreErrorData::runtime_failure("runtime_engine_exhausted", detail)` 携带 `detail = { "model_id": ..., "attempts": [...] }`。
- `attempts[]` 复用 `RuntimeEngineAttemptError { engine, outcome, message }`。
- `bin/slab-server/src/error.rs` 继续保持外层 numeric `code = 5000` 兼容旧客户端，HTTP `data.runtime_code` 与 agent `Error.code` 均为 `"runtime_engine_exhausted"`。

因此生产路径不再使用 `ServerError::RuntimeEngineExhausted`、`AppCoreErrorData::RuntimeEngineExhausted` 或 `data.rollback` 特化信封；model-pack 已完成对 general envelope 的消费。

**前端收益**：前端按 `data.runtime_code` 分支——`runtime_engine_exhausted` 显示"所有引擎失败 + 回退详情"，`runtime_memory_pressure` 显示"显存不足，建议换 CPU 引擎"，`runtime_queue_full` 显示"推理队列满，请稍后重试"。无需解析 message 字符串。

---

## 5. 实施路线图（Phases）

> 每阶段：{涉及文件（clickable 相对链接）、关闭的审计发现、校验命令（[AGENTS.md:56-95](../../../AGENTS.md#L56-L95)）、退出标准}。Phase 顺序按风险优先级：**Phase 1 = 安全 R1（P0-1）+ 泄漏 G1（P0-6）**两个 HIGH 同期落地。

### Phase 1 — P0 安全/可靠性双 P0（R1 / G1 / G7）

- **涉及文件**：
  - [crates/slab-utils/src/path/mod.rs](../../../crates/slab-utils/src/path/mod.rs)：新增 `ensure_within_root` + `ensure_within_root_or_nearest` + `EnsureWithinRootError`。
  - [crates/slab-app-core/src/domain/services/model/catalog.rs](../../../crates/slab-app-core/src/domain/services/model/catalog.rs)：删 `validate_path`（:572-588），调用方改 `ensure_within_root`。
  - [crates/slab-app-core/src/domain/services/plugin/package.rs](../../../crates/slab-app-core/src/domain/services/plugin/package.rs)：删 `ensure_path_within`（:203-221），调用方改 `ensure_within_root_or_nearest`。
  - [crates/slab-plugin/src/registry.rs](../../../crates/slab-plugin/src/registry.rs)：`is_path_within_root`（:148-156）改 thin wrapper 委托 `slab_utils::path::ensure_within_root`。
  - [bin/slab-server/src/api/v1/workspace/handler.rs](../../../bin/slab-server/src/api/v1/workspace/handler.rs)：`workspace_info`（:534）canonicalize 失败 `warn!`。
  - [bin/slab-js-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs) + [bin/slab-python-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-python-runtime/src/api/jsonrpc/mod.rs)：`request()` 包 `tokio::time::timeout` + 超时/err 删 key（:84-95）。**临时方案**：直接在两文件改；Phase 2 抽出后由共享宿主继承。
- **关闭**：**R1 / P0-1**（路径单一真源，两侧 canonicalize）；**G1 / P0-6**（pending-map 超时清理）；**G7**（workspace_info canonicalize 失败 warn）。
- **校验**：
  - `bun run check:rust`（窄）。
  - `bun run test:rust:cargo`——新增 `slab-utils::path` 负向测试（symlink 逃逸 / `..` 遍历 / 绝对分量 / Windows 分隔符）；新增 `jsonrpc` pending-map 超时测试（mock handler 永不响应 → `request()` 在 timeout 后返 Err + key 删除）。
  - `bun run lint:rust`。
- **退出标准**：
  - `validate_path` / `ensure_path_within` 在仓库 grep 零命中（除 git history）。
  - `slab_utils::path::ensure_within_root` 是唯一实现。
  - `request()` 的 `rx.await` 在两 runtime 均包 timeout；超时后 `pending.lock().await.remove(&key)` 必执行。
  - `workspace_info` canonicalize 失败进 `warn!` 日志（手动注入坏 root 验证）。
  - 现有 `slab-plugin` 路径校验测试（`is_path_within_root` 调用点）全过（wrapper 语义不变）。

### Phase 2 — JSON-RPC 共享宿主 + task 监督（R5 / G2 / G3）

- **涉及文件**：
  - [crates/slab-jsonrpc/src/lib.rs](../../../crates/slab-jsonrpc/src/lib.rs) + 新 `crates/slab-jsonrpc/src/host.rs`：抽出 `JsonRpcRuntimeHost<H>` / `PendingMap` / `RequestHandler` trait / `serve_reader` / `drain_outbound`。继承 Phase 1 的 timeout + cap 修复（从两 runtime 的临时改法迁入共享宿主）。
  - [bin/slab-js-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-js-runtime/src/api/jsonrpc/mod.rs)：删重复定义，改 `use slab_jsonrpc::host::*`；实现 `impl RequestHandler for JsHandler`；main.rs 构造时传 `pending_cap=256, request_timeout=30s, concurrency_limit=16`。
  - [bin/slab-python-runtime/src/api/jsonrpc/mod.rs](../../../bin/slab-python-runtime/src/api/jsonrpc/mod.rs)：同上；保留 `plugin.call` 派发分支（:184-196）在 `PyHandler::handle_request` 内。
  - [crates/slab-app-core/src/infra/process_supervisor.rs](../../../crates/slab-app-core/src/infra/process_supervisor.rs)：`SupervisedStdioProcess` 加 `writer_abort`/`stderr_abort`/`stdout_abort`/`wait_abort` 字段 + `Drop` impl abort 四 task；spawn 改取 `abort_handle`。
- **关闭**：**R5 / P1-7**（共享 JSON-RPC 宿主，~150 行重复消除）；**G2**（per-request JoinSet + 并发上限 + panic 兜底）；**G3 / P2-2**（process_supervisor Drop abort）。
- **校验**：
  - `bun run check:rust`。
  - `bun run test:rust:cargo`——`slab_jsonrpc::host` dispatch 测试（正向 / 超时 / panic / 并发上限 4 个分支）；`process_supervisor` 测试加 drop-后-四-task-abort 断言。
  - `bun run test:server`（端到端覆盖 js/python runtime）。
- **退出标准**：
  - `JsonRpcRuntimeHost` 仅在 `slab_jsonrpc::host` 定义；js/py runtime 各自 mod.rs < 200 行（从 357/361 降）。
  - `drain_outbound` 在仓库 grep 仅 1 处定义。
  - dispatch 测试 4 分支通过；`process_supervisor` drop 后无 task 泄漏（`tokio::task::yield_now` + 断言四 abort_handle 已 abort）。

### Phase 3 — gRPC 样板消除（R6）

- **涉及文件**：
  - [bin/slab-runtime/src/api/handlers/mod.rs](../../../bin/slab-runtime/src/api/handlers/mod.rs)：新增 `forward<P,Q,D,R,F,G,Svc,Fut>` 泛型。
  - [bin/slab-runtime/src/api/handlers/candle_diffusion.rs](../../../bin/slab-runtime/src/api/handlers/candle_diffusion.rs) / [candle_transformers.rs](../../../bin/slab-runtime/src/api/handlers/candle_transformers.rs) / [ggml_diffusion.rs](../../../bin/slab-runtime/src/api/handlers/ggml_diffusion.rs) / [ggml_llama.rs](../../../bin/slab-runtime/src/api/handlers/ggml_llama.rs) / [ggml_whisper.rs](../../../bin/slab-runtime/src/api/handlers/ggml_whisper.rs) / [onnx.rs](../../../bin/slab-runtime/src/api/handlers/onnx.rs)：每个 handler 改调 `forward`。
- **关闭**：**R6**（gRPC handler 样板 ×80 消除）。
- **校验**：
  - `bun run check:rust`。
  - `bun run test:rust:cargo`——`slab-runtime` handler 测试全过（每个改造后的 handler 对同输入返同 `Status::code()` + 同 message；可加对比测试：改造前后 fixture 输入相同输出）。
- **退出标准**：
  - `map_err(application_to_status|proto_to_status)` 在 handlers/*.rs grep 零命中（集中在 `forward` 内两处）。
  - 6 个 handler 文件总行数降 ~40%（650 → ~400）。
  - 所有 handler 单测通过。

### Phase 4 — 错误跨边界保结构（F-Stack-3 general / P1-10）

- **涉及文件**：
  - [crates/slab-runtime-core/src/base/error.rs](../../../crates/slab-runtime-core/src/base/error.rs)：`CoreError` 加 `code()` + `detail()` 方法（:10-54 一带）。
  - [bin/slab-runtime/src/api/handlers/mod.rs](../../../bin/slab-runtime/src/api/handlers/mod.rs)：`runtime_to_status`（:50-82）把 `runtime_code` 塞进 tonic metadata（`x-runtime-code` ASCII header）。
  - [crates/slab-app-core/src/infra/rpc/client.rs](../../../crates/slab-app-core/src/infra/rpc/client.rs) + [runtime_gateway.rs](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs)：`map_runtime_error`（:216-230）读 tonic metadata `x-runtime-code`，未识别 status 保留 tonic `Code` 映 `runtime_grpc_<snake>`。
  - [crates/slab-app-core/src/error.rs](../../../crates/slab-app-core/src/error.rs)：`AppCoreError` 增携带 `runtime_code` + `detail` 字段（或新增 `RuntimeStructured` 变体）。
  - [bin/slab-server/src/error.rs](../../../bin/slab-server/src/error.rs)：`ServerError::Runtime(e)`（:212-251）不再折叠——映射 `runtime_code` → HTTP `data.runtime_code` + `detail`；`RuntimeMemoryPressure`（:294-296）独立 code；`ErrorResponse.data` 增 `runtime_code` 字段。
  - [bin/slab-server/src/api/v1/agent/handler.rs](../../../bin/slab-server/src/api/v1/agent/handler.rs)：`server_error_message`（:468）经 `agent_code_message`（[error.rs:96-144](../../../bin/slab-server/src/error.rs#L96-L144)）对 `Runtime(e)` arm 提取 `e.code()` 作为 agent `code`。
  - [packages/api/src/v1.d.ts](../../../packages/api/src/v1.d.ts)：`bun run gen:api` 刷新 ErrorResponse 类型（新增 `data.runtime_code`）。
- **关闭**：**F-Stack-3 general / P1-10**（gRPC 错误跨 HTTP/agent 边界保结构）。
- **校验**：
  - `bun run gen:api` → `bun run check:frontend`（前端类型不破）。
  - `bun run test:rust:cargo`——新增：mock `CoreError::QueueFull` 经全链路传播，断言 HTTP body `data.runtime_code == "runtime_queue_full"` + `data.detail == {"queue": ..., "capacity": ...}`；agent `Error.code == "runtime_queue_full"`。
  - `bun run test:server`——端到端：触发 `DriverNotRegistered` → HTTP 503 + `runtime_driver_not_registered`（前端可分支）。
- **退出标准**：
  - 每个 `CoreError` 变体有 stable `code`；HTTP body 携带；agent `code` 与 HTTP 一致。
  - `ServerError::Runtime(_)` 不再折叠为 `"internal_error"`。
  - model_pack Phase 2 的 engine exhaustion 已消费本信封（`data.runtime_code = "runtime_engine_exhausted"`，`detail` 携带 `model_id` 与 `attempts[]`）。
  - 前端 v1.d.ts 暴露 `data.runtime_code`。

### Phase 5 — 不变式与观测性清理（G6 / G9 / G8 测试）

- **涉及文件**：
  - [crates/slab-app-core/src/infra/rpc/client.rs](../../../crates/slab-app-core/src/infra/rpc/client.rs)：三处 `unreachable!()`（:214/:440/:542）改 `return Err(typed)`。
  - [crates/slab-app-core/src/domain/services/model/config_document.rs](../../../crates/slab-app-core/src/domain/services/model/config_document.rs)：`ensure_json_object`（:524）改注释 + `debug_assert` + 兜底路径（不 `unreachable!`）。
  - [crates/slab-app-core/src/infra/model_packs/mod.rs](../../../crates/slab-app-core/src/infra/model_packs/mod.rs)：`read_persisted_model_config_from_pack_bytes`（:307）SHA 不匹配分支加 `warn!`。
  - [crates/slab-config/src/pmid_service.rs](../../../crates/slab-config/src/pmid_service.rs)：`#[cfg(test)]` helper 加 `openai_provider(api_key, api_key_env)` 单一构造器；OpenAI URL fixture 只保留一个硬编码点；新增 runtime launch PMID 到 `LaunchConfig.profiles` 的映射。
  - [crates/slab-config/src/settings/document.rs](../../../crates/slab-config/src/settings/document.rs) / [settings/pmid.rs](../../../crates/slab-config/src/settings/pmid.rs) / [descriptor.rs](../../../crates/slab-config/src/descriptor.rs)：新增 `runtime.launch.{server,desktop}.{bind_host,base_port}` 默认值、PMID 与约束。
- **关闭**：**G6 / P2-4**（unreachable! 改 return Err / 注释证明）；**G9**（manifest_sha256 不匹配 warn）；**G8**（OpenAI fixture 去重 + runtime launch bind/base port PMID 化）。
- **校验**：
  - `bun run check:rust`。
  - `bun run test:rust:cargo`——rpc/client.rs 重试循环测试全过（return Err 路径覆盖）；config_document.rs 测试全过。
- **退出标准**：
  - 仓库 grep `unreachable!` 在 Track C 范围内零命中（三处 rpc/client.rs + 一处 config_document.rs）。
  - SHA 不匹配进 `warn!` 日志（手动注入坏 SHA 验证）。
  - 测试代码 `https://api.openai.com/v1` grep 仅 1 处（`openai_provider` helper）。
  - `runtime.launch.*.base_port` 写路径拒绝 `<1` 与 `>65535`，settings schema 生成物包含四个新字段。

### 5.1 验证策略（对齐审计 §6.4）

- **Phase 1**：`slab-utils::path` 负向测试覆盖 symlink 逃逸 / `..` 遍历 / 绝对分量；`jsonrpc` pending-map 超时测试。
- **Phase 2**：`slab_jsonrpc::host` dispatch 测试 4 分支（正向/超时/panic/并发上限）；`process_supervisor` drop-abort 测试。
- **Phase 3**：handler 改造前后输出对比测试（同 fixture 输入，同 `Status` 输出）。
- **Phase 4**：全链路 `CoreError` → HTTP body `runtime_code` + `detail` 传播测试；agent `code` 一致性测试。
- **Phase 5**：rpc/client.rs `return Err` 路径覆盖测试。
- **整体**：每 Phase 用最窄校验命令先验证（[AGENTS.md:18](../../../AGENTS.md#L18)），再扩到 workspace（`bun run check` + `bun run test`）。

---

## 6. 验证策略

### 6.1 单元测试矩阵

| 测试点 | 覆盖条款 | 期望 |
|--------|---------|------|
| `slab_utils::path::ensure_within_root` 负向（symlink/`..`/绝对分量/Windows 分隔符/不存在） | R1 / P0-1 | 全部拒绝；越界返 `EscapesRoot`；不存在返 `PathCanonicalize` |
| `slab_utils::path::ensure_within_root_or_nearest` 待创建文件 | R1 / P0-1 | 容忍不存在路径，规范化到最近祖先再 starts_with |
| `slab_jsonrpc::host::JsonRpcRuntimeHost::request` 超时 | G1 / P0-6 | mock handler 永不响应 → 30s 后返 `Err("... timed out ...")`；pending-map 删 key |
| `slab_jsonrpc::host::serve_reader` 并发上限 | G2 | mock handler 慢 → 第 17 个请求被拒（concurrency_limit=16） |
| `slab_jsonrpc::host::serve_reader` panic 兜底 | G2 | mock handler panic → 对端收到 `error` 响应（非永久挂起） |
| `process_supervisor::SupervisedStdioProcess::drop` | G3 / P2-2 | drop 后四 abort_handle 全 aborted；无 task 写向 closed pipe |
| `handlers::forward` 对比测试 | R6 | 改造前后同输入返同 `Status::code()` + message |
| `CoreError::code/detail` 全变体 | F-Stack-3 / P1-10 | 每变体返 stable snake_case code；detail 序列化字段匹配 |
| 全链路 `CoreError::QueueFull` → HTTP body | F-Stack-3 / P1-10 | `data.runtime_code == "runtime_queue_full"` + `data.detail == {queue, capacity}` |
| 全链路 → agent `Error.code` | F-Stack-3 / P1-10 | agent `code` == HTTP `data.runtime_code` |
| `rpc::client` retry loop `return Err` 路径 | G6 / P2-4 | 排他界 `1..N` 模拟下返 typed error 而非 panic |
| `model_packs::read_persisted_model_config_from_pack_bytes` SHA 不匹配 | G9 | 返 `Ok(None)` + `warn!` 日志含 expected/actual |

### 6.2 集成/端到端验证

- **`bun run test:server`**：js-runtime + python-runtime 端到端 plugin 调用（验证 Phase 2 共享宿主不破坏现有协议）。
- **`bun run test:browser`**：前端 agent WS 错误响应（验证 Phase 4 agent `code` 字段）。
- **手动注入故障**：① 杀 js-runtime 子进程 → `request()` 应在 timeout 后返错（Phase 1/2）；② 触发 runtime OOM → HTTP `runtime_memory_pressure` code（Phase 4）；③ drop `SupervisedStdioProcess` → 监督日志显示四 task abort（Phase 2）。

### 6.3 静态检查

- `bun run lint:rust`（clippy）：覆盖 `unreachable!` 残留、`unwrap_or_default` 静默吞错、未使用 import。
- `bun run check:rust`：类型检查。
- `bun run gen:api` 后 `git diff packages/api/src/v1.d.ts` 审阅 ErrorResponse 变更（Phase 4）。

---

## 附录 A：审计发现 → 计划条款 闭环追溯

| 审计发现（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md)） | §6 行动项 | 本计划条款 | 实现阶段 | 状态 |
|---|---|---|---|---|
| **§5.1 R1** 路径校验 3 套实现混杂（catalog.rs:572-588 / package.rs:203-221 / registry.rs:148-156） | **P0-1** | §3.1 插件 root containment 归 `slab_utils::path::ensure_within_root` 单一真源，两侧 canonicalize；registry/package 保 thin wrapper/错误映射；catalog 模型路径改绝对路径 helper，不套 root containment | Phase 1 | 关闭 |
| **§5.1 R5** js/py JSON-RPC 宿主 ~95% 重复 ~150 行 | **P1-7** | §3.2 抽 `JsonRpcRuntimeHost`/`RequestHandler`/`serve_reader`/`drain_outbound` 入 `slab_jsonrpc::host` | Phase 2 | 关闭 |
| **§5.1 R6** gRPC handler 样板 ×80 | （§6 P2 隐含） | §3.6 `forward<Req,Resp,S>` 泛型消除 73 处 `map_err` | Phase 3 | 关闭 |
| **§5.2 G1** JSON-RPC pending-map 无界泄漏 | **P0-6** | §3.3 `rx.await` 包 timeout + 超时/err 删 key + cap（256） | Phase 1（临时）+ Phase 2（共享宿主继承） | 关闭 |
| **§5.2 G2** 每请求 tokio::spawn 无背压/无监督/panic 静默死 | **P2-2**（部分） | §3.4 per-request JoinSet + 并发上限（16）+ panic 兜底响应 | Phase 2 | 关闭 |
| **§5.2 G3** process_supervisor 四 fire-and-forget task 无 Drop abort | **P2-2**（部分） | §3.5 `SupervisedStdioProcess` 持四 `AbortHandle` + `Drop` impl abort | Phase 2 | 关闭 |
| **§5.2 G6** `unreachable!()` 承重且脆弱（rpc/client.rs:214/440/542 + config_document.rs:524） | **P2-4** | §3.7 retry loop 末尾改 `return Err`；config_document 改注释 + debug_assert + 兜底 | Phase 5 | 关闭 |
| **§5.2 G7** workspace_info canonicalize 失败静默当无 overlay | （§6 隐含 LOW） | §3.1（末尾）canonicalize 失败 `warn!` + 区分"用默认" | Phase 1 | 关闭 |
| **§5.2 G8** 硬编码 OpenAI base URL ×3 + 端口常量 | **P2-9**（G8 部分） | §3.9 OpenAI URL 三处证实为测试 fixture（审计纠错）→ `openai_provider` helper 去重；端口常量补 `runtime.launch.{server,desktop}.{bind_host,base_port}` PMID 与 schema | Phase 5 | 关闭 |
| **§5.2 G9** manifest_sha256 不匹配静默重置下载状态 | （§6 隐含 LOW） | §3.10 SHA 不匹配分支加 `warn!` | Phase 5 | 关闭 |
| **§2.3 F-Stack-3 general** CoreError 变体跨 HTTP 边界压平为不透明字符串 | **P1-10** | §3.8 + §4 `CoreError::code()`/`detail()`；`ServerError::Runtime` 不折叠；`ErrorResponse.data.runtime_code`；agent `code` 统一 | Phase 4 | 关闭（general） |
| **§2.3 F-Stack-3 model-pack engine exhaustion** | （model_pack P2） | §4.5 model-pack 已消费本规范 general envelope（`data.runtime_code = "runtime_engine_exhausted"`，`detail = { model_id, attempts[] }`） | model_pack Phase 2 | 关闭（model-pack 已消费） |

> **审计纠错记录（2026-06-18 核实）**：
> 1. **G6 config_document 路径**：审计引 `crates/slab-config/src/config_document.rs:543`；实际为 `crates/slab-app-core/src/domain/services/model/config_document.rs:524`（不同 crate，行号 524）。本计划用正确路径。
> 2. **G8 OpenAI URL 性质**：审计称"硬编码 OpenAI base URL ×3"；实际三处（pmid_service.rs:1685/1723/1777）**全部位于 `#[cfg(test)]` 测试 fixture**，非生产路径。生产 OpenAI base URL 由 PMID `chat.providers[]` 注入。本计划把 G8 的 OpenAI 部分降级为测试去重。
> 3. **R1 第三套语义**：审计对 `is_path_within_root`（slab-plugin/registry.rs:148-156）含糊描述；实际**两侧 canonicalize，是三者中最正确**。本计划选其为统一真源的语义基线。
> 4. **R6 计数**：审计称"×80"；实际 grep `map_err(application_to_status|proto_to_status)` = **73** 处；算上完整 handler 模式（request_id 提取 + decode + service + encode）约 ×80。本计划用 73 精确数。

---

## 附录 B：与既有契约的边界（对齐 [AGENTS.md](../../../AGENTS.md)）

### B.1 推理链路边界（[AGENTS.md:23](../../../AGENTS.md#L23)）

```
bin/slab-app → bin/slab-server → crates/slab-app-core runtime supervisor
            → GrpcGateway → bin/slab-runtime → crates/slab-runtime-core
```

本计划改动：
- **`crates/slab-runtime-core`**：仅加 `CoreError::code()`/`detail()` 方法（加性，非破坏）。
- **`bin/slab-runtime`**：handler 改调 `forward`（Phase 3）；`runtime_to_status` 加 tonic metadata（Phase 4）。不动 backend 加载实现。
- **`crates/slab-app-core`**：`rpc/client.rs`（G6）、`rpc/runtime_gateway.rs`（F-Stack-3）、`process_supervisor.rs`（G3）、`model/catalog.rs` + `plugin/package.rs`（R1）、`model/config_document.rs`（G6）、`model_packs/mod.rs`（G9）。**不**改 domain services 业务逻辑、不改 DB repository。
- **`bin/slab-server`**：`error.rs`（F-Stack-3）、`workspace/handler.rs`（G7）、`agent/handler.rs`（F-Stack-3 agent 入口）。
- **`bin/slab-js-runtime` / `bin/slab-python-runtime`**：`jsonrpc/mod.rs`（R5/G1/G2）。
- **`crates/slab-utils`**：新增 `path::ensure_within_root`（R1）。
- **`crates/slab-jsonrpc`**：新增 `host` 模块（R5）。
- **`crates/slab-plugin`**：`registry.rs::is_path_within_root` 改 thin wrapper（R1）——保 bool API 兼容。

### B.2 Plugin / LSP / JSON-RPC 边界（[AGENTS.md:34-46](../../../AGENTS.md#L34-L46)）

- **Plugin dispatch 边界不变**：`bin/slab-app → bin/slab-server /v1/plugins/rpc → WebSocket JSON-RPC 2.0 → crates/slab-app-core`（[AGENTS.md:36](../../../AGENTS.md#L36)）。本计划 Phase 2 抽 `slab_jsonrpc::host` **仅影响 js-runtime/python-runtime 的内部宿主管道**（host 进程 ↔ plugin runtime 子进程的 stdin/stdout JSON-RPC），**不动** `bin/slab-server ↔ slab-app-core` 的 WebSocket JSON-RPC。两层 JSON-RPC 协议形态（2.0 信封）共享 `slab_jsonrpc` 信封原语，但宿主管道是 sidecar 内部，不跨 WebSocket 边界。
- **JS/Python plugin runtime 调用遵循 JSON-RPC 2.0**（[AGENTS.md:40](../../../AGENTS.md#L40)）：本计划 Phase 2 的 `RequestHandler` trait 不改协议，仅重组织宿主态代码。
- **Plugin WebView 命令派发**（[AGENTS.md:42](../../../AGENTS.md#L42)）：不在本计划范围。
- **LSP 边界**（[AGENTS.md:43-46](../../../AGENTS.md#L43-L46)）：不在本计划范围。

### B.3 监督者 vs 网关职责切分（process vs engine）

> 这是 [slab-model-pack-2026-06-17.md §4.4](slab-model-pack-2026-06-17.md) 已确立的边界，本计划**遵守并强化**：

- **`process_supervisor`**（本计划 §3.5 G3 修复对象）：拥有**进程**存活——崩溃子进程带退避重启。Drop 修复仅让 handle 生命周期与四 IO task 一致，**不改"进程重启"职责**。
- **`GrpcGateway` / `rpc/client.rs`**：拥有**引擎**存活——同模型换 backend 重试（model_pack Phase 2 的跨引擎回退）+ gRPC 重试（本计划 G6 的 retry loop）。
- **两者从不重叠**：transport 瞬时错误（channel 中途死亡）属进程监督者职责（重启子进程），网关不得把传输抖动伪装成换引擎；引擎致命错误（OOM/缺编译）属网关职责（换 backend），监督者不感知。

本计划 Phase 4 的 `runtime_code` 信封让两层错误在前端可区分：`runtime_backend_shutdown`（进程级，提示"runtime 重启中"）vs `runtime_driver_not_registered`（引擎级，提示"换引擎"）vs `runtime_engine_exhausted`（model_pack，所有引擎耗尽）。

### B.4 crate 依赖方向（[AGENTS.md:27-29](../../../AGENTS.md#L27-L29)）

- 跨 crate 契约走 `crates/slab-types` / `crates/slab-proto`：本计划 `CoreError::code()` 在 `slab-runtime-core` 定义，`AppCoreError` 在 `slab-app-core` 携带，`ServerError` 在 `bin/slab-server` 映射——依赖方向 `bin/slab-server → slab-app-core → slab-runtime-core`（**正向**，无环）。
- **`slab_utils::path::ensure_within_root`**：`slab-utils` 是底层工具 crate，`slab-app-core` 与 `slab-plugin` 均可依赖（[AGENTS.md:27](../../../AGENTS.md#L27) 隐含）。**不**形成 `slab-app-core → slab-plugin` 反向依赖——这正是把第三套 impl 从 `slab-plugin` 迁到 `slab-utils` 的理由。
- **`slab_jsonrpc::host`**：`slab-jsonrpc` 已被 js-runtime/python-runtime 依赖（信封原语），加 host 模块不新增依赖方向。
- **`crates/slab-app-core` HTTP-free**（[AGENTS.md:28](../../../AGENTS.md#L28)）：本计划 Phase 4 的 `AppCoreError` 携带 `runtime_code` + `detail` 是**crate-internal 数据**，HTTP 映射在 `bin/slab-server/error.rs` 完成——slab-app-core 本身不产 HTTP response。

### B.5 API surface 与迁移（[AGENTS.md:25-26](../../../AGENTS.md#L25-L26)）

- **扩展现有 `/v1/*` API**：本计划 Phase 4 的 `ErrorResponse.data.runtime_code` 是**加性字段**，不改 endpoint shape。前端按 u16 `code` 分支的逻辑（向后兼容）仍工作；新逻辑按 `data.runtime_code` 分支（更精确）。
- **`bun run gen:api`**（Phase 4）：刷新 `packages/api/src/v1.d.ts` 暴露 `data.runtime_code`。

### B.6 SQLx 迁移（[AGENTS.md:32](../../../AGENTS.md#L32)）

本计划**不动** DB schema（G-scope 全部在跨进程层，不碰持久化）。SQLx 迁移 append-only 约束不影响本计划。

---

## 7. 审计 §6 行动表闭环确认

> 本计划是审计 §6 的**收口计划**。下表确认 §6 全部 P0/P1/P2 条款的归属与状态。

### §6.1 P0（阻断性 / 安全 / 数据完整性）

| # | 行动 | 归属计划 | 状态 |
|---|------|---------|------|
| P0-1 | 统一路径包含校验 | **本计划 §3.1 / Phase 1** | ✅ 关闭 |
| P0-2 | 修 `json_set` 绕过校验 | 存储-契约计划 | 委托 |
| P0-3 | `variant.id` 唯一性 | model_pack Phase 1/5 | 委托 |
| P0-4 | asset-ref 校验前移 | model_pack Phase 1/5 | 委托 |
| P0-5 | `tasks.result_data` 单一真源 | 存储-契约计划 | 委托 |
| P0-6 | JSON-RPC pending-map 超时清理 | **本计划 §3.3 / Phase 1+2** | ✅ 关闭 |
| P0-7 | `AgentThreadRow.config_json` 非 Option | 存储-契约计划 | 委托 |

### §6.2 P1（系统性设计缺陷）

| # | 行动 | 归属计划 | 状态 |
|---|------|---------|------|
| P1-1 | env→PMID 单向桥 | PMID-设置计划 | 委托 |
| P1-2 | 设置热重载扩宽 | PMID-设置计划 | 委托 |
| P1-3 | 5 层 logging 级联 | PMID-设置计划 | 委托 |
| P1-4 | telemetry.metrics_exporter schema | PMID-设置计划 | 委托 |
| P1-5 | 缺 CHECK 的列补约束 | 存储-契约计划 | 委托 |
| P1-6 | 统一未知状态默认 | 存储-契约计划 | 委托 |
| P1-7 | 抽共享 JSON-RPC 宿主 | **本计划 §3.2 / Phase 2** | ✅ 关闭 |
| P1-8 | RuntimePresets 单一构造器 | model_pack（R2/R3） | 委托 |
| P1-9 | 发布子文档 schema | model_pack Phase 6 | 委托 |
| P1-10 | gRPC 错误跨边界保结构 | **本计划 §3.8 + §4 / Phase 4** | ✅ 关闭（general） |

### §6.3 P2（清理 / 可维护性 / 命名）

| # | 行动 | 归属计划 | 状态 |
|---|------|---------|------|
| P2-1 | 数据路径 `.ok()`/`unwrap_or_default()` warn（G4） | 数据部分→存储-契约；配置部分→PMID-设置 | 委托（split） |
| P2-2 | process_supervisor Drop + JSON-RPC JoinSet + panic 兜底 | **本计划 §3.4/§3.5 / Phase 2** | ✅ 关闭 |
| P2-3 | 脱敏由 `writeOnly` 驱动 | PMID-设置计划 | 委托 |
| P2-4 | `unreachable!()` 改 `return Err` | **本计划 §3.7 / Phase 5** | ✅ 关闭 |
| P2-5 | 命名治理 | 存储-契约 + model_pack | 委托 |
| P2-6 | 文档腐烂清理 | 存储-契约计划 | 委托 |
| P2-7 | config_json 暴露 + DTO 字段 | 存储-契约 + model_pack | 委托 |
| P2-8 | manifest version/default_preset/status | model_pack | 委托 |
| P2-9 | `$config`→`$document` + OpenAI base URL const | `$config`→`$document` 归 model_pack；**OpenAI URL 归本计划 §3.9 / Phase 5**（测试去重）；runtime launch 端口由本计划补最小 PMID 闭环 | G8 关闭；`$config` 命名委托 model_pack |

### 跨计划归属确认

1. **F-Stack-3 model-pack engine exhaustion**：已由 model-pack 消费本计划 general envelope，生产路径使用 `runtime_engine_exhausted` + `detail.attempts[]`，不再作为 Track C 残留。
2. **G4 数据路径 `.ok()`/`unwrap_or_default()`**：data-path 由存储-契约计划承接/闭环，config-path（`parse_env` / setting value）由 PMID/settings 计划承接/闭环；Track C 不重复承接。

### 闭环声明

**本计划（Track C：跨进程可靠性、冗余治理与安全收敛）+ model_pack 计划（[slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md)）+ 存储-契约计划 + PMID-设置计划，四份计划合计覆盖审计 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §6 行动表的全部 P0（7/7）、P1（10/10）、P2（9/9）条款。** Track C 无未承接残留；G4 由姊妹计划归属闭环。审计 §6 全表闭环。

---

*本计划由首席可靠性/安全架构师主导，所有 path:line 证据在 2026-06-18（审计发布次日）由主审计员直接读源码重新核实——审计原文行号有 +1~+9 行漂移，且发现两处需纠错的事实（G6 config_document 实际路径在 slab-app-core 而非 slab-config；G8 OpenAI URL 三处全部位于测试代码非生产路径；R1 第三套 `is_path_within_root` 实为三者中最严谨实现）。本文引用当前工作树行号。规范以 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §5 的 R1/R5/R6 + §5.2 的 G1/G2/G3/G6/G7/G8/G9 + §2.3 F-Stack-3 general case 为闭环目标，与三份姊妹计划一起构成 §6 全表闭环。*
