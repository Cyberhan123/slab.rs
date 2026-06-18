# PMID 回显与设置可靠性专项设计 (2026-06-17)

> **文档定位**：本规划书基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §3.2 的 11 项 PMID 回显缺陷（PMID-F1–F11）、§2.3 跨边界缺陷的 settings 部分（F-Stack-1/F-Stack-2）、§5 G4（config-path 静默吞错）/G5（脱敏白名单），以及 §6 行动项 P1-1/P1-2/P1-3/P1-4/P2-3，在 `crates/slab-config` 回显引擎 + `crates/slab-app-core::SettingsService` + `bin/slab-server` 鉴权/启动管线 + `packages/api` 前端 fetch 客户端上，演进出一套**回显诚实、单源真源、声明即强制、热重载边界可观测**的设置可靠性规范。
>
> **方法**：首席配置架构师主导，逐 finding 直接读源码落地核实。所有 `path:line` 证据已对齐 **2026-06-18 工作树**（审计原件为 2026-06-17，期间发生若干行漂移与一处目录迁移——已在 §1.4 与各 finding 注明当前正确坐标）。审计 §1.3 已对抗式核实 `secret()`/`redact_setting_value` 脱敏正确，本规范直接建立其上、**不重新论证脱敏正确性**，而是把脱敏的**驱动方式**从硬编码白名单升级为 schema `writeOnly` 单源。
>
> **读者**：实现该规范的工程师与审计员。本文为**契约级设计**，非概念稿。

---

## 1. 背景与目标

### 1.1 现状与痛点

`slab.rs` 的设置体系是一个**双源（env + PMID）+ 回显（PMID）+ 热重载（窄）+ 脱敏（硬编码）**的四层结构。审计暴露的核心痛点（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §1.1 第三短板、§3.2、§2.3）：

1. **环境变量与 PMID 双源断开，且优先级未定义**（PMID-F1 / F-Stack-2）：
   - 鉴权走 `Config::admin_api_token`（[app_config.rs:79](../../../crates/slab-config/src/app_config.rs#L79)），其值来自 `SLAB_ADMIN_TOKEN` env（[app_config.rs:170](../../../crates/slab-config/src/app_config.rs#L170)）；而设置页的 `server.admin.token` PMID（[descriptor.rs:402](../../../crates/slab-config/src/descriptor.rs#L402)）写回 settings.json，**与鉴权完全不相通**。`auth_middleware`（[auth.rs:22-26](../../../bin/slab-server/src/api/middleware/auth.rs#L22-L26)）读 `state.context.config.admin_api_token` —— 用户在设置页改 `server.admin.token` 不改变实际鉴权要求。
   - 同理 `SLAB_BIND`（[app_config.rs:157](../../../crates/slab-config/src/app_config.rs#L157)）vs PMID `server.address`（[descriptor.rs:395](../../../crates/slab-config/src/descriptor.rs#L395)）；`SLAB_LOG`（[app_config.rs:159](../../../crates/slab-config/src/app_config.rs#L159)）vs `logging.*` PMID；`SLAB_QUEUE_CAPACITY`/`SLAB_BACKEND_CAPACITY`（[app_config.rs:163-164](../../../crates/slab-config/src/app_config.rs#L163-L164)）vs `runtime.capacity.*` PMID。
   - 前端 `createSlabApiFetchClient`（[index.ts:29-37](../../../packages/api/src/index.ts#L29-L37)）只设 `baseUrl`+`fetch`，**全仓库无 Authorization 拦截器**；鉴权 100% 依赖 loopback 旁路（[auth.rs:44](../../../bin/slab-server/src/api/middleware/auth.rs#L44) → `is_loopback_bind_address`，[:61-73](../../../bin/slab-server/src/api/middleware/auth.rs#L61-L73)）。`bind_address=0.0.0.0` 而未设 token → 前端无 token 可发 → 401 全不通，桌面应用"看似启动但全部接口不通"，UI 无引导。

2. **设置热重载范围过窄，改了不生效且无 UI 信号**（F-Stack-1）：
   - `SettingsService::update_setting`（[settings.rs:31-43](../../../crates/slab-app-core/src/domain/services/settings.rs#L31-L43)）仅当 `affects_agent_runtime(pmid)` 为真才触发 `agent_runtime.reload()`；而 `affects_agent_runtime`（[settings.rs:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)）只匹配 `agent.hooks.*`/`agent.memories.*` 两个前缀。
   - `runtime.*`/`inference.*`/`model.*`/`chat.*`/`diffusion.*`/`server.*` 的写入**持久化到 DB 但无任何 live reload**，运行中的 slab-runtime 与推理 backend 不会被通知。`sync_runtime_restart_states()` 是**按需拉取**（查状态时）而非设置变更时推送。用户改了 workers/context_length/device，必须手动重载模型或重启 runtime，UI 无任何提示。

3. **回显层对枚举/数值/错误不诚实**（PMID-F3/F4/F5/F6/F7/F8/F10）：
   - `telemetry.metrics_exporter` 是 tagged union（[OtelExporter](../../../crates/slab-otel/src/config.rs#L57-L74) `{none, local_file{directory}, otlp_http{...}}`），但回显层 `value_type = Object`（[pmid_service.rs:852](../../../crates/slab-config/src/pmid_service.rs#L852)）、标 `multiline`、**无 `json_schema`**（`json_schema()` 函数 [pmid_service.rs:940-959](../../../crates/slab-config/src/pmid_service.rs#L940-L959) 对 `metrics_exporter` 返 `None`）。客户端看到自由 object，无 enum 提示（PMID-F3）。
   - `OtelSettings` 暴露 `exporter`/`trace_exporter`/`metrics_exporter` 三字段（[config.rs:112-120](../../../crates/slab-otel/src/config.rs#L112-L120)），descriptor catalog 只注册 `metrics_exporter`；测试（[pmid_service.rs:2150-2153](../../../crates/slab-config/src/pmid_service.rs#L2150-L2153)）显式断言 `slab_home`/`exporter`/`trace_exporter` 不被回显。logs/traces 的 exporter 运行期从 `slab_home` 注入（[config.rs:136-141](../../../crates/slab-otel/src/config.rs#L136-L141)），哪个 exporter 控制哪个信号在 catalog 无文档（PMID-F4）。
   - `setting_value_from`（[descriptor.rs:434-436](../../../crates/slab-config/src/descriptor.rs#L434-L436)）`.unwrap_or_default()` 吞 `Serialize` 失败 → 静默 `Null`（PMID-F5）。
   - `SettingValue::from(Value::Number)`（[view.rs:42-51](../../../crates/slab-config/src/view.rs#L42-L51)）u64 溢出 i64 后静默降 `as_f64().unwrap_or_default()`；对称地 [:68-70](../../../crates/slab-config/src/view.rs#L68-L70) NaN/±Inf → `Null`（PMID-F6）。
   - `parse_env`（[app_config.rs:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)）`v.parse().ok().unwrap_or(default)`：`SLAB_QUEUE_CAPACITY=abc` 静默变默认 64；`SLAB_ENABLE_SWAGGER`（[app_config.rs:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)）除 `"0"`/`"false"` 外一切视为 enabled —— `SLAB_ENABLE_SWAGGER=no` 反而启用（PMID-F7 / G4 config-path 部分）。
   - `minimum_value`（[pmid_service.rs:924-938](../../../crates/slab-config/src/pmid_service.rs#L924-L938)）为数值 PMID 声明 `minimum=0`，但写路径 `set_setting_value`（[descriptor.rs:438-447](../../../crates/slab-config/src/descriptor.rs#L438-L447)）仅做"能否反序列化"检查，`runtime.capacity.queue = -5` 不会被拒（PMID-F8）。
   - `value_type`（[pmid_service.rs:844-894](../../../crates/slab-config/src/pmid_service.rs#L844-L894)）把 `Integer` 与 `Number` 都映成 `SettingValueType::Integer`；枚举无 `Float`（[view.rs:82-91](../../../crates/slab-config/src/view.rs#L82-L91)）。任何 f64 PMID 被误标 Integer（PMID-F10）。

4. **脱敏白名单硬编码两处，未由 schema `writeOnly` 驱动**（PMID-F9 / G5 / P2-3）：
   - `secret()`（[pmid_service.rs:971-975](../../../crates/slab-config/src/pmid_service.rs#L971-L975)）与 `redact_setting_value()`（[:977-983](../../../crates/slab-config/src/pmid_service.rs#L977-L983)）各维护一份硬编码路径列表（`server.admin.token`/`providers.registry`/`agent.tools.websearch.providers`），需**两处同步**。schema 文档已标 `writeOnly:true`（[document.rs:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405)，测试 [pmid_service.rs:1682-1701](../../../crates/slab-config/src/pmid_service.rs#L1682-L1701)），但回显层**不消费** `writeOnly` 来驱动脱敏——是平行维护。`agent.tools.mcp.servers` 的 `env` 据设计为 env-var 名引用（[document.rs:1476-1479](../../../crates/slab-config/src/settings/document.rs#L1476-L1479)，"Secret values are resolved at launch and are not stored here"）非明文，故当前安全；但 `mcp_servers_json_schema()` 无任何 `writeOnly` 字段，未来若存明文 secret 会漏。
   - **审计 §1.3 已对抗式核实**：脱敏**实现本身正确**（`secret_setting_views_redact_literal_secret_values` 测试在 [pmid_service.rs:1714](../../../crates/slab-config/src/pmid_service.rs#L1714) 钉死，`redact_api_key_fields` 对 leaf 与对象/数组内 `api_key` 字段均正确脱敏，写回经 `restore_secret_placeholders`（[:1016](../../../crates/slab-config/src/pmid_service.rs#L1016)）保留原值）。本规范**不重造脱敏算法**，只把"哪些路径要脱敏"的**判定从硬编码迁移到 schema `writeOnly`**。

5. **5 层并行 logging override 无级联解析器**（PMID-F2）：
   - 5+ 并行 logging 组：根 `logging`（[document.rs:154-165](../../../crates/slab-config/src/settings/document.rs#L154-L165)）、`runtime.logging`、`runtime.ggml.logging`、`runtime.ggml.backends.{llama,whisper,diffusion}.logging`、`runtime.candle.logging`/`runtime.onnx.logging`/`server.logging`。根是 `LoggingConfig`，其余是 `LoggingOverrideConfig = Option<...>`（[document.rs:173-185](../../../crates/slab-config/src/settings/document.rs#L173-L185)）。
   - 引擎仅解析**一个** fallback：`runtime_log_dir = settings.runtime.logging.path.or_else(|| settings.logging.path)`（[pmid_service.rs:180-181](../../../crates/slab-config/src/pmid_service.rs#L180-L181)）。对 `level`/`json`，`load_config` 内**无任何级联解析**——每个叶字段只经 `setting_value` 读取，消费者各自继承。用户设 `runtime.ggml.backends.llama.logging.level` 无法从回显判断它是否覆盖 `runtime.logging.level`，优先级**隐式且无文档**。

6. **PMID shim 空抽象**（PMID-F11）：`domain/services/pmid.rs` 全文 `pub use slab_config::PmidService;`（[pmid.rs:1](../../../crates/slab-app-core/src/domain/services/pmid.rs#L1)）。非死代码，但语义空——无 app-core 特定抽象。

### 1.2 目标

| 目标 | 衡量标准 |
|------|----------|
| **G1 单源真源** | env 仅作**首次启动种子**（seed-once），运行期生效值以 settings.json (PMID) 为准；回显诚实反映"哪个源在生效"。对齐 PMID-F1 / F-Stack-2。 |
| **G2 回显诚实** | 每个回显字段的真实类型、真实生效值、约束边界都准确上报：tagged union 发射 `json_schema`、Float 与 Integer 区分、u64 不降精度、NaN/Inf 入口拒绝、序列化错误传播。对齐 PMID-F3/F4/F5/F6/F10。 |
| **G3 声明即强制** | schema 声明的 `minimum`/`maximum`/`writeOnly` 全部在写路径或回显路径被消费，无"声明而不强制"。对齐 PMID-F8 / PMID-F9 / G5 / P2-3。 |
| **G4 静默降级不可接受** | 所有数据/配置路径的 `.unwrap_or_default()`/`.ok()` 在失败时至少 `warn!`，畸形 env 解析失败显式报错而非回退默认。对齐 PMID-F7 / G4 config-path 部分。 |
| **G5 热重载边界可观测** | 用户在设置页看到的每个 PMID 明确标注"立即生效 / 需重启 / 需手动重载模型"；运行期不消费热重载的子系统，UI 必须显示 needs-restart 信号。对齐 F-Stack-1 / P1-2。 |

### 1.3 非目标（指向兄弟计划）

- **不改动 `model_pack` 配置 Schema**（`crates/slab-model-pack`）——属 [slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md)。该计划 Phase 3 **增量扩展** `secret()` 白名单以纳入 `download.<handle>` 一个新子树，**最终应被本规范的 writeOnly 驱动机制吸收**（见 §3.7 的并轨说明）。
- **不改动 API ↔ DB 契约**（`tasks.result_data` 信封化、CHECK 约束、布尔 INTEGER）——属另一计划（DB 契约专项）。
- **不改动跨进程 JSON-RPC 可靠性**（pending-map 无界泄漏）与 gRPC 错误跨边界结构（F-Stack-3）——属另一计划。本规范只在 §3.1 的 token 注入路径与 F-Stack-2 的 401 引导上与前端契约**轻接触**。
- **不改动路径包含校验三套实现**（§5 R1 安全）——属安全专项。
- **不重做 `ref://` 寻址、子文档 kind 体系**——属 model_pack 计划。

### 1.4 行号与目录漂移更正（2026-06-18 复核记录）

> 审计原件为 2026-06-17。复核期间确认以下漂移，本规划一律采用**当前正确坐标**：

| 审计引用 | 实际位置（2026-06-18） | 状态 |
|---------|----------------------|------|
| `crates/slab-app-core/src/app_config.rs` | **`crates/slab-config/src/app_config.rs`** | 目录迁移。`Config::from_env_source` 在 [:146](../../../crates/slab-config/src/app_config.rs#L146)（审计 :128）；`SLAB_BIND` [:157](../../../crates/slab-config/src/app_config.rs#L157)（审计 :139）；`SLAB_LOG` [:159](../../../crates/slab-config/src/app_config.rs#L159)；`SLAB_QUEUE_CAPACITY`/`SLAB_BACKEND_CAPACITY` [:163-164](../../../crates/slab-config/src/app_config.rs#L163-L164)；`SLAB_ADMIN_TOKEN` [:170](../../../crates/slab-config/src/app_config.rs#L170)（审计 :156）；`SLAB_ENABLE_SWAGGER` [:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)（审计 :152-154）；`parse_env` [:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)（审计 :196-198）。 |
| `crates/slab-config/src/document.rs` | **`crates/slab-config/src/settings/document.rs`** | 目录迁移。`LoggingConfig` [:154-165](../../../crates/slab-config/src/settings/document.rs#L154-L165)；`LoggingOverrideConfig` [:173-185](../../../crates/slab-config/src/settings/document.rs#L173-L185)（审计 :174-185 接近）；`writeOnly:true` [:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405)；mcp env 说明 [:1476-1479](../../../crates/slab-config/src/settings/document.rs#L1476-L1479)。 |
| `bin/slab-server/src/api/v1/middleware/auth.rs` | **`bin/slab-server/src/api/middleware/auth.rs`**（无 `v1`） | 路径修正。`admin_api_token` 读取 [:23](../../../bin/slab-server/src/api/middleware/auth.rs#L23)（审计 :23 ✓）；`bind_address` 读取 [:24](../../../bin/slab-server/src/api/middleware/auth.rs#L24)；loopback 调用 [:44](../../../bin/slab-server/src/api/middleware/auth.rs#L44)；`is_loopback_bind_address` [:61-73](../../../bin/slab-server/src/api/middleware/auth.rs#L61-L73)（审计 :61-73 ✓）。 |
| `affects_agent_runtime` `settings.rs:73-75` | **`settings.rs:46-48`** | 行漂移。逻辑不变（仍只匹配 `agent.hooks.*`/`agent.memories.*`）。 |
| `pmid_service.rs:167-169` (runtime_log_dir) | **[:180-181](../../../crates/slab-config/src/pmid_service.rs#L180-L181)** | 行漂移。 |
| `pmid_service.rs:821` (metrics_exporter Object) | **[:852](../../../crates/slab-config/src/pmid_service.rs#L852)** | 行漂移。 |
| `pmid_service.rs:849-852` (value_type) | **[:844-894](../../../crates/slab-config/src/pmid_service.rs#L844-L894)**，Integer/Number→Integer 在 [:882](../../../crates/slab-config/src/pmid_service.rs#L882)/[:887](../../../crates/slab-config/src/pmid_service.rs#L887) | 行漂移。 |
| `pmid_service.rs:893-907` (minimum) | **`minimum_value` [:924-938](../../../crates/slab-config/src/pmid_service.rs#L924-L938)** | 行漂移。逻辑不变。 |
| `pmid_service.rs:909-928` (json_schema) | **`json_schema()` [:940-959](../../../crates/slab-config/src/pmid_service.rs#L940-L959)** | 行漂移。`metrics_exporter` 仍返 `None`。 |
| `pmid_service.rs:1868-1871` (trio 不对称测试) | **[:2150-2153](../../../crates/slab-config/src/pmid_service.rs#L2150-L2153)** | 行漂移。`slab_home`/`exporter`/`trace_exporter` 不回显的断言仍成立。 |
| `pmid_service.rs:1714` (secret 测试) | **[:1714](../../../crates/slab-config/src/pmid_service.rs#L1714)** ✓ | 无漂移。 |
| `descriptor.rs:434-436`/`:438-447` | ✓ 一致 | 无漂移。 |
| `view.rs:42-51`/`:68-70`/`:84-91` | **`:42-58`/`:68-70`/`:82-91`** | 微漂移。 |

**审计 §1.3 纠错记录确认无误**：PMID `secret()`/`redact_setting_value` 脱敏**实现正确**，本规范直接建立其上，仅升级驱动方式。

---

## 2. 架构设计原则

> 审计 §1.1 第三短板即"env/PMID 双源陷阱"。本规范的五项原则逐一对应 G1–G5。

### 2.1 单源真源（env 种子，PMID 为准）

**P1. env 仅作首次启动种子（seed-once），运行期生效值以 PMID 为准。** 现状的 `Config::from_env()`（[app_config.rs:142-199](../../../crates/slab-config/src/app_config.rs#L142-L199)）每进程启动都从 env 读，与 settings.json 形成两条独立写入路径，**优先级未定义**。本规范确立单向桥：env 在**首次** settings.json 不存在时种子化，此后 env 仅作"启动期覆盖信号"，且**覆盖关系必须在回显中可见**（见 §3.1 的 `source` 字段）。

**P2. 回显必须诚实反映生效值。** 回显的 `effective_value` 不得因序列化失败变 `Null`（PMID-F5）、不得因 u64 溢出变 f64（PMID-F6）、不得把 tagged union 洗成不透明 Object（PMID-F3）、不得把 f64 标 Integer（PMID-F10）。回显失败本身是 bug，必须传播。

### 2.2 声明即强制

**P3. schema 声明的约束在写路径或回显路径必须被消费。** `minimum`/`maximum` 声明在 [pmid_service.rs:924-938](../../../crates/slab-config/src/pmid_service.rs#L924-L938) 却不强制（PMID-F8）；`writeOnly` 声明在 [document.rs:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405) 却不驱动脱敏（PMID-F9）。**声明而不强制 = 谎报契约**。本规范把两者都接线到消费点。

### 2.3 静默降级不可接受

**P4. 数据/配置路径的失败必须可观测。** `parse_env` 畸形 → 默认无 warn（PMID-F7）；`setting_value_from` 序列化失败 → `Null`（PMID-F5）；u64 溢出 → f64（PMID-F6）。这些都是"坏数据静默存活"的形态，与审计 §4 的 DB 侧静默强转同源。**至少 `warn!`，最好报错**。

### 2.4 热重载边界必须可观测

**P5. 改了不生效的设置，UI 必须告诉用户。** `affects_agent_runtime`（[settings.rs:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)）只匹配 agent 两个前缀，其余设置改了不 reload 且无信号（F-Stack-1）。本规范不盲目扩宽热重载（runtime/inference 改了真重载成本高、风险大），而是引入**needs-restart 信号**回显到设置视图，让用户知道"这个改动要重启/手动重载模型才生效"。

---

## 3. 核心设计

### 3.1 env→PMID 单向桥 + 设置页"被 env 覆盖"标注（PMID-F1 / F-Stack-2 token 注入路径）

**根因**：鉴权 `auth_middleware`（[auth.rs:22-26](../../../bin/slab-server/src/api/middleware/auth.rs#L22-L26)）读 `state.context.config.admin_api_token`，该值来自 `SLAB_ADMIN_TOKEN` env（[app_config.rs:170](../../../crates/slab-config/src/app_config.rs#L170)）；`server.admin.token` PMID（[descriptor.rs:402](../../../crates/slab-config/src/descriptor.rs#L402)）写 settings.json，与鉴权不相通。bind/log/queue 同理。哪个生效取决于消费者走哪条代码路径，**优先级未定义**。

**修复（两轨并行，二者择一由部署模式决定，但都要文档化）**：

**轨 A — 单向桥（推荐，桌面默认）**：env 仅作**首次启动种子**。`Config::from_env()` 改为：若 settings.json 中对应字段为缺省值且 env 存在，则把 env 值种子化写入 settings.json（一次性），此后进程启动读 settings.json，env **不再覆盖**（除 `SLAB_SETTINGS_PATH`/`SLAB_DATABASE_URL` 这类纯启动锚点，保持 env 优先）。映射表（env → PMID）：

| env | PMID | 当前生效源 | 单向桥后 |
|-----|------|-----------|---------|
| `SLAB_ADMIN_TOKEN` | `server.admin.token` ([descriptor.rs:402](../../../crates/slab-config/src/descriptor.rs#L402)) | env（[app_config.rs:170](../../../crates/slab-config/src/app_config.rs#L170) → [auth.rs:23](../../../bin/slab-server/src/api/middleware/auth.rs#L23)） | PMID 为准；env 仅种子 |
| `SLAB_BIND` | `server.address` ([descriptor.rs:395](../../../crates/slab-config/src/descriptor.rs#L395)) | env（[app_config.rs:157](../../../crates/slab-config/src/app_config.rs#L157)） | PMID 为准 |
| `SLAB_LOG` | `logging.level` ([descriptor.rs:26](../../../crates/slab-config/src/descriptor.rs#L26)) | env（[app_config.rs:159](../../../crates/slab-config/src/app_config.rs#L159)） | PMID 为准 |
| `SLAB_LOG_JSON` | `logging.json` ([descriptor.rs:27](../../../crates/slab-config/src/descriptor.rs#L27)) | env（[app_config.rs:160](../../../crates/slab-config/src/app_config.rs#L160)） | PMID 为准 |
| `SLAB_LOG_FILE` | `logging.path` ([descriptor.rs:28](../../../crates/slab-config/src/descriptor.rs#L28)) | env（[app_config.rs:161](../../../crates/slab-config/src/app_config.rs#L161)） | PMID 为准 |
| `SLAB_QUEUE_CAPACITY` | `runtime.capacity.queue` ([descriptor.rs:120-122](../../../crates/slab-config/src/descriptor.rs#L120-L122)) | env（[app_config.rs:163](../../../crates/slab-config/src/app_config.rs#L163)） | PMID 为准 |
| `SLAB_BACKEND_CAPACITY` | `runtime.capacity.concurrent_requests` ([descriptor.rs:123-126](../../../crates/slab-config/src/descriptor.rs#L123-L126)) | env（[app_config.rs:164](../../../crates/slab-config/src/app_config.rs#L164)） | PMID 为准 |
| `SLAB_ENABLE_SWAGGER` | `server.swagger.enabled` ([descriptor.rs:403-405](../../../crates/slab-config/src/descriptor.rs#L403-L405)) | env（[app_config.rs:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)） | PMID 为准 |
| `SLAB_CLOUD_HTTP_TRACE` | `server.cloud_http_trace` ([descriptor.rs:406-408](../../../crates/slab-config/src/descriptor.rs#L406-L408)) | env（[app_config.rs:162](../../../crates/slab-config/src/app_config.rs#L162)） | PMID 为准 |
| `SLAB_TRANSPORT` | `runtime.transport` ([descriptor.rs:113](../../../crates/slab-config/src/descriptor.rs#L113)) | env（[app_config.rs:171](../../../crates/slab-config/src/app_config.rs#L171)） | PMID 为准 |

**鉴权消费点改写**：`auth_middleware`（[auth.rs:22-26](../../../bin/slab-server/src/api/middleware/auth.rs#L22-L26)）改读 `state.context.pmid().config().server.admin.token`（经 [PmidService::config()](../../../crates/slab-config/src/pmid_service.rs#L55-L57) 实时读 settings.json 投影），而非 `state.context.config.admin_api_token`（启动期 env 快照）。这样用户在设置页改 `server.admin.token` → 下次请求鉴权立即反映。`PmidService` 已有 `spawn_periodic_refresh`（[pmid_service.rs:59-71](../../../crates/slab-config/src/pmid_service.rs#L59-L71)）做磁盘刷新，鉴权读 `config()` 拿到的总是最新投影。

**轨 B — 显式文档化边界（服务器/无头部署）**：保留 env 优先，但**回显必须标注**。`SettingPropertyView` 增字段：

```rust
// view.rs — SettingPropertyView 扩展（PMID-F1）
pub struct SettingPropertyView {
    // ...既有字段...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<OverrideSource>,   // 新：标注"此字段被 env 覆盖"
}

pub enum OverrideSource {
    Env { var_name: String, var_value_present: bool },  // 不回显 env 值（可能敏感）
    Parent { pmid: String },                             // logging override 级联（见 §3.2）
}
```

当 `server.admin.token` PMID 被 `SLAB_ADMIN_TOKEN` env 覆盖（轨 B 模式），回显携带 `overridden_by: Env { var_name: "SLAB_ADMIN_TOKEN", var_value_present: true }`。设置页据此显示徽标"由环境变量 SLAB_ADMIN_TOKEN 覆盖，编辑此处不生效"。

**F-Stack-2 token 注入路径（前端补救，两轨都需要）**：当前 [index.ts:29-37](../../../packages/api/src/index.ts#L29-L37) 无 Authorization 拦截器。引入一个**可选** token provider（不硬编码 token，由设置页/启动配置注入）：

```typescript
// packages/api/src/index.ts — 可选 Authorization 拦截器
export type SlabApiClientOptions = {
  baseUrl?: string | null;
  fetch?: ...;
  useErrorMiddleware?: boolean;
  getAdminToken?: () => string | null;   // 新：调用方注入，API 层不持有 token
};

export function createSlabApiFetchClient(options: SlabApiClientOptions = {}) {
  const client = createFetchClient<paths>(buildClientConfig(options));
  if (options.getAdminToken) {
    client.use(({ request }) => {
      const token = options.getAdminToken!();
      if (token) request.headers.set("Authorization", `Bearer ${token}`);
      return request;
    });
  }
  if (options.useErrorMiddleware) client.use(errorMiddleware);
  return client;
}
```

配套：401 响应时 `errorMiddleware`（[errors.ts](../../../packages/api/src/errors.ts)）携带**可操作诊断**——区分"loopback 模式下不该 401（疑似 bind 改了）"与"需要配置 admin token"。前端在设置页检测 `bind_address != loopback` 且无 token 时，主动提示"当前 bind 非回环，前端需要 admin token 才能访问 API"。

> **red-test（审计 §6.4 P1-1）**：集成测试断言"改 `server.admin.token` PMID 后，下次请求的鉴权行为改变"——**当前会 fail**（env 优先、PMID 写了不读），是驱动修复的红灯测试。轨 A 上线后该测试转绿。

### 3.2 logging 5 层级联解析器 + description_md 反映优先级（PMID-F2 / P1-3）

**根因**：5+ 并行 logging 组（根 `logging` [document.rs:154-165](../../../crates/slab-config/src/settings/document.rs#L154-L165)、`runtime.logging`、`runtime.ggml.logging`、`runtime.ggml.backends.{llama,whisper,diffusion}.logging`、`runtime.candle.logging`/`runtime.onnx.logging`/`server.logging`）。引擎仅解析 `runtime.logging.path.or_else(|| logging.path)`（[pmid_service.rs:180-181](../../../crates/slab-config/src/pmid_service.rs#L180-L181)）一个 fallback，对 `level`/`json` 无级联。优先级隐式无文档。

**修复（显式级联 + 回显反映）**：

引入 `resolve_effective_logging(pmid, document) -> EffectiveLogging` 解析器，定义**显式优先级链**（叶子胜出，缺省回退父级）：

```
对每个 backend B (llama/whisper/diffusion/candle/onnx)：
  effective.level = B.logging.level
                 ?? runtime.ggml.logging.level    (ggml backends only)
                 ?? runtime.logging.level
                 ?? logging.level                  (root)
  effective.json  = 同链
  effective.path  = 同链                           // 取代 :180-181 的单点 fallback
```

`runtime.logging`/`server.logging` 各自是 `runtime.*`/`server.*` 子树的中间层。`runtime.ggml.backends.llama.logging` 是 llama backend 的叶子层。

**回显反映优先级**（不破坏现有 `LoggingOverrideConfig` 结构，只改回显层）：每个 `runtime.ggml.backends.*.logging.level` 的 `description_md`（[view.rs:122](../../../crates/slab-config/src/view.rs#L122)）注明"覆盖 `runtime.ggml.logging.level` 与 `runtime.logging.level`；为空则继承父级"。同时回显 `override_value`（[view.rs:128-129](../../../crates/slab-config/src/view.rs#L128-L129)，字段已存在但当前未用于 logging）填入**实际从父级继承的值**，`is_overridden`（[:130](../../../crates/slab-config/src/view.rs#L130)）反映"本叶是否设置了覆盖"。

**§3.1 的 `overridden_by: Parent { pmid }` 复用于此**：叶子 logging 字段若未设，回显 `overridden_by: Parent { pmid: "runtime.logging.level" }`（或更上层），客户端可据此渲染继承链。

> **决策**：不**移除**任何 override 层（它们服务真实场景——per-backend 日志隔离），只补**解析器**与**文档**。移除层是破坏性改动，与"演进不重造"（[slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md) §1.3）精神一致。

### 3.3 telemetry enum: 发射 json_schema + 诚实 value_type + 三 exporter 对称化（PMID-F3 / F4 / P1-4）

**根因（F3）**：`OtelExporter` 是 `#[serde(tag="type")]` 联合（[config.rs:57-74](../../../crates/slab-otel/src/config.rs#L57-L74)）`{none, local_file{directory}, otlp_http{endpoint,headers,protocol,tls}}`，但回显 `value_type=Object`（[pmid_service.rs:852](../../../crates/slab-config/src/pmid_service.rs#L852)）、标 `multiline`、**无 `json_schema`**（`json_schema()` [pmid_service.rs:940-959](../../../crates/slab-config/src/pmid_service.rs#L940-L959) 对 `metrics_exporter` 返 `None`）。客户端看到自由 object，无 enum 提示。

**根因（F4）**：`OtelSettings`（[config.rs:95-127](../../../crates/slab-otel/src/config.rs#L95-L127)）暴露 `exporter`/`trace_exporter`/`metrics_exporter`（:112/:118/:120），descriptor catalog 只注册 `metrics_exporter`；测试（[pmid_service.rs:2150-2153](../../../crates/slab-config/src/pmid_service.rs#L2150-L2153)）断言 `slab_home`/`exporter`/`trace_exporter` 不回显。logs/traces exporter 运行期从 `slab_home` 注入（[config.rs:136-141](../../../crates/slab-otel/src/config.rs#L136-L141)），哪个 exporter 控制哪个信号 catalog 无文档。

**修复（F3）**：

1. `json_schema()` 增 arm：`"telemetry.metrics_exporter" => Some(otel_exporter_json_schema())`，其中 `otel_exporter_json_schema()` 由 `schemars::schema_for!(OtelExporter)` 生成（`OtelExporter` 已 derive `JsonSchema`，[config.rs:57](../../../crates/slab-otel/src/config.rs#L57)），发射 `oneOf`/discriminator 形态的 tagged-union schema。客户端据此渲染 enum 选择器 + 条件子字段。
2. `value_type` 增变体 `SettingValueType::TaggedUnion`（见 §3.8），`metrics_exporter` 映射为 `TaggedUnion` 而非 `Object`（更诚实）。`multiline` 移除（对 tagged union 语义错误）。

**修复（F4）—— 决策：三 exporter 对称暴露 + 文档化派生**：

经核实 `exporter`/`trace_exporter` 当前从 `slab_home` 派生（[config.rs:136-141](../../../crates/slab-config/src/config.rs#L136-L141) 的 `with_slab_home`），`slab_home` 是运行期注入（`#[schemars(skip)]`，[:105](../../../crates/slab-otel/src/config.rs#L105)）。两个选择：

| 方案 | 描述 | 取舍 |
|------|------|------|
| **(a) 三 exporter 全暴露** | `exporter`/`trace_exporter` 也进 descriptor catalog，标 `editable: true` | 用户完全控制，但 logs/traces exporter 当前依赖 `slab_home` 派生逻辑要拆除 |
| **(b) 文档化派生（采纳）** | 仅 `metrics_exporter` 可编辑；`exporter`/`trace_exporter` 在 `description_md` 注明"logs/traces exporter 自动派生自 `slab_home`，当前不可独立配置；如需独立控制请设 `slab_home`" | 最小改动，诚实反映现状 |

采纳 **(b)**：`metrics_exporter` 的 `description_md`（[pmid_service.rs:1186-1188](../../../crates/slab-config/src/pmid_service.rs#L1186-L1188)）增补："本字段控制 metrics 信号。logs/traces exporter 自动派生自运行期 `slab_home`（见 [config.rs:136-141](../../../crates/slab-otel/src/config.rs#L136-L141)），当前不可独立编辑。" 现状的不对称被**显式文档化**，消除"trio 看起来同族却只有一个可编辑"的困惑。

> 若未来要采纳 (a)，本规范的 `otel_exporter_json_schema()` 可直接复用——这是 (b)→(a) 的平滑升级路径。

### 3.4 错误传播（PMID-F5 / F6）

**F5 根因**：`setting_value_from`（[descriptor.rs:434-436](../../../crates/slab-config/src/descriptor.rs#L434-L436)）`serde_json::to_value(value).map(SettingValue::from).unwrap_or_default()` —— `Serialize` 失败（如 Windows 下非 UTF8 `PathBuf`）静默返 `Null`，客户端据此可能 `Unset`。

**F5 修复**：改为返回 `Result<SettingValue, ConfigError>`，失败时 `ConfigError::Internal(format!("failed to serialize setting {pmid}: {error}"))`。调用点（`descriptor!` 宏的 `get` 闭包）签名从 `fn(&SettingsDocument) -> SettingValue` 改为 `fn(&SettingsDocument) -> Result<SettingValue, ConfigError>`，`setting_value`（[descriptor.rs:413-421](../../../crates/slab-config/src/descriptor.rs#L413-L421)）传播。`build_document_view`（[pmid_service.rs:81-88](../../../crates/slab-config/src/pmid_service.rs#L81-L88)）对单个属性失败记录到 `warnings`（[view.rs:160](../../../crates/slab-config/src/view.rs#L160) 已有该字段），不砖整个文档。

**F6 根因**：`SettingValue::from(Value::Number)`（[view.rs:42-51](../../../crates/slab-config/src/view.rs#L42-L51)）：u64 > i64::MAX 时 `as_u64().and_then(i64::try_from)` 落空 → `as_f64().unwrap_or_default()` 静默整数→浮点精度损失（影响 [document.rs:918](../../../crates/slab-config/src/settings/document.rs#L918) 一带的 `models.auto_unload.min_free_*_memory_bytes` u64 字段）。对称地 [:68-70](../../../crates/slab-config/src/view.rs#L68-L70) `Number::from_f64(value).map_or(Value::Null, ...)` 把 NaN/±Inf 映成 `Null`。

**F6 修复**：
- `From<Value> for SettingValue`（[view.rs:37-60](../../../crates/slab-config/src/view.rs#L37-L60)）：u64 溢出 i64 时**不再降 f64**，而是新增变体 `SettingValueType::Unsigned`（或在 `Integer` 用 i128 承载——见决策）。采纳**新增 `SettingValue::Unsigned(u64)` 变体**（最诚实，u64 字段回显为 u64）。`value_type()`（[pmid_service.rs:844-894](../../../crates/slab-config/src/pmid_service.rs#L844-L894)）把 u64 路径的 PMID（`min_free_*_memory_bytes`）映为 `SettingValueType::Unsigned`（与 §3.8 的 `Float`/`TaggedUnion` 一并扩枚举）。
- NaN/Inf 入口拒绝：`From<SettingValue> for Value`（[view.rs:62-80](../../../crates/slab-config/src/view.rs#L62-L80)）的 `Number` arm，若 `value.is_nan() || value.is_infinite()` 返回 `Err`（需把 `From` 改 `TryFrom`）或在该路径上游用 `serde_json` 的 `arbitrary_precision` feature 拒绝。鉴于 NaN/Inf 本就不该来自正常反序列化（JSON spec 不含 NaN/Inf），入口拒绝是防线而非破坏正常路径。

### 3.5 parse_env 失败 warn + truthy/falsy 统一（PMID-F7 / G4 config-path 部分）

**根因**：`parse_env`（[app_config.rs:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)）`source.var(key).and_then(|v| v.parse().ok()).unwrap_or(default)` —— `SLAB_QUEUE_CAPACITY=abc` 静默变默认 64。`SLAB_ENABLE_SWAGGER`（[app_config.rs:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)）`v != "0" && !v.eq_ignore_ascii_case("false")` —— 除这两值外一切（含 `"no"`/`"off"`/`"disabled"`）视为 enabled，**语义反转**。

**修复**：
1. `parse_env` 解析失败时 `warn!(key, value = %v, "env var parse failed, falling back to default")`，对齐审计 G4"数据路径 `.ok()`/`unwrap_or_default()` 至少 `warn!`"。
2. truthy/falsy 统一到一个 `parse_bool_env(source, key, default) -> bool` helper，识别标准 truthy (`"1"`/`"true"`/`"yes"`/`"on"`，case-insensitive) 与 falsy (`"0"`/`"false"`/`"no"`/`"off"`/`"disabled"`)，**其余值 warn 并回退 default**（而非隐式 truthy）。`SLAB_ENABLE_SWAGGER`/`SLAB_LOG_JSON`/`SLAB_CLOUD_HTTP_TRACE` 全部用此 helper。`SLAB_ENABLE_SWAGGER=no` 正确禁用（修复语义反转）。

> 现有测试 [app_config.rs:469-485](../../../crates/slab-config/src/app_config.rs#L469-L485) `from_env_falls_back_for_invalid_capacity_and_unrecognized_boolean_values` 断言 `"yes"`/`"on"` → false（当前 `parse_trueish_env` 只认 `"1"`/`"true"`）。修复后 `"yes"`/`"on"` → **true**（更符合直觉），该测试需更新——这是行为修正，非回归。

### 3.6 write-path 消费 minimum/maximum（PMID-F8）

**根因**：`minimum_value`（[pmid_service.rs:924-938](../../../crates/slab-config/src/pmid_service.rs#L924-L938)）声明 `minimum=0`，但写路径 `set_setting_value`（[descriptor.rs:438-447](../../../crates/slab-config/src/descriptor.rs#L438-L447)）只做"能否反序列化"检查。`runtime.capacity.queue = -5` 因 u32 可反序列化为正数（实际 serde 会拒负 u32，但 `concurrent_requests` 等若类型放宽则漏）。

**修复**：`set_setting_value` 增约束校验。`descriptor!` 宏扩参携带 `minimum`/`maximum`（从 `minimum_value`/`maximum_value` 查表），写路径在反序列化后额外校验数值 ≥ minimum / ≤ maximum，违反返 `ConfigError::BadRequest(format!("setting '{pmid}' value {v} violates minimum {m}"))`。`SettingValidationErrorData`（[view.rs:178-185](../../../crates/slab-config/src/view.rs#L178-L185)）已在 schema 中，前端可结构化展示。

> 注意：`minimum`/`maximum` 当前是 `Option<i64>`（[view.rs:100-102](../../../crates/slab-config/src/view.rs#L100-L102)）。u64 字段（§3.4 引入 `Unsigned`）需扩 `Option<u64>` 或泛型化。一并处理。

### 3.7 writeOnly 驱动脱敏单源（PMID-F9 / G5 / P2-3）

**根因**：`secret()`（[pmid_service.rs:971-975](../../../crates/slab-config/src/pmid_service.rs#L971-L975)）与 `redact_setting_value()`（[:977-983](../../../crates/slab-config/src/pmid_service.rs#L977-L983)）各维护一份硬编码路径列表（`server.admin.token`/`providers.registry`/`agent.tools.websearch.providers`）。schema 已标 `writeOnly:true`（[document.rs:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405)），测试 [pmid_service.rs:1682-1701](../../../crates/slab-config/src/pmid_service.rs#L1682-L1701) 验证，但回显层**不消费** `writeOnly`。两份硬编码需同步。`agent.tools.mcp.servers` 不在列表（其 `env` 是 env-var 名引用，[document.rs:1476-1479](../../../crates/slab-config/src/settings/document.rs#L1476-L1479)，当前安全）。

> **审计 §1.3 纠错记录前提**：脱敏**算法本身正确**（`redact_secret_leaf`/`redact_api_key_fields`/`restore_secret_placeholders` 经 `secret_setting_views_redact_literal_secret_values` 测试 [:1714](../../../crates/slab-config/src/pmid_service.rs#L1714) 钉死）。本规范**不重造算法**，只把"哪些路径脱敏"的**判定**从硬编码迁移到 schema `writeOnly`。

**修复（单源驱动）**：

1. **schema 标注单源**：每个含 secret 的 leaf 在对应 `json_schema()`（[pmid_service.rs:940-959](../../../crates/slab-config/src/pmid_service.rs#L940-L959)）发射的 schema 内标 `writeOnly: true`。当前 `writeOnly` 只出现在 `providers.registry`/websearch 的内嵌 auth `api_key`（[document.rs:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405)）。需补：
   - `server.admin.token`：当前 `secret()` 标记它（[pmid_service.rs:972](../../../crates/slab-config/src/pmid_service.rs#L972)），但它是简单 string leaf 无独立 `json_schema`。给它发射一个 `json_schema` 标 `writeOnly: true`，或扩 `SettingPropertySchema`（[view.rs:93-115](../../../crates/slab-config/src/view.rs#L93-L115)）直接用 `secret: bool`（字段已在 [:110](../../../crates/slab-config/src/view.rs#L110)）——**采纳后者**：`secret` 字段已存在，让 `secret()` 函数返回值继续填它，但 `secret()` 的判定从硬编码路径列表改为"schema 任一层 writeOnly=true"。
2. **判定函数改写**：引入 `is_secret_by_schema(pmid, schema) -> bool`，遍历该 PMID 的 `json_schema`（若有）任一 leaf 标 `writeOnly` 即返 true；对无 `json_schema` 的简单 leaf（如 `server.admin.token`），用 `SettingPropertySchema.secret`（[view.rs:110](../../../crates/slab-config/src/view.rs#L110)）显式标注。`secret()`（[pmid_service.rs:971](../../../crates/slab-config/src/pmid_service.rs#L971)）与 `redact_setting_value()`（[:977](../../../crates/slab-config/src/pmid_service.rs#L977)）改为**统一调用** `is_secret_by_schema`，消除两份硬编码。
3. **`agent.tools.mcp.servers` 评估纳入**：经核实其 `env` 是 env-var 名引用（[document.rs:1476-1479](../../../crates/slab-config/src/settings/document.rs#L1476-L1479)，"Secret values are resolved at launch and are not stored here"），`mcp_servers_json_schema()`（[document.rs:1420](../../../crates/slab-config/src/settings/document.rs#L1420) 一带）无 `writeOnly`。**结论：当前不纳入**（设计上不存明文），但在 schema 的 `command`/`args` 字段补注释"不得存明文 secret，用 env 引用"。若未来设计变更允许明文，需在这些字段加 `writeOnly`，届时 `is_secret_by_schema` 自动纳入——这正是 writeOnly 驱动的**前向兼容**价值。

**与 model_pack 计划的并轨（重要）**：[slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md) Phase 3 增量扩展 `secret()` 白名单纳入 `download.<handle>` 一个新子树（该计划 §5.4）。**最终应被本规范的 writeOnly 驱动机制吸收**：model_pack 计划应在 `download.<handle>` 的 schema 标 `writeOnly: true`（而非加硬编码路径），本规范的 `is_secret_by_schema` 自动覆盖。建议执行序：本规范 Phase 2（writeOnly 驱动）先落地，model_pack 计划 Phase 3 改为"在 download schema 标 writeOnly"而非"扩硬编码白名单"。

### 3.8 Float value_type 变体（PMID-F10）

**根因**：`SettingValueType`（[view.rs:82-91](../../../crates/slab-config/src/view.rs#L82-L91)）无 `Float`。`value_type`（[pmid_service.rs:880-893](../../../crates/slab-config/src/pmid_service.rs#L880-L893)）把 `Integer` 与 `Number` 都映 `Integer`（:882/:887）。任何 f64 PMID（如未来的 `temperature`/`top_p` 类采样参数若上提到 PMID）被误标。

**修复**：`SettingValueType` 增 `Float`/`Unsigned`/`TaggedUnion`（一并支持 §3.3/§3.4）：

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueType {
    Boolean,
    Integer,      // i64
    Unsigned,     // u64 (§3.4 F6)
    Float,        // f64 (本节 F10)
    #[default]
    String,
    Array,
    Object,
    TaggedUnion,  // §3.3 F3 (telemetry.metrics_exporter)
}
```

`value_type`（[pmid_service.rs:844-894](../../../crates/slab-config/src/pmid_service.rs#L844-L894)）的 `match effective` 臂（:880-893）改：`SettingValue::Integer(_) => Integer`、`SettingValue::Unsigned(_) => Unsigned`、`SettingValue::Number(_) => Float`。当前 f64 PMID 暂无（采样参数在 inference payload 非 PMID），但扩枚举是**前向兼容**——未来上提采样参数到 PMID 时正确分类。

### 3.9 pmid.rs shim 决策（PMID-F11）

**现状**：[pmid.rs:1](../../../crates/slab-app-core/src/domain/services/pmid.rs) 全文 `pub use slab_config::PmidService;`。非死代码（re-export 进 app-core 命名空间），但语义空。`SettingsService`（[settings.rs:6-44](../../../crates/slab-app-core/src/domain/services/settings.rs#L6-L44)）包一层加 agent-runtime reload 副作用。

**决策（采纳：保留并赋予职责）**：不删 shim，而是把 §3.1 轨 A 的"鉴权读 PMID 实时投影"、§3.6 的 needs-restart 信号分发（§4）等 app-core 特定逻辑挂到 `PmidService` 的 domain 包装。具体：把 `SettingsService::update_setting`（[settings.rs:31-43](../../../crates/slab-app-core/src/domain/services/settings.rs#L31-L43)）的 `affects_agent_runtime` 判定升级为 §4 的 `effective_change_kind(pmid) -> ChangeKind { LiveAgent, NeedsRestart { subsystem }, NeedsModelReload }` 分类，分发到不同 reload 策略。这让 `pmid.rs` re-export 升级为"app-core 视角的设置变更语义层"。

> 删 shim 直接 import 也合法，但保留 shim 让 app-core 有地方挂 domain 行为（reload 策略、needs-restart 分发），与 [slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md) §4.7 的"网关拥有引擎存活、supervisor 拥有进程存活"职责分层精神一致。

### 3.10 热重载扩宽 + needs-restart 信号（F-Stack-1 / P1-2）

见 §4 的完整设置变更生效模型。本节只给入口：`SettingsService::update_setting`（[settings.rs:31-43](../../../crates/slab-app-core/src/domain/services/settings.rs#L31-L43)）当前对 `affects_agent_runtime` 触发 agent reload，其余静默。修复后每个 PMID 的 `SettingPropertyView` 携带 `change_effect: ChangeEffect`（新字段），取值 `Live`/`NeedsRestart`/`NeedsModelReload`/`None`，前端据此渲染徽标。

---

## 4. 设置变更生效模型（F-Stack-1 的核心）

> 这是 F-Stack-1 的根治设计。当前 `affects_agent_runtime`（[settings.rs:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)）只匹配 agent 两个前缀，其余改了不生效且无信号。本节定义"哪个 PMID 前缀影响哪个运行期子系统，live vs needs-restart，UI 如何呈现"。

### 4.1 生效分类矩阵

| PMID 前缀 | 影响子系统 | 生效方式 | UI 信号 |
|----------|-----------|---------|---------|
| `agent.hooks.*` | AgentRuntime（hooks 脚本） | **Live**（`agent_runtime.reload()`，已实现 [settings.rs:37-41](../../../crates/slab-app-core/src/domain/services/settings.rs#L37-L41)） | 无（即时） |
| `agent.memories.*` | AgentRuntime（记忆管线） | **Live**（同上） | 无 |
| `agent.debug` | AgentRuntime（调试日志） | **Live** | 无 |
| `agent.tools.mcp.*` | AgentRuntime（MCP 工具） | **Live**（mcp server 配置下次工具调用生效） | 无 |
| `agent.tools.websearch.*` | AgentRuntime（websearch 工具） | **Live** | 无 |
| `runtime.capacity.*` | slab-runtime（队列/并发） | **NeedsRestart**（容量在进程启动时固定） | "需重启 slab-runtime 生效" |
| `runtime.mode`/`runtime.transport` | slab-runtime 进程模型 | **NeedsRestart** | "需重启 slab-runtime 生效" |
| `runtime.sessions.state_dir` | slab-runtime 会话状态 | **NeedsRestart** | 同上 |
| `runtime.ggml.backends.*.context_length` | 推理 backend（KV cache） | **NeedsModelReload**（下次 `POST /v1/models/load` 生效） | "需重新加载模型生效" |
| `runtime.ggml.backends.*.flash_attn` | 推理 backend | **NeedsModelReload** | 同上 |
| `runtime.ggml.backends.*.enabled` | slab-runtime backend 注册 | **NeedsRestart** | "需重启 slab-runtime 生效" |
| `runtime.ggml.backends.*.source.*` | 二进制下载 | **NeedsRestart**（下载后重启） | "需下载并重启" |
| `runtime.ggml.backends.*.endpoint.*` | GrpcGateway 路由 | **Live**（gateway 重新解析 endpoint） | 无 |
| `runtime.ggml.install_dir` | 二进制加载路径 | **NeedsRestart** | 同上 |
| `runtime.logging.*` / `logging.*` | tracing subscriber | **NeedsRestart**（filter 在启动时初始化） | "需重启生效" |
| `runtime.candle.*` / `runtime.onnx.*` | 同 ggml.backends | 同上 | 同上 |
| `models.cache_dir` / `models.config_dir` | 模型存储路径 | **NeedsRestart** | 同上 |
| `models.auto_unload.*` | 卸载策略 | **Live**（下次压力检查生效） | 无 |
| `models.download_source` | 下载默认源 | **Live**（下次下载生效） | 无 |
| `chat.providers.*` / `providers.registry` | 云端路由 | **Live**（下次 cloud chat 生效） | 无 |
| `plugin.*` | 插件加载 | **NeedsRestart**（插件在启动期扫描） | "需重启生效" |
| `server.address` | HTTP bind | **NeedsRestart**（bind 在启动时固定） | "需重启 slab-server 生效" |
| `server.admin.token` | 鉴权（§3.1 轨 A 后） | **Live**（[auth.rs:23](../../../bin/slab-server/src/api/middleware/auth.rs#L23) 读实时投影） | 无 |
| `server.logging.*` | slab-server 日志 | **NeedsRestart** | 同上 |
| `server.cors.allowed_origins` | CORS 中间件 | **NeedsRestart** | 同上 |
| `server.swagger.enabled` | Swagger 路由 | **NeedsRestart**（路由在启动期注册） | 同上 |
| `server.cloud_http_trace` | 云端 HTTP trace | **Live**（下次请求读 flag） | 无 |
| `telemetry.*` | OTel 初始化 | **NeedsRestart**（OTel pipeline 在启动期建） | "需重启生效" |
| `tools.ffmpeg.*` | FFmpeg sidecar | **NeedsRestart**（sidecar 在启动期 spawn） | 同上 |
| `general.language` | i18n | **Live**（前端下次渲染） | 无 |
| `database.url` | SQLite 连接 | **NeedsRestart** | 同上 |

### 4.2 ChangeEffect 枚举与回显

```rust
// view.rs — SettingPropertyView 扩展（F-Stack-1）
pub struct SettingPropertyView {
    // ...既有字段...
    #[serde(default)]
    pub change_effect: ChangeEffect,           // 新：生效方式
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overridden_by: Option<OverrideSource>, // §3.1/§3.2 复用
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeEffect {
    Live,             // 写入即生效（agent/runtime 子系统监听）
    NeedsRestart,     // 需重启进程（slab-server / slab-runtime）
    #[default]
    NeedsModelReload, // 需重新加载模型（context_length/flash_attn）
    None,             // 仅持久化，无运行期消费者（罕见）
}
```

`change_effect_for(pmid) -> ChangeEffect` 函数（挂到 [pmid.rs](../../../crates/slab-app-core/src/domain/services/pmid.rs) 的 domain 包装，§3.9）实现 §4.1 矩阵的查找表。`SettingsService::update_setting`（[settings.rs:31-43](../../../crates/slab-app-core/src/domain/services/settings.rs#L31-L43)）根据 `ChangeEffect` 分发：

```rust
match change_effect_for(pmid) {
    ChangeEffect::Live if affects_agent_runtime(pmid) => { agent_runtime.reload().await?; }
    ChangeEffect::Live => { /* cloud/tools/general 子系统自行监听 periodic_refresh */ }
    ChangeEffect::NeedsRestart | ChangeEffect::NeedsModelReload | ChangeEffect::None => {
        /* 仅持久化；回显的 change_effect 字段告知前端 */
    }
}
```

### 4.3 前端呈现

设置页（[packages/slab-desktop](../../../packages/slab-desktop) 的 settings page）对每个属性读 `change_effect`：
- `Live` → 无徽标（或淡色"即时生效"）。
- `NeedsRestart` → 橙色徽标"⚠ 需重启生效"，并在页面顶部聚合一个"有 N 项设置待重启"提示条。
- `NeedsModelReload` → 蓝色徽标"↻ 需重新加载模型"，旁边给"重载模型"快捷按钮（调 `POST /v1/models/load`）。
- `None` → 灰色"仅持久化"。

> **不盲目扩宽热重载**：runtime/inference 改了真重载成本高（KV cache 重分配、backend 重新注册），且 slab-runtime 是独立进程，跨进程 live reload 需新 gRPC 信号。本设计选择**诚实标注 + 用户驱动重启**，而非假装 live reload。这比当前"改了静默不生效"严格更优。

---

## 5. 实施路线图

> 每阶段：{涉及文件（可点击相对链接）、关闭的审计发现、校验命令（[AGENTS.md](../../../AGENTS.md) §56-95）、退出标准}。迁移是**加性**的；现有回显契约的扩展字段全部 `#[serde(default)]`，前端旧版本不破。

### Phase 0 — 回显 schema 基元扩展（枚举扩容 + OverrideSource/ChangeEffect）

- **涉及文件**：
  - [view.rs](../../../crates/slab-config/src/view.rs)：`SettingValueType` 增 `Unsigned`/`Float`/`TaggedUnion`（§3.8）；`SettingValue` 增 `Unsigned(u64)` 变体（§3.4 F6）；`SettingPropertyView` 增 `change_effect: ChangeEffect` 与 `overridden_by: Option<OverrideSource>` 字段；新枚举 `ChangeEffect`/`OverrideSource`（§4.2/§3.1）。所有新字段 `#[serde(default)]`。
  - [document.rs](../../../crates/slab-config/src/settings/document.rs)：无结构性改动（`writeOnly` 标注已存在 [:1171](../../../crates/slab-config/src/settings/document.rs#L1171)/[:1405](../../../crates/slab-config/src/settings/document.rs#L1405)）。
- **关闭**：无（基础设施）。
- **校验**：`bun run check:rust`（最窄，[AGENTS.md:69](../../../AGENTS.md)）→ `bun run gen:api` 刷 [v1.d.ts](../../../packages/api/src/v1.d.ts)（新字段进 OpenAPI）。
- **退出标准**：新枚举/字段编译通过；既有 `secret_setting_views_redact_literal_secret_values`（[pmid_service.rs:1714](../../../crates/slab-config/src/pmid_service.rs#L1714)）等测试不破（新字段 default）。

### Phase 1 — 回显诚实化（PMID-F3 / F4 / F5 / F6 / F10）

- **涉及文件**：
  - [descriptor.rs](../../../crates/slab-config/src/descriptor.rs)：`setting_value_from`（[:434-436](../../../crates/slab-config/src/descriptor.rs#L434-L436)）改返 `Result<SettingValue, ConfigError>`，失败 `ConfigError::Internal`（PMID-F5）；`descriptor!` 宏 `get` 闭包签名升级；`setting_value`（[:413-421](../../../crates/slab-config/src/descriptor.rs#L413-L421)）传播。
  - [view.rs](../../../crates/slab-config/src/view.rs)：`From<Value> for SettingValue`（[:37-60](../../../crates/slab-config/src/view.rs#L37-L60)）u64 溢出走 `Unsigned` 变体不再降 f64（PMID-F6）；`From<SettingValue> for Value`（[:62-80](../../../crates/slab-config/src/view.rs#L62-L80)）NaN/Inf 拒绝（改 `TryFrom` 或上游防线）。
  - [pmid_service.rs](../../../crates/slab-config/src/pmid_service.rs)：`value_type`（[:844-894](../../../crates/slab-config/src/pmid_service.rs#L844-L894)）分发到 `Integer`/`Unsigned`/`Float`/`TaggedUnion`（PMID-F10/F3）；`json_schema()`（[:940-959](../../../crates/slab-config/src/pmid_service.rs#L940-L959)）增 `"telemetry.metrics_exporter" => Some(otel_exporter_json_schema())` arm（PMID-F3）；`build_document_view`（[:81-88](../../../crates/slab-config/src/pmid_service.rs#L81-L88)）对单属性失败记 `warnings`（[view.rs:160](../../../crates/slab-config/src/view.rs#L160)）。
  - 新 helper `otel_exporter_json_schema()`：`schemars::schema_for!(OtelExporter)` 生成（[OtelExporter](../../../crates/slab-otel/src/config.rs#L57) 已 derive `JsonSchema`）。
  - [pmid_service.rs:1186-1188](../../../crates/slab-config/src/pmid_service.rs#L1186-L1188)：`metrics_exporter` 的 `description_md` 增"logs/traces exporter 派生自 slab_home"说明（PMID-F4 文档化）。
- **关闭**：**PMID-F3、PMID-F4、PMID-F5、PMID-F6、PMID-F10**。
- **校验**：`bun run test:rust:cargo`（[AGENTS.md:75](../../../AGENTS.md)）。新增/更新测试：
  - `metrics_exporter` 回显携带 `json_schema` 且 `value_type == TaggedUnion`（非 Object）。
  - `setting_value_from` 序列化失败传播 `ConfigError::Internal`（不再 Null）。
  - u64 > i64::MAX 的 `min_free_*_memory_bytes` 回显为 `Unsigned`（非 f64）。
  - NaN/Inf 入口拒绝。
  - 既有测试 [pmid_service.rs:2150-2153](../../../crates/slab-config/src/pmid_service.rs#L2150-L2153)（trio 不对称）保持绿（exporter/trace_exporter 仍不回显）。
- **退出标准**：5 个新/改测试通过；既有脱敏测试不破。

### Phase 2 — 脱敏 writeOnly 单源 + parse_env warn（PMID-F9 / G5 / P2-3 / PMID-F7 / G4 config-path）

- **涉及文件**：
  - [pmid_service.rs](../../../crates/slab-config/src/pmid_service.rs)：新 `is_secret_by_schema(pmid, schema) -> bool`，遍历 `json_schema` 任一 leaf `writeOnly=true` 或 `SettingPropertySchema.secret=true` 即 true；`secret()`（[:971-975](../../../crates/slab-config/src/pmid_service.rs#L971-L975)）与 `redact_setting_value()`（[:977-983](../../../crates/slab-config/src/pmid_service.rs#L977-L983)）统一调用 `is_secret_by_schema`，**删除两份硬编码路径列表**（PMID-F9）。
  - [view.rs](../../../crates/slab-config/src/view.rs)：`SettingPropertySchema.secret`（[:110](../../../crates/slab-config/src/view.rs#L110)）填值改为由 schema 驱动。
  - [app_config.rs](../../../crates/slab-config/src/app_config.rs)：`parse_env`（[:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)）失败 `warn!`（PMID-F7）；新 `parse_bool_env` helper 统一 truthy/falsy，`SLAB_ENABLE_SWAGGER`（[:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)）/`SLAB_LOG_JSON`（[:160](../../../crates/slab-config/src/app_config.rs#L160)）/`SLAB_CLOUD_HTTP_TRACE`（[:162](../../../crates/slab-config/src/app_config.rs#L162)）改用之（修复 `"no"` 语义反转）。
- **关闭**：**PMID-F9、PMID-F7、G5、G4 config-path 部分、P2-3**。
- **校验**：`bun run test:rust:cargo`。测试：
  - `secret_setting_views_redact_literal_secret_values`（[:1714](../../../crates/slab-config/src/pmid_service.rs#L1714)）在 writeOnly 驱动下仍通过（算法不变，判定源变）。
  - 新测试：给一个 PMID 的 `json_schema` 标 `writeOnly: true` → 自动脱敏（无需改硬编码）。
  - `parse_env` 失败 warn（用 `tracing_test` 捕获）。
  - `SLAB_ENABLE_SWAGGER=no` → false（修复反转）；更新 [app_config.rs:469-485](../../../crates/slab-config/src/app_config.rs#L469-L485) 测试期望（`"yes"`/`"on"` → true）。
- **退出标准**：硬编码路径列表删除；脱敏由 writeOnly 驱动；env bool 语义统一。

### Phase 3 — env→PMID 单向桥 + 鉴权实时投影 + 前端 token 注入（PMID-F1 / F-Stack-2）

- **涉及文件**：
  - [app_config.rs](../../../crates/slab-config/src/app_config.rs)：`Config::from_env_source`（[:146-199](../../../crates/slab-config/src/app_config.rs#L146-L199)）实现轨 A 单向桥（env 仅种子化首次 settings.json，此后 PMID 为准）；保留轨 B 的 `overridden_by` 标注能力（部署模式开关）。
  - [auth.rs](../../../bin/slab-server/src/api/middleware/auth.rs)：`auth_middleware`（[:22-26](../../../bin/slab-server/src/api/middleware/auth.rs#L22-L26)）改读 `state.context.pmid().config().server.admin.token` 实时投影（而非 `state.context.config.admin_api_token` 启动期快照）；`bind_address` 同理读实时投影（loopback 判定）。
  - [index.ts](../../../packages/api/src/index.ts)：`createSlabApiFetchClient`（[:29-37](../../../packages/api/src/index.ts#L29-L37)）增可选 `getAdminToken` 拦截器；`errorMiddleware` 增 401 可操作诊断（区分 loopback 误判 vs 缺 token）。
  - 前端设置页：检测 `bind_address != loopback && !token` 时主动提示。
- **关闭**：**PMID-F1、F-Stack-2**。
- **校验**：`bun run test:rust:cargo` + `bun run test:frontend`（[AGENTS.md:73](../../../AGENTS.md)）。**red-test（审计 §6.4 P1-1）**：集成测试断言"改 `server.admin.token` PMID → 下次请求鉴权行为改变"（轨 A 上线后转绿）。
- **退出标准**：设置页改 admin token 立即影响鉴权；前端在非 loopback 无 token 时给引导；red-test 通过。

### Phase 4 — logging 级联解析器 + write-path minimum 强制 + ChangeEffect 分发（PMID-F2 / F8 / F-Stack-1 / P1-2 / P1-3）

- **涉及文件**：
  - [pmid_service.rs](../../../crates/slab-config/src/pmid_service.rs)：新 `resolve_effective_logging(pmid, document)` 解析器（§3.2 显式优先级链）；logging 叶子字段的 `override_value`（[view.rs:128-129](../../../crates/slab-config/src/view.rs#L128-L129)）填父级继承值，`description_md` 注明覆盖链；`overridden_by: Parent { pmid }` 反映继承。
  - [descriptor.rs](../../../crates/slab-config/src/descriptor.rs)：`set_setting_value`（[:438-447](../../../crates/slab-config/src/descriptor.rs#L438-L447)）增 minimum/maximum 校验（PMID-F8）；`descriptor!` 宏携约束。
  - [settings.rs](../../../crates/slab-app-core/src/domain/services/settings.rs)：`affects_agent_runtime`（[:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)）升级为 `change_effect_for(pmid) -> ChangeEffect`（§4.1 矩阵）；`update_setting`（[:31-43](../../../crates/slab-app-core/src/domain/services/settings.rs#L31-L43)）按 `ChangeEffect` 分发。
  - [pmid.rs](../../../crates/slab-app-core/src/domain/services/pmid.rs)：从 1 行 re-export（§3.9）升级为 domain 包装，挂 `change_effect_for` 与 needs-restart 分发。
  - 前端设置页：读 `change_effect` 渲染徽标（Live/NeedsRestart/NeedsModelReload）+ 顶部"待重启 N 项"提示条。
- **关闭**：**PMID-F2、PMID-F8、F-Stack-1、P1-2、P1-3**。
- **校验**：`bun run test:rust:cargo` + `bun run test:frontend`。测试：
  - `runtime.ggml.backends.llama.logging.level` 未设时 `override_value` 反映 `runtime.logging.level` 继承值。
  - `runtime.capacity.queue = -5` 写入被拒（`ConfigError::BadRequest`，PMID-F8）。
  - 改 `agent.hooks.*` 触发 reload（既有行为保持）；改 `runtime.capacity.*` 不触发 reload 但回显 `change_effect: NeedsRestart`。
- **退出标准**：logging 5 层级联可观测；minimum 写路径强制；设置页显示生效方式徽标。

### Phase 5 — pmid.rs shim 收尾 + 文档 + 复审（PMID-F11）

- **涉及文件**：
  - [pmid.rs](../../../crates/slab-app-core/src/domain/services/pmid.rs)：Phase 4 已挂 domain 逻辑，本阶段补 doc 注释说明"app-core 视角的设置变更语义层"职责（§3.9）。
  - 复审：基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) 出一份新审计文档，确认 PMID-F1–F11 / F-Stack-1 / F-Stack-2 / G4-config / G5 关闭且回显路径无新发现。
- **关闭**：**PMID-F11**（sham 决策落地）+ 全部 P1-1/P1-2/P1-3/P1-4/P2-3。
- **校验**：全量 `bun run check` + `bun run test`（[AGENTS.md:68](../../../AGENTS.md)/[:72](../../../AGENTS.md)）。
- **退出标准**：复审显示所有 owned finding 关闭；新审计无回归。

### 5.1 验证策略（对齐审计 §6.4）

- **Phase 1**：回显诚实化的 5 个测试（F3/F4/F5/F6/F10）。
- **Phase 2**：writeOnly 驱动的脱敏测试 + parse_env warn 测试（用 `tracing_test` 捕获日志）。
- **Phase 3**：**red-test**（审计 §6.4 P1-1）——改 `server.admin.token` PMID 后鉴权行为改变，当前 fail 驱动修复。前端 401 诊断测试。
- **Phase 4**：logging 级联继承值回显测试 + minimum 写拒绝测试 + ChangeEffect 分发测试。
- **整体**：每 Phase 用最窄校验命令（[AGENTS.md:18](../../../AGENTS.md)）先验证，再扩到 workspace。

---

## 6. 验证策略

1. **对抗式回显测试**：对每个回显字段，构造"应当报错/降级的输入"，断言回显**不**静默吞错：
   - u64 溢出 → `Unsigned`（非 f64）。
   - 序列化失败 → `ConfigError::Internal`（非 Null）。
   - tagged union → `json_schema` 存在 + `TaggedUnion`（非裸 Object）。
2. **red-test 驱动（审计 §6.4 P1-1）**：在 Phase 3 实现**前**先写"改 admin token PMID → 鉴权改变"测试，确认当前 fail，再实现使其转绿。
3. **脱敏单源验证**：Phase 2 删除硬编码列表后，所有原脱敏路径（`server.admin.token`/`providers.registry`/websearch）仍由 writeOnly 驱动脱敏；新加 `writeOnly` 字段自动脱敏（前向兼容测试）。
4. **生效模型 e2e**：Phase 4 前端设置页 e2e 测试——改 `runtime.capacity.queue`，UI 显示"需重启"徽标，agent reload 不被错误触发。
5. **不破既有契约**：所有新字段 `#[serde(default)]`，旧前端版本忽略不破；既有 `secret_setting_views_redact_literal_secret_values`（[pmid_service.rs:1714](../../../crates/slab-config/src/pmid_service.rs#L1714)）在 writeOnly 驱动下仍通过。

---

## 附录 A：审计发现 → 计划条款 闭环追溯

| 审计发现（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md)） | 本计划机制 | 实施阶段 |
|---|---|---|
| **PMID-F1** env vs PMID 双源断开（[app_config.rs:170](../../../crates/slab-config/src/app_config.rs#L170) vs [descriptor.rs:402](../../../crates/slab-config/src/descriptor.rs#L402)） | env→PMID 单向桥（轨 A 种子化）+ `overridden_by` 标注（轨 B）+ 鉴权读实时投影 | Phase 3 |
| **PMID-F2** 5 层 logging override 无级联（[pmid_service.rs:180-181](../../../crates/slab-config/src/pmid_service.rs#L180-L181) 仅 path fallback） | `resolve_effective_logging` 显式优先级链 + `override_value`/`overridden_by: Parent` 回显 + `description_md` 文档 | Phase 4 |
| **PMID-F3** metrics_exporter 类型 laundering（[pmid_service.rs:852](../../../crates/slab-config/src/pmid_service.rs#L852) Object/无 json_schema） | `otel_exporter_json_schema()` 发射 + `TaggedUnion` value_type | Phase 1 |
| **PMID-F4** 三 exporter 不对称（descriptor 仅 metrics_exporter） | 文档化 logs/traces 派生自 slab_home（[config.rs:136-141](../../../crates/slab-otel/src/config.rs#L136-L141)）；`description_md` 注明 | Phase 1 |
| **PMID-F5** setting_value_from 吞序列化错误（[descriptor.rs:434-436](../../../crates/slab-config/src/descriptor.rs#L434-L436)） | 返 `Result<_, ConfigError::Internal>`，`build_document_view` 记 `warnings` | Phase 1 |
| **PMID-F6** u64 溢出降 f64/NaN→Null（[view.rs:42-51](../../../crates/slab-config/src/view.rs#L42-L51)/[:68-70](../../../crates/slab-config/src/view.rs#L68-L70)） | `SettingValue::Unsigned(u64)` 变体 + NaN/Inf 入口拒绝 | Phase 1 |
| **PMID-F7** parse_env 畸形静默默认 + SWAGGER 语义反转（[app_config.rs:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)/[:165-168](../../../crates/slab-config/src/app_config.rs#L165-L168)） | `parse_env` 失败 warn + `parse_bool_env` 统一 truthy/falsy | Phase 2 |
| **PMID-F8** minimum 声明不强制（[pmid_service.rs:924-938](../../../crates/slab-config/src/pmid_service.rs#L924-L938) 声明，[descriptor.rs:438-447](../../../crates/slab-config/src/descriptor.rs#L438-L447) 不校验） | `set_setting_value` 增 minimum/maximum 校验 → `ConfigError::BadRequest` | Phase 4 |
| **PMID-F9** 脱敏白名单硬编码两处（[pmid_service.rs:971-975](../../../crates/slab-config/src/pmid_service.rs#L971-L975)/[:977-983](../../../crates/slab-config/src/pmid_service.rs#L977-L983)） | `is_secret_by_schema` 统一驱动（消费 `writeOnly`/`secret`），删两份硬编码 | Phase 2 |
| **PMID-F10** 数值 value_type 无 Float（[pmid_service.rs:882](../../../crates/slab-config/src/pmid_service.rs#L882)/[view.rs:82-91](../../../crates/slab-config/src/view.rs#L82-L91)） | `SettingValueType` 增 `Float`/`Unsigned`/`TaggedUnion` | Phase 1 |
| **PMID-F11** pmid.rs 1 行 re-export 空抽象（[pmid.rs:1](../../../crates/slab-app-core/src/domain/services/pmid.rs#L1)） | 保留 shim 挂 domain 职责（`change_effect_for`/needs-restart 分发） | Phase 4 + Phase 5 |
| **F-Stack-1** 热重载仅 agent（[settings.rs:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)） | §4 生效模型矩阵 + `ChangeEffect` 回显 + 前端徽标 | Phase 4 |
| **F-Stack-2** 认证依赖 loopback 旁路 + 前端无 token 注入（[auth.rs:22-26](../../../bin/slab-server/src/api/middleware/auth.rs#L22-L26)、[index.ts:29-37](../../../packages/api/src/index.ts#L29-L37)） | 鉴权读 PMID 实时投影（§3.1）+ 前端可选 `getAdminToken` 拦截器 + 401 可操作诊断 | Phase 3 |
| **G4 config-path 部分** parse_env/setting_value 静默吞错（[app_config.rs:208-210](../../../crates/slab-config/src/app_config.rs#L208-L210)、[descriptor.rs:434-436](../../../crates/slab-config/src/descriptor.rs#L434-L436)） | 同 PMID-F5 / PMID-F7（warn/报错） | Phase 1 + Phase 2 |
| **G5** 脱敏白名单两处同步 | 同 PMID-F9（writeOnly 单源） | Phase 2 |
| **P1-1** env→PMID 单向桥或文档化 | §3.1 轨 A/B | Phase 3 |
| **P1-2** 热重载扩宽或 needs-restart 信号 | §4 `ChangeEffect` 模型 | Phase 4 |
| **P1-3** 5 层 logging 级联解析器或移除层 + 文档优先级 | §3.2 `resolve_effective_logging` | Phase 4 |
| **P1-4** telemetry json_schema + value_type + 三 exporter 对称 | §3.3 F3/F4 | Phase 1 |
| **P2-3** 脱敏 writeOnly 驱动 + mcp.servers 评估 | §3.7（mcp.servers 当前不纳入，设计上 env 引用） | Phase 2 |

---

## 附录 B：与既有契约的边界（对齐 [AGENTS.md](../../../AGENTS.md)）

- **架构边界不变**（[AGENTS.md:23](../../../AGENTS.md)）：`bin/slab-app → bin/slab-server → crates/slab-app-core → GrpcGateway → bin/slab-runtime → crates/slab-runtime-core`。本规范只改 `crates/slab-config`（回显/解析/脱敏）、`crates/slab-app-core::SettingsService`/`pmid.rs`（生效分发）、`bin/slab-server::auth_middleware`（读 PMID 投影）、`packages/api`（可选 token 拦截器）、前端设置页（徽标）。**不动 `slab-runtime-core`，不动推理 backend**。
- **`crates/slab-app-core` 保持 HTTP-free**（[AGENTS.md:28](../../../AGENTS.md)）：本规范在 app-core 只改 domain service（`SettingsService`/`pmid.rs`），不引入 HTTP 依赖。鉴权改读 PMID 投影是 `bin/slab-server` 侧改动（middleware），不破 app-core 边界。
- **API shape 变更走 `bun run gen:api`**（[AGENTS.md:26](../../../AGENTS.md)）：`SettingPropertyView` 新字段（`change_effect`/`overridden_by`）经 OpenAPI 刷 [v1.d.ts](../../../packages/api/src/v1.d.ts)，前端类型派生自 `paths[...]`（[slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md) §2.1 已核实此契约纪律）。
- **SQLx 迁移 append-only**（[AGENTS.md:32](../../../AGENTS.md)）：本规范**不涉及 DB 迁移**（设置存 settings.json，非 SQL；`PmidService` 经 `SettingsDocumentProvider` 读 JSON）。DB 契约缺陷（审计 §4）属另一计划。
- **跨 crate 契约走 `slab-types`/`slab-proto`**（[AGENTS.md:27](../../../AGENTS.md)）：`OtelExporter` 的 `JsonSchema` derive 在 [slab-otel](../../../crates/slab-otel/src/config.rs)；`otel_exporter_json_schema()` 由 slab-config 引用 slab-otel 的 `schema_for!`，跨 crate 但走既有依赖，无需新 proto。
- **不扩 `crates/slab-config` 的 HTTP 依赖**：`slab-config` 保持纯配置层，鉴权逻辑留在 `bin/slab-server`。
- **与 model_pack 计划的并轨**（[slab-model-pack-2026-06-17.md](./slab-model-pack-2026-06-17.md) Phase 3）：该计划增量扩展 `secret()` 白名单纳入 `download.<handle>`，**最终被本规范 Phase 2 的 writeOnly 驱动机制吸收**。建议执行序：本规范 Phase 2 先落地（writeOnly 驱动），model_pack 计划 Phase 3 改为"在 download schema 标 writeOnly"而非"扩硬编码白名单"，两者在 Phase 2 完成后自然并轨。
- **与 DB 契约计划、JSON-RPC 计划、安全（路径校验）计划正交**：本规范只在 §3.1 的前端 token 注入与 F-Stack-2 的 401 引导上与前端契约**轻接触**，不重复其他计划的 owned findings。

---

*本规范由首席配置架构师主导，逐 finding 直接读源码落地核实（app_config.rs/descriptor.rs/view.rs/pmid_service.rs/settings/document.rs/settings.rs/auth.rs/otel/config.rs/packages/api/index.ts 均已对齐 2026-06-18 工作树；审计 §1.3 的 `secret()`/`redact_setting_value` 纠错记录确认无误，本规范建立其上）。规范以 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §3.2 PMID-F1–F11、§2.3 F-Stack-1/F-Stack-2、§5 G4-config/G5 与 §6 P1-1/P1-2/P1-3/P1-4/P2-3 为闭环目标。*
