# 持久层与数据契约加固专项设计 (2026-06-17)

> **文档定位**：本规划书基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) 的 §2.2（数据转换链路 T1–T4）、§4（接口与数据库表设计 D1–D13）、§5（跨模块冗余 R2/R3/R4 与数据路径静默吞错 G4 的 data-path 部分）、§6（Action items P0-2/P0-5/P0-7/P1-5/P1-6/P1-8/P2-1/P2-5/P2-6/P2-7）所暴露的持久层与数据契约系统性短板，给出一套**DB 即契约最后一道防线、静默强转不可接受、单一真源、append-only migration 优先、约束声明即强制**的加固方案。
>
> **方法**：首席架构师主导，所有 `path:line` 证据在 2026-06-18 工作树重新核实。审计为 2026-06-17 发布；核实推翻或漂移的引用已在 §1.4「核实纠错记录」显式记录（与审计 §1.3 同纪律）。
>
> **读者**：实现该规范的工程师与审计员。本文为**契约级设计**，非概念稿——所有 schema 改动落到具名 migration 文件、所有 repository 改动落到具名函数与行号、所有验证落到可执行测试断言。

---

## 1. 背景与目标

### 1.1 现状与痛点

`slab.rs` 的持久层是 slab-app-core 之下的 SQLite + sqlx（[AGENTS.md](../../../AGENTS.md) §Architecture Boundaries；migrations 在 [crates/slab-app-core/migrations/](../../../crates/slab-app-core/migrations/)，append-only，§32）。整体分层良好——DTO↔Entity↔DB row 的三层切分清晰、openapi 代码生成驱动前端契约（审计 §1.1）。但审计在持久层暴露了三类系统性短板，构成本规范的闭环目标：

1. **JSON 列无 `json_valid()` CHECK + `json_set` 原地改写绕过 serde**（§2.2 T1、§4 D4）：[repository/model.rs:163](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L163) 用 `json_set(spec, '$.local_path', ?1)` 原地改 `models.spec`，完全绕过 `ModelSpec` 的 serde；[20260608000000_add_storage_value_checks.sql:44-61](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L44-L61) 给 `status`/`kind` 补了 CHECK 但**漏了 `json_valid(spec)`**。一次并发或 buggy 更新即可让 `spec` 进入 [parse_json_field](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L65-L71)（`:65-71`）拒绝的形态，`get_model`/`list_models` 对该行全部返回 `Store` 错误。

2. **静默强转 + 破坏性 fallback 在转换链路扩散**（§2.2 T2/T3/T4、§5 G4 data-path）：
   - [media_task.rs:427](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L427) `fps: row.fps as f32`（行字段 f64、实体 f32）高帧率舍入；[:394-396](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L394-L396)、[:424-425](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L424-L425) 的 `to_u32()`（[media_task.rs:493-495](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L493-L495)）即 `try_into().unwrap_or_default()`，负数或 `>u32::MAX` 的 width/height/frames 静默变 0。
   - [agent.rs:33](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L33) `config_json: String` 非 Option 但列可空（[20260325000000_agent_tables.sql:16](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L16) 实为 `NOT NULL DEFAULT '{}'`，见 §1.4 纠错），单行 NULL 让 `FromRow` 整表失败；[:55](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L55) `depth: r.depth as u32` 负值回绕成巨大 u32。
   - [agent.rs:66-77](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L66-L77) 内容解析 fallback 在存储 JSON 畸形时**破坏性清空** `tool_calls`/`name`/`tool_call_id`，原始串塞进 `Text`，无日志。
   - [task.rs:207-217](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L207-L217) `encode_task_payload` 最终 `.ok()` 失败返 `None` **无日志**（sibling `decode_task_payload` [:222/:227](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L222) 却 warn）；[media_task.rs:257-258](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L258) artifact JSON 畸形 → `"[]"` 无日志。

3. **DB 约束缺失与语义过载**（§4 D1–D13）：
   - [D1](../audits/code-audits-2026-06-17.md) `tasks.result_data` 信封化只保护主表（[20260530010000_task_payload_envelopes.sql:3](../../../crates/slab-app-core/migrations/20260530010000_task_payload_envelopes.sql#L3) 用了 `json_valid()`），三个 media 子表 `result_data` 既未信封化又与主表双写（[media_task.rs:162](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L162) 写主表、[:257-314](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L314) 写子表），单一真源不明。
   - [D2](../audits/code-audits-2026-06-17.md) `model_downloads.source_key` 列可空（[20260415010000_model_download_source_key.sql:2](../../../crates/slab-app-core/migrations/20260415010000_model_download_source_key.sql#L2)）但实体 `source_key: String` 非 Option（[entities/model_download.rs:10](../../../crates/slab-app-core/src/infra/db/entities/model_download.rs#L10)），legacy NULL 行经 sqlx `Option<String>→String` 解码 panic。
   - [D3](../audits/code-audits-2026-06-17.md) 多个 TEXT 状态/角色列缺 CHECK：`chat_messages.role` 有（[20260608000000_add_storage_value_checks.sql:66-68](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L66-L68)），但 `agent_thread_messages.role`（[20260519000000_agent_thread_messages.sql:9](../../../crates/slab-app-core/migrations/20260519000000_agent_thread_messages.sql#L9)）、`agent_threads.status`（[20260325000000_agent_tables.sql:14](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L14)）、`plugin_states.runtime_status`/`source_kind`（[20260422010000_plugin_states.sql:9](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L9)/[:3](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L3)）、`agent_memory_usage_events.source_kind`（[20260611010000_agent_memory_usage_source_kind.sql:4](../../../crates/slab-app-core/migrations/20260611010000_agent_memory_usage_source_kind.sql#L4)）**全无 CHECK**。
   - [D7](../audits/code-audits-2026-06-17.md) 布尔列 INTEGER 无 `IN (0,1)`：`plugin_states.enabled`（[20260422010000_plugin_states.sql:8](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L8)）、`agent_memory_phase1_outputs.selected_for_phase2`（[20260611000000_agent_memories.sql:32](../../../crates/slab-app-core/migrations/20260611000000_agent_memories.sql#L32)）、`audio_transcription_tasks.detect_language`（[20260421010000_media_tasks.sql:59](../../../crates/slab-app-core/migrations/20260421010000_media_tasks.sql#L59)），repo 用 `value != 0` 解码（[media_task.rs:454](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L454)），手工写入的 `2`/`-1` 静默成 true。
   - [D5](../audits/code-audits-2026-06-17.md) 选择状态横跨 `model_config_state` 与 `models` 两表，单次 PUT 跨表无事务包裹（[schemas/models.rs:213](../../../crates/slab-app-core/src/api/schemas/models.rs#L213) `UpdateModelConfigSelectionRequest` 与 [:196](../../../crates/slab-app-core/src/api/schemas/models.rs#L196) `UpdateModelEnhancementRequest`）。
   - [D6](../audits/code-audits-2026-06-17.md) `agent_threads.config_json` 持久化（[agent.rs:114](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L114) 写）但 [AgentThreadResponse](../../../crates/slab-app-core/src/api/schemas/agent.rs#L350-L360)（`:350-360`）从不返回——持久化的配置 API 永不回显。
   - [D10](../audits/code-audits-2026-06-17.md) `tasks.core_task_id` 无 UNIQUE（[20240101000000_initial.sql:10](../../../crates/slab-app-core/migrations/20240101000000_initial.sql#L10)）却被文档化为 1:1；且 `Option<i64>` 装不下完整 u64。
   - [D13](../audits/code-audits-2026-06-17.md) 文档腐烂：[entities/model.rs:15](../../../crates/slab-app-core/src/infra/db/entities/model.rs#L15) 等注释引用已删除的列/表。

4. **跨模块冗余（data-layer 部分）**（§5 R2/R4、G4 已并入上文）：
   - [R2](../audits/code-audits-2026-06-17.md) `RuntimePresets` 组装散落多处——[command.rs:33-58](../../../crates/slab-app-core/src/infra/model_packs/command.rs#L33-L58) `build_runtime_presets`、[schemas/models.rs:657-669](../../../crates/slab-app-core/src/api/schemas/models.rs#L657-L669)（响应映射）、[:954-966](../../../crates/slab-app-core/src/api/schemas/models.rs#L954-L966)（请求→domain），加字段须改 ≥3 处（审计称 4 处，§1.4 纠错）。
   - [R4](../audits/code-audits-2026-06-17.md) 未知状态默认不一致：[domain/models/task.rs:45-54](../../../crates/slab-app-core/src/domain/models/task.rs#L45-L54) unknown→`Failed`，[repository/agent.rs:13-22](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L13-L22) unknown→`Pending`——损坏的 agent 线程被静默当作可恢复。

### 1.2 目标

| 目标 | 衡量标准 |
|------|----------|
| **G1 DB 是契约的最后一道防线** | 所有 JSON 列加 `json_valid()` CHECK；所有枚举/布尔列加 `CHECK (col IN (...))`；违反约束的写被 DB 拒（round-trip 测试断言）。对齐 T1/D4/D3/D7。 |
| **G2 静默强转不可接受** | repository 层消灭 `as f32`/`as u32`/`unwrap_or_default()` 静默路径；越界/截断 `warn!` 并拒绝而非吞错。对齐 T2/T3/G4。 |
| **G3 单一真源** | `tasks.result_data` 与三个 media 子表 result_data 确立唯一真源 + 版本化解码注册表；选择状态跨表写入经事务包裹。对齐 D1/D5。 |
| **G4 FromRow NULL-safety** | 可空列对应 `Option<T>`；数值列用 `try_from` 而非 `as`；时间戳用 `DateTime<Utc>`。对齐 T3/D2/D6。 |
| **G5 append-only migration 优先** | 所有 schema 改动走具名 append-only SQLx migration；NULL 先回填再加 NOT NULL；无数据损失。对齐 AGENTS.md §32。 |
| **G6 破坏性 fallback 加诊断** | 内容解析 fallback 分支至少 `warn!` + 旁路 raw 字段保留原始串，不再静默清空 tool_calls。对齐 T4。 |
| **G7 命名与文档治理** | `kind`/`status`/`source` 过载命名消歧（v3 `deployment` 已由 model_pack 计划处理，本规范负责 DB 侧）；腐烂注释清理。对齐 D8/D13。 |

### 1.3 非目标（指向 sibling 计划）

本规范**不**触及以下轨道，由 sibling 计划负责：

- **model_pack 配置体系**（`variant.id` 唯一性、`chat_template` asset-ref、子文档 schema、`$config` vs `$load_config`、manifest `version`/`default_preset`/`status`）——见 [slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md)。
- **PMID 回显与运行时配置热重载**（env→PMID 双源、5 层 logging override、`telemetry.metrics_exporter` 类型 laundering、`parse_env` 静默回退）——属 PMID 治理计划。
- **路径包含校验统一**（R1，3 套 `ensure_path_within_root`）——属 reliability/safety 计划。
- **gRPC 错误跨边界保结构**（F-Stack-3）——属 model_pack Phase 2 / reliability 计划。
- **JSON-RPC 宿主骨架去重**（R5）、pending-map 超时（G1 RPC）、process_supervisor Drop（G3）——属跨进程可靠性计划。

**与 model_pack 计划的衔接**：model_pack 计划 Phase 4 已声明「`model_config_state.selected_engine` 经 Rust re-serialize 整列写、禁用 `json_set`」（针对 T1/D4 的**该列**）。**本规范提供的是 substrate**——仓库范围内 `json_set`→re-serialize + `json_valid()` CHECK 的**通用 remediation**（覆盖 `models.spec` 及所有受影响 JSON 列），model_pack Phase 4 是该 substrate 的一个消费者。两者不重复劳动：本规范 Phase 1 落地通用机制，model_pack Phase 4 在其上落 `selected_engine` 列。

### 1.4 核实纠错记录（2026-06-18 工作树，对齐审计 §1.3 纪律）

| 审计原文（2026-06-17） | 2026-06-18 核实结论 | 处理 |
|------|----------|------|
| **T3** 「`agent_threads.config_json` SQL 列无 NOT NULL 保证」 | **部分推翻**。[20260325000000_agent_tables.sql:16](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L16) 实为 `config_json TEXT NOT NULL DEFAULT '{}'`——**列确有 NOT NULL + 默认值**。审计「无 NOT NULL 保证」描述不准。但 T3 的**核心风险仍然成立**：① 任何经旧迁移/手工/部分写入绕过默认值的 NULL 行（SQLite 默认值不强制存量 NULL，且 `INSERT` 显式绑 NULL 仍可写入 NULL），仍会让 `FromRow` 解码 `String` 失败、整表报错；② Rust 端 `config_json: String` 与「DB 列在历史路径下可能持有 NULL」的假设不符，**Option 化仍是正确防御**。 | 保留 T3 为有效发现，但根因从「列无 NOT NULL」修正为「Rust 类型与 DB 可能的 NULL 不匹配 + 缺乏逐行容错」 |
| **R2** 「`RuntimePresets` 组装在 4 处重复」 | **部分推翻**。审计称 [command.rs:163-183](../../../crates/slab-app-core/src/infra/model_packs/command.rs#L163-L183) 有 `build_runtime_presets_from_manifest`——**该函数不存在**（command.rs 全文无此符号）。当前真实组装点是 3 处：[command.rs:33-58](../../../crates/slab-app-core/src/infra/model_packs/command.rs#L33-L58) `build_runtime_presets`、[schemas/models.rs:657-669](../../../crates/slab-app-core/src/api/schemas/models.rs#L657-L669)、[:954-966](../../../crates/slab-app-core/src/api/schemas/models.rs#L954-L966)。 | 保留 R2 为有效发现（dedup 仍必要），组装点数从 4 修正为 3 |
| **R3** 「`manifest_status` ↔ `pack_status_from_unified` 是无共享测试的逆 twin」 | **推翻**。`grep manifest_status\|pack_status_from_unified\|PackModelStatus` 在 slab-app-core 全量**零命中**。[command.rs:26-31](../../../crates/slab-app-core/src/infra/model_packs/command.rs#L26-L31) 是 `default_status_for_runtime_bridge`（2-arm：`NotDownloaded`/`Ready`），[mod.rs:650](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L650) 是 `infer_artifact_format_from_config`（format 推断，非 status 映射）。**审计 R3 的「4-arm 逆 twin」描述基于已不存在的代码**。 | **降级为 N/A**（已在 §6 复审清单登记；本规范不为不存在的 twin 设计 remediation；R3 的「单一 match」诉求由 R4 统一未知状态 + model_pack Phase 0 的 `RuntimeModelStatus` 收口覆盖） |
| **G4** 「`media_task.rs:490` artifact JSON 畸形 → `[]` 无日志；`:426` frames 溢出 → `unwrap_or_default()`」 | **行号漂移**。artifact JSON silent `"[]"` 实际在 [media_task.rs:257-258](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L258)（非 :490）；`frames` 溢出现在 [:426](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L426) `row.frames.try_into().unwrap_or_default()`（行号准确）。 | 保留 G4，artifact JSON 行号更正为 :257-258 |
| **D2** 「[20260608000000...:140] 未补 NOT NULL」 | **确认**。[20260608000000_add_storage_value_checks.sql:140](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L140) 仍是 `source_key TEXT`（重建表时未加 NOT NULL），且 [20260415010000...:7-9](../../../crates/slab-app-core/migrations/20260415010000_model_download_source_key.sql#L7-L9) 回填只覆盖 `NULL`/空串，**legacy NULL 存活**。 | 保留 D2 |
| **D5** 「[20260409000000_model_config_state.sql:1-6]」 | **路径核实**。该 migration 文件存在且定义 `model_config_state` 表（`selected_preset_id`/`selected_variant_id`）；[20260607000000_model_download_state.sql:1-2](../../../crates/slab-app-core/migrations/20260607000000_model_download_state.sql#L1-L2) 加 `materialized_artifacts`/`selected_download_source`。两表跨写无事务包裹的判断成立。 | 保留 D5 |
| **D13** 「[entities/model.rs:15] 注释描述 `models.provider`」 | **部分已修**。`models.provider` 确已在 [20260530000000_remove_models_provider.sql:3](../../../crates/slab-app-core/migrations/20260530000000_remove_models_provider.sql#L3) `DROP COLUMN`；但需核实注释是否同步删除（§3.10 复审）。[repository/task.rs:11-12](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L11-L12) `model_catalog` 注释、[entities/chat.rs:10](../../../crates/slab-app-core/src/infra/db/entities/chat.rs#L10) role 注释仍需核实。 | 保留 D13，逐条核实后清理 |

---

## 2. 架构设计原则

### 2.1 DB 是契约的最后一道防线（P1）

应用层的 Rust 类型校验是**第一道**防线，但它可被绕过——`json_set` 原地改写（T1）、并发竞态、手工 SQL、未来其它语言的写入器。**DB 约束是不可绕过的最后防线**：JSON 列必有 `json_valid()`、枚举列必有 `CHECK (col IN (...))`、布尔列必有 `CHECK (col IN (0,1))`。违反约束的写应在 DB 层即失败，round-trip 测试必须断言这一点（审计 §6.4）。

### 2.2 静默强转不可接受（P2）

`as f32`、`as u32`、`unwrap_or_default()`、`.ok()` 在**数据路径**上是 anti-pattern：它们把数据完整性错误降级为语义错误的「0×0 任务」「空 tool_calls」「丢失的 result_data」，坏行以合法形态存活而非暴露。规则：
- **类型必须匹配**：DB f64 列对应 Rust `f64`（不 `as f32`）；i64 列装 u32 语义值时用 `u32::try_from` 而非 `as`。
- **越界拒绝**：`try_from` 失败时 `warn!`（含原始值）并返 `Err`，**不得** `unwrap_or_default()`。
- **吞错加诊断**：凡 `.ok()`/`unwrap_or_default()` 在数据路径，至少 `warn!` 记录字段名与失败原因。

### 2.3 单一真源（P3）

`result_data` 在 `tasks` 主表与三个 media 子表双写、view 查询 SELECT 两者、`media_state_from_task` 只消费主表——这是典型的「写两份、读一份、另一份腐烂」反模式。规则：**每个语义值只有一个权威存储**。媒体子表的 `result_data` 要么删除（主表信封为真源）、要么独立语义化（不再与主表重复）。同理选择状态跨 `model_config_state` + `models` 两表必须经事务包裹。

### 2.4 append-only migration 优先（P4，对齐 AGENTS.md §32）

SQLx migration 是 append-only（[AGENTS.md:32](../../../AGENTS.md#L32)）。所有 schema 改动走**新 migration 文件**（`YYYYMMDDHHMMSS_description.sql`），**不编辑**既有 migration。SQLite 的限制（无法直接 `ALTER ... ADD CONSTRAINT`）通过 `CREATE TABLE _new ... ; INSERT ... ; DROP old ; ALTER RENAME` 重建模式实现，与 [20260608000000_add_storage_value_checks.sql](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql) 既有的重建手法一致。回填先于 NOT NULL：先 `UPDATE ... SET col = default WHERE col IS NULL`，再在重建时加 NOT NULL。

### 2.5 约束声明即强制（P5）

CHECK 约束一旦声明，SQLite 在 `INSERT`/`UPDATE` 时强制（NULL 行为除外）。**声明 = 强制**，无应用层兜底的灰色地带。因此枚举值集必须与 Rust enum 的 `FromStr`/`serde` 表示**完全同步**——加 Rust enum 变体的 PR 必须同时加 migration 的 CHECK 值，反之亦然（§3.6 给出同步契约）。

### 2.6 破坏性 fallback 加旁路 raw（P6）

存储 JSON 畸形时，fallback 到 `Text(raw)` 是**可接受的降级**（保数据不丢），但**不可接受的是静默清空结构化字段**（`tool_calls = Vec::new()`）。规则：fallback 分支必须 ① `warn!`（含 row id、失败原因）、② 把原始串塞进旁路字段（`Text(content)` 已满足）、③ **不再构造看起来合法但内容错误的强类型消息**。前端据此知道「这是 raw fallback」而非「这是一条没有 tool_calls 的合法 assistant 消息」。

---

## 3. 核心设计

### 3.1 集群 A：JSON 列 `json_valid()` CHECK + re-serialize 整列写（T1/D4，对齐 P0-2）

**根因**：[repository/model.rs:163-165](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L163-L165) 用 SQLite `json_set(spec, '$.local_path', ?1)` 原地改 JSON。`json_set` 是 SQL 函数，**绕过 `ModelSpec` 的 serde**——它不校验写入值是否符合 Rust 类型（例如把 `local_path` 改成数组、把 `pricing` 改成字符串都不会报错）。配合 [20260608000000_add_storage_value_checks.sql:44-61](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L44-L61) **漏加 `json_valid(spec)`**，一次坏写让 `spec` 进入 [parse_json_field](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L65-L71) 拒绝的形态，`get_model`/`list_models` 对该行全部 `Store` 错误。

**Remediation（substrate，仓库范围通用）**：

1. **repository 层 re-serialize 整列写**：[repository/model.rs](../../../crates/slab-app-core/src/infra/db/repository/model.rs) 的 `update_*` 路径改为「读出当前 `ModelSpec` → 在 Rust 中 mutate → `serde_json::to_string` 整列写回」。删除所有 `json_set(spec, ...)` 调用。这是审计 P0-2 的 Rust 侧。
2. **DB 侧 `json_valid()` CHECK**：新 migration 给 `models.spec`（及其它受影响 JSON 列，见下表）加 `CHECK (json_valid(spec))`。SQLite 重建表模式（§2.4）。

**受影响 JSON 列清单**（本规范统一加 `json_valid()` CHECK）：

| 列 | migration | 当前状态 | 本规范动作 |
|---|---|---|---|
| `models.spec` | [20240101000000_initial.sql](../../../crates/slab-app-core/migrations/20240101000000_initial.sql) / 重建 [20260608000000...:44-61](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L44-L61) | 无 CHECK | 加 `CHECK (json_valid(spec))` |
| `models.materialized_artifacts` | [20260607000000_model_download_state.sql:1](../../../crates/slab-app-core/migrations/20260607000000_model_download_state.sql#L1) `DEFAULT '{}'` | 无 CHECK | 加 `CHECK (materialized_artifacts IS NULL OR json_valid(materialized_artifacts))` |
| `model_config_state.*` JSON 列 | [20260409000000_model_config_state.sql](../../../crates/slab-app-core/migrations/20260409000000_model_config_state.sql) | 无 CHECK | 逐列评估，按需加 |
| `agent_threads.config_json` | [20260325000000_agent_tables.sql:16](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L16) `DEFAULT '{}'` | 无 CHECK | 加 `CHECK (json_valid(config_json))`（与 §3.3 Option 化协同：NULL 不触发 CHECK，`json_valid(NULL)` 返 NULL → CHECK 通过） |
| `tasks.result_data` | [20260530010000_task_payload_envelopes.sql:3](../../../crates/slab-app-core/migrations/20260530010000_task_payload_envelopes.sql#L3) | 已有 `json_valid()`（信封迁移） | **不变**（已是正确范例） |
| `media 子表.result_data` ×3 | [20260421010000_media_tasks.sql:16/41/64](../../../crates/slab-app-core/migrations/20260421010000_media_tasks.sql#L16) | 无 CHECK | §3.2 决定真源后，若保留则加 `json_valid()` |

**CHECK 测试（§6）**：`INSERT INTO models (id, ..., spec) VALUES ('bad', ..., 'not-json{')` 应被 DB 拒（`CHECK constraint failed`）。

**与 model_pack 计划的边界**：model_pack Phase 4 在本规范 substrate 上落 `model_config_state.selected_engine`（re-serialize + 禁 json_set）。本规范 Phase 1 先落通用机制。

### 3.2 集群 B：`result_data` 单一真源 + 版本化解码注册表（D1，对齐 P0-5）

**根因**：[20260530010000_task_payload_envelopes.sql:3](../../../crates/slab-app-core/migrations/20260530010000_task_payload_envelopes.sql#L3) 把 `tasks.result_data` 重包为 `{kind:"task_result",version:1,data:...}` 信封（带 `json_valid()`）。但三个 media 子表 `result_data`（[20260421010000_media_tasks.sql:16/41/64](../../../crates/slab-app-core/migrations/20260421010000_media_tasks.sql#L16)）**从不信封化**。[media_task.rs:162](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L162) 既写主表信封（`insert_task_row(..., task.result_data.as_deref())`），又由 `update_*_result`（[:250-314](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L250-L314)）写子表裸 result_data。view 查询（[:375-382](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L375-L382)）SELECT 两者，`media_state_from_task`（[:482](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L482)）只消费**主表**那个算进度。子表的 `result_data` 是**死重量双写**。更糟：[task.rs:219-239](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L219-L239) `decode_task_payload` 遇非信封 payload 静默返 `None`+warn——未来版本升级后旧任务结果从 API 静默消失。

**决策：主表 `tasks.result_data` 是唯一真源；media 子表的 `result_data` 列删除。**

理由：① `media_state_from_task` 已只用主表（子表 result_data 无消费者）；② 信封 + 版本化解码器已在主表就位，子表裸存是历史包袱；③ 删子表列消除「双写双读、单一真源不明」。子表保留**自身特有**字段（`artifact_paths`/`video_path`/`transcript_text` 等），这些不是 `result_data` 的副本而是 media-type-specific 派生数据。

**Remediation**：

1. **migration（append-only，重建子表）**：新 migration `20260620000000_drop_media_subtable_result_data.sql`，对 `image_generation_tasks`/`video_generation_tasks`/`audio_transcription_tasks` 各重建表去掉 `result_data` 列。`INSERT INTO _new SELECT (除 result_data 外所有列) FROM old`。
2. **repository 层**：[media_task.rs:250-314](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L250-L314) `update_*_result` 删除 `result_data` 参数与绑定；调用方改为只更新主表 `tasks.result_data`（经 `update_task_result` 已有路径）。view 查询去掉子表 `result_data` SELECT。
3. **版本化解码注册表**：[task.rs:219-239](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L219-L239) `decode_task_payload` 重构为注册表驱动：

```rust
// 概念草图（非最终代码）
trait TaskPayloadDecoder {
    const KIND: &'static str;
    fn decode(envelope: &TaskPayloadEnvelope) -> Option<Value>;
}

struct TaskResultV1Decoder;  // kind="task_result", version=1
// 未来：struct TaskResultV2Decoder;  // 新版本/新形态

fn decode_task_payload(raw: Option<String>) -> Option<Value> {
    let raw = raw?;
    let envelope: TaskPayloadEnvelope = serde_json::from_str(&raw).map_err(|e| {
        tracing::warn!(error = %e, "stored task payload is not an envelope; ignoring");
    }).ok()?;
    DECODERS.iter().find_map(|d| d.try_decode(&envelope))
    // 找不到匹配 decoder → warn!(kind, version, "no decoder for payload envelope")
}
```

加版本/形态时只需注册新 decoder，旧 decoder 保留以读旧数据——不再「版本升级→旧数据静默消失」。**保留** `decode_task_payload` 既有的 `warn!`（[:222/:227](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L222)），未匹配也 warn。

**数据迁移**：子表 `result_data` 在删列前**无需回填主表**（主表已有信封、子表是副本）。但需一次性校验：`SELECT task_id FROM image_generation_tasks WHERE result_data IS NOT NULL AND task_id NOT IN (SELECT id FROM tasks WHERE result_data IS NOT NULL)` 应为空集（否则有「只存子表、主表 NULL」的孤儿，migration 前先回填主表）。

**CHECK 测试**：删列后 `SELECT result_data FROM image_generation_tasks` 应报「no such column」；主表信封读写 round-trip 一致。

### 3.3 集群 C：FromRow NULL-safety + `Option<String>` + `try_from`（T3/D2/D6，对齐 P0-7）

**根因**：Rust FromRow 类型与 DB 列的可空性不匹配 + 数值静默强转。

| 项 | 当前（行号已 2026-06-18 核实） | 风险 | Remediation |
|---|---|---|---|
| `AgentThreadRow.config_json` | [agent.rs:33](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L33) `config_json: String`；列 [20260325000000...:16](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L16) `NOT NULL DEFAULT '{}'`（§1.4 纠错：审计「无 NOT NULL」不准，但显式 INSERT NULL 仍可绕过默认） | 历史路径 NULL 行让 FromRow 整表 panic | 改 `config_json: Option<String>`；读处 `unwrap_or_default()`；migration 加 `CHECK (config_json IS NULL OR json_valid(config_json))`（§3.1） |
| `AgentThreadRow.depth` | [agent.rs:55](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L55) `depth: r.depth as u32`（r.depth: i64） | 负值回绕成巨大 u32 | 改 `u32::try_from(r.depth).unwrap_or_else(\|v\| { warn!(...); 0 })`——更优：返 `Result` 让坏行显式失败而非静默 0（§3.4 统一） |
| `AgentThreadRow.created_at`/`updated_at` | [agent.rs:35-36](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L35-L36) `String` | 畸形时间戳蒙混过关、下游才崩 | 改 `DateTime<Utc>`（sqlx 支持），`FromStr` 失败即 FromRow 报错（早暴露）；§3.9 时间戳统一 |
| `ModelDownloadRecord.source_key` | [entities/model_download.rs:10](../../../crates/slab-app-core/src/infra/db/entities/model_download.rs#L10) `source_key: String`；列 [20260415010000...:2](../../../crates/slab-app-core/migrations/20260415010000_model_download_source_key.sql#L2) 可空 | legacy NULL（[:7-9](../../../crates/slab-app-core/migrations/20260415010000_model_download_source_key.sql#L7-L9) 回填未覆盖所有路径）panic | ① migration 一次性 NULL 清理（`UPDATE ... SET source_key = 'auto::' \|\| repo_id \|\| '::' \|\| filename WHERE source_key IS NULL`）；② 重建表加 `NOT NULL`；③ 实体保持 `String`（清理后保证非空）**或**改 `Option<String>` 防御——本规范选**清理 + NOT NULL**（单一真源，P3） |
| `AgentThreadResponse` 不返回 config_json | [schemas/agent.rs:350-360](../../../crates/slab-app-core/src/api/schemas/agent.rs#L350-L360) 无该字段；[agent.rs:114](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L114) 写入 | 持久化的配置 API 永不回显（D6） | **决策：暴露**——`AgentThreadResponse` 加 `config_json: Option<String>`（与 §3.3 Option 化协同）。理由：config_json 是 thread 上下文重建所需，停写会丢失恢复能力。加字段需 `bun run gen:api` 刷 [v1.d.ts](../../../packages/api/src/v1.d.ts) |

**`core_task_id`（D10 顺带）**：[entities/task.rs:16-17](../../../crates/slab-app-core/src/infra/db/entities/task.rs#L16-L17) 注释「slab-core runtime TaskId (u64)」暗示 1:1，但 [20240101000000_initial.sql:10](../../../crates/slab-app-core/migrations/20240101000000_initial.sql#L10) 无 UNIQUE。Remediation：migration 加 `CREATE UNIQUE INDEX idx_tasks_core_task_id ON tasks(core_task_id) WHERE core_task_id IS NOT NULL`（partial unique，允许多个 NULL 的 server-only 任务）。类型 `Option<i64>` 装不下完整 u64——本规范**不**改类型（slab-core runtime TaskId 实际范围未超 i64，改类型跨 crate 影响大），仅在注释标明「u64 上界受 i64 限制」。

### 3.4 集群 D：截断/溢出拒绝（T2，对齐 P0-7 周边）

**根因**：[media_task.rs:427](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L427) `fps: row.fps as f32`（row.fps: f64）；[:394-396](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L394-L396)/[:424-425](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L424-L425) 经 [:493-495](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L493-L495) `to_u32 = try_into().unwrap_or_default()`。

| 字段 | 当前 | Remediation |
|---|---|---|
| `fps` | DB f64 → 实体 f32 via `as` | **实体升 f64**（与 DB 列一致），渲染/请求侧按需 `.round() as f32` 但**不**在 FromRow 强转 |
| `width`/`height`/`requested_count` | `to_u32` 静默 0 | 改 `try_from` 失败时 `warn!(row_id, value, "out-of-range width; rejecting row")` 并返 `Err(AppCoreError::Store(...))`——**坏行显式失败**而非「0×0 任务」存活。或：若业务允许，保留 `unwrap_or_default()` 但**加 warn**（最小改动）。本规范选**warn + 拒绝**（P2） |
| `frames` | [media_task.rs:426](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L426) `row.frames.try_into().unwrap_or_default()` | 同上 |

** CHECK 测试**：插入 `width = -1` 的行（migration 重建表时本就应加 `CHECK (width >= 0)`，见 §3.6），DB 拒；repo 解码 `width = i64::MAX` 时（若 CHECK 未覆盖）返 `Err`。

### 3.5 集群 E：破坏性 fallback 加 warn + 旁路 raw（T4，对齐 G4 data-path）

**根因**：[agent.rs:66-77](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L66-L77) `serde_json::from_str::<ConversationMessage>(&self.content).unwrap_or_else(|_| ConversationMessage{ role, content: Text(self.content), name: None, tool_call_id: None, tool_calls: Vec::new() })`。存储 JSON 畸形时，`tool_calls`/`name`/`tool_call_id` **全部清空**，原始串塞进 `Text`——对带工具调用的 assistant 消息，这是**破坏性、静默、无日志**的数据损失。同型：[repository/chat.rs:58-65](../../../crates/slab-app-core/src/infra/db/repository/chat.rs#L58-L65) ChatMessage.content、[domain/models/chat.rs:230-236](../../../crates/slab-app-core/src/domain/models/chat.rs#L230-L236)。

**Remediation（P6）**：

1. **fallback 分支加 `warn!`**：所有上述 `unwrap_or_else` 改为先 `serde_json::from_str`，失败时 `tracing::warn!(row_id, role, error, "stored message content is not valid JSON; falling back to raw text")` 再构造 fallback。
2. **旁路 raw 已满足**：`Text(self.content)` 把原始串塞进 Text——这是正确降级（保数据）。**关键是诊断可见**。
3. **不再构造「看起来合法」的强类型消息**：保留当前 fallback 形态（role + Text + 空 tool_calls），但 warn 让运维知道「这条是 raw fallback，不是合法的无工具调用消息」。

**G4 data-path 吞错（encode_task_payload / media artifact）**：

| 项 | 当前 | Remediation |
|---|---|---|
| `encode_task_payload` | [task.rs:207-217](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L207-L217) 最终 `.ok()` 失败返 `None` **无日志** | 加 `tracing::warn!(error, "failed to encode task payload envelope; result_data will be NULL")`——与 sibling `decode_task_payload` [:222/:227](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L222) 的 warn 对称 |
| media artifact JSON | [media_task.rs:257-258](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L258) `serde_json::to_string(artifact_paths).unwrap_or_else(\|_\| "[]".to_owned())` 无日志 | 加 `tracing::warn!(task_id, "failed to serialize artifact_paths; storing []")` |
| `decode_string_array` | [media_task.rs:489-491](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L489-L491) 静默 `[]` | 加 warn |

**`parse_env`（PMID-F7/G4 config-path 部分）属 PMID 计划，本规范不触及**（§1.3 边界）。

### 3.6 集群 F：CHECK 约束补齐（D3/D7，对齐 P1-5）

**根因**：[20260608000000_add_storage_value_checks.sql:66-68](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql#L66-L68) 给 `chat_messages.role` 加了 CHECK（且 [schemas/validation.rs:89](../../../crates/slab-app-core/src/api/schemas/validation.rs#L89) `validate_chat_role` 同步），但**约束应用不一致**——下列列无 CHECK：

| 列 | migration（行号核实） | 枚举值（来自 Rust） | 本规范 CHECK |
|---|---|---|---|
| `agent_thread_messages.role` | [20260519000000_agent_thread_messages.sql:9](../../../crates/slab-app-core/migrations/20260519000000_agent_thread_messages.sql#L9) | 同 chat role（system/developer/user/assistant/tool/function） | `CHECK (role IN ('system','developer','user','assistant','tool','function'))` |
| `agent_threads.status` | [20260325000000_agent_tables.sql:14](../../../crates/slab-app-core/migrations/20260325000000_agent_tables.sql#L14) | `ThreadStatus` enum（[agent.rs:13-22](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L13-L22) fallback Pending，需核实变体集） | `CHECK (status IN (<ThreadStatus 变体>))` |
| `plugin_states.runtime_status` | [20260422010000_plugin_states.sql:9](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L9) | [plugin.rs:40-42](../../../crates/slab-app-core/src/domain/services/plugin.rs#L40-L42) 三字面量 | `CHECK (runtime_status IN (<三字面量>))` |
| `plugin_states.source_kind` | [20260422010000_plugin_states.sql:3](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L3) | [plugin.rs:37-39](../../../crates/slab-app-core/src/domain/services/plugin.rs#L37-L39) | `CHECK (source_kind IN (<变体>))` |
| `agent_memory_usage_events.source_kind` | [20260611010000_agent_memory_usage_source_kind.sql:4](../../../crates/slab-app-core/migrations/20260611010000_agent_memory_usage_source_kind.sql#L4) | 5 变体（审计） | `CHECK (source_kind IN (<5 变体>))` |
| `plugin_states.enabled` | [20260422010000_plugin_states.sql:8](../../../crates/slab-app-core/migrations/20260422010000_plugin_states.sql#L8) INTEGER | bool | `CHECK (enabled IN (0,1))` |
| `agent_memory_phase1_outputs.selected_for_phase2` | [20260611000000_agent_memories.sql:32](../../../crates/slab-app-core/migrations/20260611000000_agent_memories.sql#L32) INTEGER | bool | `CHECK (selected_for_phase2 IN (0,1))` |
| `audio_transcription_tasks.detect_language` | [20260421010000_media_tasks.sql:59](../../../crates/slab-app-core/migrations/20260421010000_media_tasks.sql#L59) INTEGER（可空） | Option<bool> | `CHECK (detect_language IS NULL OR detect_language IN (0,1))` |
| `image_generation_tasks.width`/`height`/`requested_count` | [20260421010000_media_tasks.sql](../../../crates/slab-app-core/migrations/20260421010000_media_tasks.sql) | u32 语义 | `CHECK (width >= 0 AND width <= 4294967295)`（与 §3.4 协同） |

**同步契约（P5）**：每个 CHECK 的值集必须与对应 Rust enum 的 `FromStr`/`serde` 表示**逐字同步**。PR 模板加 checkbox：「☐ 若改 enum 变体，已同步 DB CHECK migration」。值集单源：考虑抽 `crates/slab-app-core/src/infra/db/check_constraints.rs` 常量（如 `pub const THREAD_STATUS_VALUES: &[&str] = &["pending","running",...]`），migration 文档与 Rust enum 都引用——但 SQL migration 是静态 SQL，无法 import Rust 常量，故采用**注释 + 测试**双保险：migration 注释「// keep in sync with ThreadStatus in agent.rs」，测试断言 `THREAD_STATUS_VALUES` 与 DB `SELECT DISTINCT status FROM agent_threads` 的合法集一致（§6）。

**per-column 测试（§6.4）**：每个新 CHECK 配一条「违反约束的写应被 DB 拒」测试，如 `INSERT INTO plugin_states (..., enabled) VALUES (..., 2)` 期望 `CHECK constraint failed`。

### 3.7 集群 G：跨表事务包裹（D5，对齐 P1）

**根因**：[model_config_state](../../../crates/slab-app-core/migrations/20260409000000_model_config_state.sql)（`selected_preset_id`/`selected_variant_id`）与 [models](../../../crates/slab-app-core/migrations/20260607000000_model_download_state.sql)（`selected_download_source`/`materialized_artifacts`）跨表，[schemas/models.rs:213](../../../crates/slab-app-core/src/api/schemas/models.rs#L213) `UpdateModelConfigSelectionRequest` 与 [:196](../../../crates/slab-app-core/src/api/schemas/models.rs#L196) `UpdateModelEnhancementRequest` 一次 PUT 写两表，**服务层无事务包裹**——中途失败留不一致状态。

**Remediation**：服务层（`ModelService::update_model_config_selection` / `update_model_enancement`）用 sqlx `pool.begin()` → 两表写 → `tx.commit()`。失败 `tx.rollback()`（drop 即自动回滚）。本规范不改 schema，只改 service 方法签名（`&pool` → `&mut tx` 或内部 begin）。

**与 model_pack 计划的边界**：model_pack Phase 4 落 `selected_engine` 列时也会触发该服务方法；本规范先建立事务包裹的 substrate，model_pack Phase 4 在其上加列。

### 3.8 集群 H：命名治理（D8）

**根因**：`kind`/`status`/`source` 跨表多语义过载。

| 过载 | 现状 | Remediation（本规范 DB 侧） |
|---|---|---|
| `models.kind`（仅 local\|cloud） | [20240408000000_model_kind_and_backend.sql:5](../../../crates/slab-app-core/migrations/20240408000000_model_kind_and_backend.sql#L5) | **不改列名**（重命名跨 crate 影响大）；在实体注释标明「deployment type, values: local\|cloud」消歧。v3 的 `deployment` 字段已在 model_pack 计划处理（manifest 层），DB 层保持 `kind` + 注释 |
| `status` 跨 5 表 | task/model/thread/plugin/session 各异 | 各表 CHECK 的值集本身就是消歧（§3.6）；注释标明语义（`task_state`/`model_lifecycle_state`/`thread_state`）。**不改列名** |
| `source` 三义 | `model_downloads.source_key`（slug）/ `models.selected_download_source`（JSON）/ `plugin_states.source_kind`（enum） | 注释消歧；`source_kind` 已比 `source` 清晰，保留 |

**本规范 D8 范围**：纯文档/注释治理，不动 schema。`models.kind`→`deployment_type` 的真正重命名属未来大版本，不在本迭代。`BackendConfigDocument.id` 改 Option 属 model_pack 计划（F7），不在本规范。

### 3.9 集群 I：时间戳统一（D9）

**根因**：[20260611000000_agent_memories.sql:37/54](../../../crates/slab-app-core/migrations/20260611000000_agent_memories.sql#L37) 用 `DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))`（毫秒）；[20240101000000_initial.sql:17](../../../crates/slab-app-core/migrations/20240101000000_initial.sql#L17) 老表无默认；[agent.rs:164](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L164) 手写 strftime（毫秒）与 `to_rfc3339()`（亚纳秒）混用。

**Remediation（务实，不搞大重命名）**：
1. **老表加默认**：新 migration 给无默认的时间戳列加 `DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))`（SQLite `ALTER TABLE` 不支持改默认，需重建表——仅对真正缺默认且写入路径依赖默认的表做）。
2. **写路径统一**：repository 层时间戳一律 `Utc::now().to_rfc3339()`（[media_task.rs:242](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L242) 已是此风格），不再手写 strftime SQL。删除 [agent.rs:164](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L164) 的 `updated_at = strftime(...)` SQL，改 bind `Utc::now().to_rfc3339()`。
3. **精度不一致可容忍**：同列毫秒 vs 纳秒混存是历史遗留，`DateTime<Utc>` 解析两者皆可（§3.3），不强求回填统一。

### 3.10 集群 J：dedup — `RuntimePresets` 单一构造器（R2）+ 未知状态统一（R4）

**R2 Remediation**：抽 `RuntimePresets::from_optional_fields(...)` 单一构造器，[command.rs:33-58](../../../crates/slab-app-core/src/infra/model_packs/command.rs#L33-L58) `build_runtime_presets`、[schemas/models.rs:657-669](../../../crates/slab-app-core/src/api/schemas/models.rs#L657-L669)（响应）、[:954-966](../../../crates/slab-app-core/src/api/schemas/models.rs#L954-L966)（请求）三处都调用它。加字段只改构造器一处。

```rust
// domain/models/（概念）
impl RuntimePresets {
    pub fn from_optional_fields(
        max_tokens: Option<u32>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        min_p: Option<f32>,
        presence_penalty: Option<f32>,
        repetition_penalty: Option<f32>,
    ) -> Option<Self> {
        (max_tokens.is_some() || temperature.is_some() || /* ... */)
            .then_some(Self { /* ... */ })
    }
}
```

**R3 处理**：审计 R3 的「`manifest_status`/`pack_status_from_unified` 4-arm 逆 twin」**在 2026-06-18 工作树不存在**（§1.4 纠错）。本规范**不为不存在的代码设计 remediation**。R3 的「状态映射单一 match」诉求由 ① R4（未知状态统一 Failed，§下文）+ ② model_pack Phase 0 的 `RuntimeModelStatus` 收口覆盖。本规范在 §6 复审清单登记 R3 为「审计描述漂移，现状无 twin」。

**R4 Remediation**：统一未知状态默认为 **`Failed`**（更安全）。[repository/agent.rs:13-22](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L13-L22) `parse_status` 的 `unwrap_or_else(... Pending)` 改 `Failed`，与 [domain/models/task.rs:45-54](../../../crates/slab-app-core/src/domain/models/task.rs#L45-L54) `from_stored` 一致。理由：损坏的 agent 线程被当 Pending 可恢复是危险的（agent 可能尝试 resume 一个状态不可信的线程）；当 Failed 显式暴露，由上层决定是否重试。`warn!` 保留。

**与 §3.6 CHECK 协同**：`agent_threads.status` 的 CHECK 值集确定后，未知值根本无法入库（DB 拒）——R4 的 fallback 只在「legacy 行 + CHECK 未覆盖历史值」时触发，是最后一道兜底。

### 3.11 集群 K：文档腐烂清理（D13）

逐条核实并清理（§1.4 已部分核实）：

| 项 | 核实 | 动作 |
|---|---|---|
| [entities/model.rs:15](../../../crates/slab-app-core/src/infra/db/entities/model.rs#L15) `models.provider` 注释 | `provider` 已 [DROP](../../../crates/slab-app-core/migrations/20260530000000_remove_models_provider.sql#L3)；需确认注释是否同步 | 删注释或更新 |
| [repository/task.rs:11-12](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L11-L12) `model_catalog` 注释 | [20240101000000_initial.sql:3](../../../crates/slab-app-core/migrations/20240101000000_initial.sql#L3) 已删旧表名 | 改 `models` |
| [entities/chat.rs:10](../../../crates/slab-app-core/src/infra/db/entities/chat.rs#L10) role 注释只列 user\|assistant\|system | CHECK 已扩 6 值（§3.6） | 更新注释为完整 6 值 |

**D11（chat_sessions.name 空串）** 与 **D12（软删不统一）**：审计评级 LOW。本规范**不**改 schema（`name NOT NULL DEFAULT ''` 改 nullable 影响大、软删统一是跨表大改）。仅在 §6 复审登记，留待未来迭代。

---

## 4. 数据迁移与回填策略（对齐 AGENTS.md §32 append-only）

所有 migration 具名 `YYYYMMDDHHMMSS_description.sql`，append-only，**不编辑既有 migration**。SQLite 重建表模式：

```sql
-- 通用模式（示意，具体列见各 Phase）
CREATE TABLE _new_table (... 新约束 ...);
INSERT INTO _new_table (col1, col2, ...) SELECT col1, col2, ... FROM old_table;  -- 回填/过滤
DROP TABLE old_table;
ALTER TABLE _new_table RENAME TO old_table;
-- 重建索引
```

### 4.1 NULL 回填先于 NOT NULL（P4 关键）

任何「加 NOT NULL」前，先在该 migration 内 `UPDATE ... SET col = default WHERE col IS NULL`。例（`model_downloads.source_key`，D2）：

```sql
-- 在重建 model_downloads 之前
UPDATE model_downloads SET source_key = 'auto::' || COALESCE(repo_id, 'unknown') || '::' || COALESCE(filename, 'unknown')
WHERE source_key IS NULL OR TRIM(source_key) = '';
-- 然后 CREATE TABLE _new (... source_key TEXT NOT NULL ...) + INSERT + DROP + RENAME
```

### 4.2 CHECK 加约束前的数据校验

加 CHECK 前扫描存量数据是否违反（migration 内 `SELECT CASE WHEN EXISTS (... 违反 ...) THEN RAISE(ABORT, '...') END`），或 migration 失败时人工修复后重跑。本规范每个新 CHECK 在 Phase 5 配「违反约束的写应被 DB 拒」测试（§6），并在 Phase 4 migration 内加数据校验 guard。

### 4.3 数据损失避免

- **media 子表 result_data 删除（§3.2）**：删列前 migration 内断言「子表 result_data 非空 ⇔ 主表 result_data 非空」（孤儿检测），否则先回填主表。
- **JSON 列加 `json_valid()`（§3.1）**：migration 内 `SELECT COUNT(*) FROM models WHERE NOT json_valid(spec)` → 若 >0，migration 失败并打印 bad row id（人工修复后再跑）。绝不静默丢坏行。
- **类型升级（§3.4 fps f64）**：DB 列已是 f64，实体升 f64 无数据损失；反方向（f64→f32）才有损失，本规范不做。

### 4.4 迁移不可逆性

append-only migration 一旦合并不可改。因此每个 migration 在 PR 评审时必须确认：① 回填逻辑覆盖所有 NULL/坏值路径；② CHECK 值集与 Rust enum 同步；③ 重建表的索引/触发器/外键完整重建。本规范 Phase 4 列出每个 migration 的完整 SQL 草案（实现时直接落地）。

---

## 5. 实施路线图（Phases）

> 每阶段：{涉及文件（相对链接，3 级 `../../../` 到 repo root）、关闭的审计发现、校验命令（[AGENTS.md:56-95](../../../AGENTS.md#L56-L95)）、退出标准}。迁移 append-only（§32）。**最窄校验命令优先**（AGENTS.md:18）。

### Phase 1 — JSON 列 `json_valid()` CHECK + re-serialize 整列写（substrate）

- **涉及文件**：
  - [repository/model.rs](../../../crates/slab-app-core/src/infra/db/repository/model.rs)：删 `json_set` 调用（[:163](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L163) 一带），改「读 ModelSpec → mutate → `serde_json::to_string` 整列写」。
  - 新 migration `crates/slab-app-core/migrations/20260620000001_json_valid_checks.sql`：`models.spec` 加 `CHECK (json_valid(spec))`；`agent_threads.config_json` 加 `CHECK (config_json IS NULL OR json_valid(config_json))`；`models.materialized_artifacts` 同理。重建表模式。migration 内加 `SELECT CASE WHEN EXISTS (SELECT 1 FROM models WHERE NOT json_valid(spec)) THEN RAISE(ABORT,'models.spec has invalid JSON') END` guard。
- **关闭**：**T1、D4**（仓库范围通用 substrate；model_pack Phase 4 是消费者）。
- **校验**：`bun run check:rust` → `bun run test:rust:cargo`，新增测试：`INSERT INTO models (..., spec) VALUES (..., 'not-json{')` 期望 `CHECK constraint failed`；round-trip 测试读写 `ModelSpec` 一致。
- **退出标准**：`grep json_set crates/slab-app-core/src/infra/db/repository/model.rs` 零命中；坏 JSON 写被 DB 拒。

### Phase 2 — `result_data` 单一真源 + 版本化解码注册表

- **涉及文件**：
  - 新 migration `20260620000002_drop_media_subtable_result_data.sql`：重建三个 media 子表去 `result_data` 列；孤儿检测 guard（§4.3）。
  - [media_task.rs](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs)：`update_*_result`（[:250-314](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L250-L314)）删 `result_data` 参数；view 查询（[:375-382](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L375-L382)）去子表 `result_data` SELECT；调用方改只更新主表。
  - [task.rs](../../../crates/slab-app-core/src/infra/db/repository/task.rs)：`decode_task_payload`（[:219-239](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L219-L239)）重构为注册表驱动；`encode_task_payload`（[:207-217](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L207-L217)）`.ok()` 加 warn（§3.5）。
- **关闭**：**D1、G4（encode_task_payload 部分）**。
- **校验**：`bun run test:rust:cargo`，新增测试：① `SELECT result_data FROM image_generation_tasks` 报「no such column」；② 主表信封 round-trip；③ 注册表无匹配 decoder 时 warn（不静默 None）。
- **退出标准**：media 子表无 `result_data` 列；主表 `tasks.result_data` 是唯一真源；版本化 decoder 可扩展。

### Phase 3 — FromRow NULL-safety + `try_from` + 时间戳

- **涉及文件**：
  - [repository/agent.rs](../../../crates/slab-app-core/src/infra/db/repository/agent.rs)：`AgentThreadRow.config_json` → `Option<String>`（[:33](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L33)）；`depth` → `u32::try_from` + warn/Err（[:55](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L55)）；`created_at`/`updated_at` → `DateTime<Utc>`（[:35-36](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L35-L36)）；`updated_at` SQL 改 bind `Utc::now().to_rfc3339()`（[:164](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L164)）。
  - [entities/model_download.rs](../../../crates/slab-app-core/src/infra/db/entities/model_download.rs)：`source_key` 保持 `String`（清理 + NOT NULL 后保证非空）。
  - 新 migration `20260620000003_source_key_not_null.sql`：回填 `model_downloads.source_key` NULL → 重建表加 NOT NULL（§4.1）。
  - [media_task.rs](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs)：`fps` 实体升 f64（[:427](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L427)）；`to_u32` 改 try_from + warn/Err（[:493-495](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L493-L495)）；`frames`（[:426](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L426)）同理。
  - [schemas/agent.rs](../../../crates/slab-app-core/src/api/schemas/agent.rs)：`AgentThreadResponse`（[:350-360](../../../crates/slab-app-core/src/api/schemas/agent.rs#L350-L360)）加 `config_json: Option<String>`。
- **关闭**：**T2、T3、D2、D6**。
- **校验**：`bun run gen:api`（刷 [v1.d.ts](../../../packages/api/src/v1.d.ts)）→ `bun run check:rust` → `bun run test:rust:cargo`。新增测试：① legacy NULL `config_json` 行不炸列表（返 None）；② `depth = -1` 行 warn + 拒绝/0；③ `fps` f64 高精度 round-trip；④ `AgentThreadResponse` 携带 config_json。
- **退出标准**：FromRow 对 NULL/越界健壮；时间戳统一 `DateTime<Utc>`；`source_key` NOT NULL。

### Phase 4 — CHECK 约束补齐

- **涉及文件**：
  - 新 migration `20260620000004_storage_check_constraints.sql`：§3.6 表中所有列加 CHECK（重建表模式）。值集与 Rust enum 同步（§3.6 契约）。每列配数据校验 guard。
  - [repository/agent.rs](../../../crates/slab-app-core/src/infra/db/repository/agent.rs)：`parse_status` unknown→`Failed`（R4，[:13-22](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L13-L22)）。
  - [schemas/validation.rs](../../../crates/slab-app-core/src/api/schemas/validation.rs)：若缺，加 `validate_thread_status`/`validate_plugin_runtime_status` 等（与 chat role 同纪律）。
- **关闭**：**D3、D7、R4**。
- **校验**：`bun run test:rust:cargo`，每个新 CHECK 配「违反约束的写应被 DB 拒」测试（§6.4）。如 `INSERT INTO plugin_states (..., enabled) VALUES (..., 2)` 期望失败。
- **退出标准**：所有枚举/布尔列有 CHECK；未知状态统一 Failed。

### Phase 5 — 跨表事务 + dedup + 文档清理

- **涉及文件**：
  - `domain/services/model/`（`update_model_config_selection` / `update_model_enancement`）：跨表写包 `pool.begin()` → `tx.commit()`（§3.7）。
  - `domain/models/`（`RuntimePresets`）：加 `from_optional_fields` 单一构造器（§3.10）。
  - [command.rs](../../../crates/slab-app-core/src/infra/model_packs/command.rs)、[schemas/models.rs](../../../crates/slab-app-core/src/api/schemas/models.rs)：三处组装点改调构造器（§3.10）。
  - [media_task.rs](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs)：artifact JSON `unwrap_or_else("[]")` 加 warn（[:257-258](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L258)）；`decode_string_array` 加 warn（[:489-491](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L489-L491)）。
  - [repository/agent.rs:66-77](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L66-L77)、[repository/chat.rs:58-65](../../../crates/slab-app-core/src/infra/db/repository/chat.rs#L58-L65)、[domain/models/chat.rs:230-236](../../../crates/slab-app-core/src/domain/models/chat.rs#L230-L236)：fallback 加 warn（T4）。
  - 文档清理（D13）：[entities/model.rs:15](../../../crates/slab-app-core/src/infra/db/entities/model.rs#L15)、[repository/task.rs:11-12](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L11-L12)、[entities/chat.rs:10](../../../crates/slab-app-core/src/infra/db/entities/chat.rs#L10)。
  - 新 migration `20260620000005_tasks_core_task_id_unique.sql`：partial unique index（D10）。
- **关闭**：**D5、D10、D13、R2、T4、G4（media artifact / decode_string_array 部分）**。
- **校验**：`bun run check:rust` → `bun run test:rust:cargo`。新增测试：① 跨表 PUT 中途失败回滚；② `RuntimePresets` 加字段只改一处；③ fallback 分支 warn 可观测。
- **退出标准**：跨表写原子；RuntimePresets 单源；fallback 可诊断；注释无腐烂。

### Phase 6 — D8 命名注释治理 + 复审

- **涉及文件**：实体注释（`models.kind` 标「deployment type」、各 `status` 标语义、`source` 三义消歧）。
- **关闭**：**D8**（文档级）；D11/D12 登记留待未来。
- **校验**：`bun run check:rust`；基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) 出复审文档，确认 T1–T4、D1–D10、D13、R2、R4、G4（data-path）关闭且 R3 标记为「审计漂移，现状无 twin」。
- **退出标准**：注释消歧；复审显示本规范范围内发现全部关闭。

### 5.1 验证策略（对齐审计 §6.4）

- **Phase 1/2（P0-2/P0-5）**：迁移加 CHECK 后补 round-trip 测试——坏 JSON 写入应失败而非砖读（§6.4 原文）。
- **Phase 4（P1-5）**：每个新 CHECK 配「违反约束的写应被 DB 拒」测试（§6.4 原文）。
- **Phase 3（P0-7）**：legacy NULL 行不炸列表的 red-test（驱动 Option 化）。
- **整体**：每 Phase 用最窄校验命令（AGENTS.md:18），先 `check:rust` 再 `test:rust:cargo`，最后才 workspace。

---

## 6. 验证策略（汇总）

### 6.1 Round-trip 测试

- **JSON 列**：写一个 `ModelSpec` → 存 → 读 → 断言等价。写一个非法 JSON 串 → 断言 DB 拒（`CHECK constraint failed`）。
- **`result_data` 信封**：写 v1 信封 → 存 → 读 → 断言 decoder 匹配。写一个未来 kind/version → 断言 warn + None（不静默）。
- **时间戳**：写 `DateTime<Utc>` → 存 → 读 → 断言等价（毫秒/纳秒混存可解析）。

### 6.2 「违反约束的写应被 DB 拒」测试（审计 §6.4）

每个新 CHECK 一条：

| 列 | 违反写 | 期望 |
|---|---|---|
| `models.spec` | `INSERT ... VALUES (..., 'not-json{')` | `CHECK constraint failed: json_valid` |
| `plugin_states.enabled` | `INSERT ... VALUES (..., 2)` | `CHECK constraint failed` |
| `agent_threads.status` | `INSERT ... VALUES (..., 'bogus')` | `CHECK constraint failed` |
| `audio_transcription_tasks.detect_language` | `INSERT ... VALUES (..., 5)` | `CHECK constraint failed` |
| `image_generation_tasks.width` | `INSERT ... VALUES (..., -1)` | `CHECK constraint failed` |
| `model_downloads.source_key`（NOT NULL 后） | `INSERT ... VALUES (..., NULL)` | `NOT NULL constraint failed` |
| `tasks.core_task_id`（unique 后） | 两条同值 | `UNIQUE constraint failed` |

### 6.3 NULL/越界 FromRow 测试

- `agent_threads` 插入 `config_json = NULL` 行 → `list_session_threads` 不 panic，返 `config_json: None`。
- `agent_threads` 插入 `depth = -1` 行 → 解码 warn + 拒绝（或显式 0，视 §3.4 决策）。
- `model_downloads` legacy NULL `source_key` → migration 回填后无 NULL（`SELECT COUNT(*) WHERE source_key IS NULL` = 0）。

### 6.4 fallback 可观测测试

- `agent_thread_messages` 插入畸形 `content` JSON → `into_record` fallback 时 warn 日志可断言（`tracing::mock` 或日志捕获）；`tool_calls` 为空但 `content` 含原始串。
- `tasks.result_data` encode 失败 → warn 日志可断言。

### 6.5 跨表事务测试

- `update_model_config_selection` 第二表写人为失败 → 第一表回滚（`SELECT` 验证未变更）。

---

## 附录 A：审计发现 → 计划条款 闭环追溯

| 审计发现（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md)） | 本规范机制 | 实施阶段 |
|---|---|---|
| **T1** `json_set` 绕过 serde（[model.rs:163](../../../crates/slab-app-core/src/infra/db/repository/model.rs#L163)） | re-serialize 整列写 + `json_valid()` CHECK（§3.1） | Phase 1 |
| **T2** fps f64→f32 + i64→u32 截断（[media_task.rs:427](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L427)/[:493-495](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L493-L495)） | 实体升 f64 + `try_from` 拒绝越界（§3.4） | Phase 3 |
| **T3** `config_json` 非 Option + `depth as u32`（[agent.rs:33](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L33)/[:55](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L55)） | Option 化 + try_from + `DateTime<Utc>`（§3.3）。**注**：审计「列无 NOT NULL」已 §1.4 纠错（实有 NOT NULL DEFAULT），但 Option 化仍是正确防御 | Phase 3 |
| **T4** 内容解析 fallback 破坏性丢 tool_calls（[agent.rs:66-77](../../../crates/slab-app-core/src/infra/db/repository/agent.rs#L66-L77)） | fallback 加 warn + 旁路 raw（§3.5） | Phase 5 |
| **D1** `result_data` 信封只保护主表 + 子表双写 | 主表单一真源 + 子表删列 + 版本化 decoder（§3.2） | Phase 2 |
| **D2** `source_key` 可空但实体非可选 | NULL 回填 + NOT NULL（§3.3/§4.1） | Phase 3 |
| **D3** 多 TEXT 状态/角色列缺 CHECK | per-column CHECK（§3.6） | Phase 4 |
| **D4** `models.spec` 无 `json_valid()` CHECK | 加 CHECK（§3.1） | Phase 1 |
| **D5** 选择状态跨两表无事务 | 服务层事务包裹（§3.7） | Phase 5 |
| **D6** `config_json` 持久化但 API 不返回 | `AgentThreadResponse` 暴露（§3.3） | Phase 3 |
| **D7** 布尔 INTEGER 无 CHECK | `CHECK (col IN (0,1))`（§3.6） | Phase 4 |
| **D8** kind/status/source 命名过载 | 注释消歧（§3.8；DB 侧不改列名） | Phase 6 |
| **D9** 时间戳 TEXT 不一致 | 老表加默认 + 写路径统一 `to_rfc3339()`（§3.9） | Phase 3 |
| **D10** `core_task_id` 无 UNIQUE | partial unique index（§3.3） | Phase 5 |
| **D11** chat name 空串不可区分 | **登记留待未来**（§3.11，不改 schema） | 不在本迭代 |
| **D12** 跨表删除语义不统一 | **登记留待未来**（§3.11） | 不在本迭代 |
| **D13** 文档腐烂注释 | 逐条清理（§3.11） | Phase 5 |
| **R2** `RuntimePresets` 多处组装（§1.4 纠错：3 处非 4） | `from_optional_fields` 单一构造器（§3.10） | Phase 5 |
| **R3** `manifest_status`/`pack_status_from_unified` 逆 twin | **审计漂移**（§1.4）：2026-06-18 工作树无此 twin；不为不存在代码设计。诉求由 R4 + model_pack Phase 0 覆盖 | N/A（登记） |
| **R4** TaskStatus unknown→Failed vs agent→Pending | 统一 Failed（§3.10） | Phase 4 |
| **G4**（data-path）`encode_task_payload` 吞错（[task.rs:207-217](../../../crates/slab-app-core/src/infra/db/repository/task.rs#L207-L217)） | `.ok()` 加 warn（§3.5） | Phase 2 |
| **G4**（data-path）media artifact JSON 畸形（[media_task.rs:257-258](../../../crates/slab-app-core/src/infra/db/repository/media_task.rs#L257-L258)，§1.4 行号纠错） | `unwrap_or_else("[]")` 加 warn（§3.5） | Phase 5 |
| **G4**（config-path）`parse_env` | **属 PMID 计划**（§1.3 边界） | 不在本规范 |
| **P0-2** json_set re-serialize + json_valid CHECK | §3.1 | Phase 1 |
| **P0-5** result_data 单一真源 + 版本化解码器 | §3.2 | Phase 2 |
| **P0-7** config_json Option + depth try_from | §3.3 | Phase 3 |
| **P1-5** 补 CHECK 约束 | §3.6 | Phase 4 |
| **P1-6** 统一未知状态默认 Failed | §3.10（R4） | Phase 4 |
| **P1-8** RuntimePresets 单一构造器（manifest_status 单一 match 部分） | §3.10（R2；R3 部分审计漂移） | Phase 5 |
| **P2-1** 数据路径 `.ok()`/`unwrap_or_default()` 加 warn | §3.5 | Phase 2/5 |
| **P2-5** 命名治理 | §3.8 | Phase 6 |
| **P2-6** 文档腐烂清理 | §3.11 | Phase 5 |
| **P2-7** config_json 暴露或停写（agent_threads 部分） | §3.3（暴露） | Phase 3 |

---

## 附录 B：与既有契约的边界（对齐 [AGENTS.md](../../../AGENTS.md)）

- **推理链路边界不变**（AGENTS.md §Architecture Boundaries）：`bin/slab-app → bin/slab-server → slab-app-core → GrpcGateway → bin/slab-runtime → slab-runtime-core`。本规范只改 `crates/slab-app-core` 的 repository/entity/schema/domain service 层与 migrations，**不动** `slab-runtime-core`、不动 gRPC 协议、不动 inference backend。
- **SQLx migrations append-only**（AGENTS.md §32）：所有 schema 改动走具名新 migration（§4），**不编辑** [crates/slab-app-core/migrations/](../../../crates/slab-app-core/migrations/) 既有文件。重建表模式与 [20260608000000_add_storage_value_checks.sql](../../../crates/slab-app-core/migrations/20260608000000_add_storage_value_checks.sql) 既有手法一致。
- **API shape 变更走 `bun run gen:api`**（AGENTS.md §Architecture Boundaries / §56-95）：Phase 3 给 `AgentThreadResponse` 加 `config_json` 字段后，必须 `bun run gen:api` 刷 [packages/api/src/v1.d.ts](../../../packages/api/src/v1.d.ts)。
- **跨 crate 契约走 slab-types/slab-proto**：本规范不改跨 crate 类型（`ModelSpec` 等已在 `slab-types`/slab-app-core domain），无新跨 crate 契约。
- **与 model_pack 计划的衔接**（[slab-model-pack-2026-06-17.md](slab-model-pack-2026-06-17.md)）：本规范 Phase 1 提供 `json_valid()` CHECK + re-serialize 的通用 substrate；model_pack Phase 4 的 `model_config_state.selected_engine` 是该 substrate 的消费者（落 selected_engine 列时复用本规范的 re-serialize 模式 + 已有的 `json_valid()` CHECK 体系）。两者不重复——本规范是基础设施，model_pack 是特定列应用。model_pack 计划负责的 F1/F2/F3/F4/F5/F7/F8/F10/F-Stack-3 不在本规范范围。
- **与 PMID 计划的边界**：`parse_env` 静默回退（G4 config-path、PMID-F7）、env→PMID 双源、5 层 logging override、`telemetry.metrics_exporter` 类型 laundering 属 PMID 治理计划，本规范不触及。
- **与 reliability/safety 计划的边界**：路径包含校验统一（R1）、gRPC 错误跨边界保结构（F-Stack-3）、JSON-RPC 宿主去重（R5）、pending-map 超时（G1 RPC）、process_supervisor Drop（G3）属 reliability/safety 计划，本规范不触及。
- **最窄校验命令优先**（AGENTS.md:18）：每 Phase 先 `bun run check:rust` 再 `bun run test:rust:cargo`，避免无谓 workspace 全量。

---

*本规范由首席架构师主导，所有 `path:line` 证据在 2026-06-18 工作树重新核实。审计（2026-06-17）中 T3「列无 NOT NULL」、R2「4 处组装」、R3「逆 twin」、G4「:490 artifact JSON」经核实已漂移或部分推翻，记录于 §1.4 纠错表（与审计 §1.3 同纪律）。规范以 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §2.2 T1–T4、§4 D1–D13、§5 R2/R4 与 G4 data-path 部分、§6 P0-2/P0-5/P0-7/P1-5/P1-6/P1-8/P2-1/P2-5/P2-6/P2-7 为闭环目标；model_pack 配置、PMID 回显、跨进程可靠性属 sibling 计划。*
