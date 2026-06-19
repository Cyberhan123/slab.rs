# 全栈代码审计报告 (2026-06-17)

> **审计方法**：多智能体协作（6 路并行深度审计，分 2 批 × 3 路以规避账号并发限流）覆盖 `model_pack` 配置体系、PMID 回显引擎、API↔DB 契约、DTO↔Entity↔DB 转换链、跨模块冗余与逻辑死角、全栈数据流拓扑六大维度。所有 High 级发现由主审计员**直接读源码落地核实**，被核实推翻或已修复的发现已剔除/降级（见 §1.3 的纠错记录）。所有结论均要求 `path:line` 证据。
>
> **审计范围**：生产代码（Rust workspace ~42 crate + TS 前端 `packages/slab-desktop`）。不含测试脚手架（已由 2026-06-17《Test Capability Audit》覆盖）。

---

## 0. 2026-06-18 PMID/settings 专项闭环

`docs/development/planning/slab-pmid-settings-2026-06-17.md` 已按当前代码实施，关闭范围仅限 PMID/settings 专项：

- 已关闭：PMID-F1–F11、F-Stack-1、F-Stack-2、G4-config 中 parse_env/setting_value_from 部分、G5、P1-1、P1-2、P1-3、P1-4、P2-3。
- 关键代码落点：`crates/slab-config` 回显类型/脱敏/约束/env seed，`crates/slab-app-core` 设置变更语义，`bin/slab-server` PMID admin 鉴权投影，`packages/api` token 注入与 401 诊断，`packages/slab-desktop/src/pages/settings` 生效/继承徽标。
- 验证记录：`cargo test -p slab-config`、`cargo test -p slab-app-core settings`、`cargo test -p slab-server settings`、`bun run test:frontend -- packages/slab-desktop/src/pages/settings packages/api`、`bun run gen:api`。
- 非本轮范围：model_pack、DB 契约、JSON-RPC/gRPC 错误结构、路径安全治理仍按各自专项处理，本闭环不改变这些条目的状态。


## 1. 审计综述（整体设计质量与健康度）

### 1.1 总体评级：**B−（弱 B，倾向 C+）**

`slab.rs` 是一个工程完成度较高的本地 AI 模型运行时 / 桌面应用（Tauri + slab-server HTTP 网关 + slab-app-core 业务核 + slab-runtime 推理 worker + 多语言插件 sidecar）。**架构分层清晰、边界纪律良好**（见 AGENTS.md 的 inference/plugin/LSP 边界，且经本次核实基本落地），**前后端契约由 openapi 代码生成驱动**（前端从 `packages/api/src/v1.d.ts` 的 `paths[...]` 派生类型而非手写，见 §2.1），**敏感设置脱敏已正确实现**（§1.3 纠错）。这些都是显著加分项。

但本次审计暴露出**三类系统性短板**，将整体健康度拉低：

1. **`model_pack` 配置体系的字段语义存在结构性模糊**（§3.1）：引用字段 `$config` 与 `$load_config`/`$inference_config` 双词汇表、`kind`+`scope` 双标记、`variant.id` 无唯一性约束、已发布的 JSON Schema 只覆盖 manifest 不覆盖子文档——这是本仓库最大的"配置即契约"风险面。
2. **数据持久层有多处"无 CHECK 约束 + 静默强转"组合**（§4）：多个状态/角色 TEXT 列、布尔 INTEGER 列、JSON 列缺少 CHECK 与 `json_valid()` 校验，配合 repository 层的 `.unwrap_or_default()` / `as f32`，形成"坏数据静默存活 → 读路径整表失败"的链式风险。
3. **PMID 回显与运行时配置存在"环境变量 / PMID 双源"陷阱**（§3.2）：`SLAB_ADMIN_TOKEN`（env）与 `server.admin.token`（PMID）服务于同一语义却互不相通，且设置热重载范围过窄（仅 agent），构成可观测的"改了不生效"陷阱。

### 1.2 三大首要风险（各一句）

1. **`model_pack` 的 `variant.id` 可重复且被静默 last-wins 覆盖**，已发布的两个 llama pack 各自带一个重复 `Q4_K_M`（§3.1 F1）——schema 不校验、解析器不报错、被丢弃的变体无声消失。
2. **`tasks.result_data` 信封化只保护了 `tasks` 表，三个 media 子表 `result_data` 既未信封化又与主表重复存储**（§4.4），双写双读、单一真源不明。
3. **路径包含校验存在 3 套实现且语义不一致**（§5.1 F1），其中一套对 `path` 不做 canonicalize 即 `starts_with`，是典型的越界绕过形态（安全相关）。

### 1.3 纠错记录（对抗式核实推翻的初始假设）

| 初始假设（来源） | 核实结论 | 处理 |
|------|----------|------|
| PMID `secret` 标记是"纯装饰"，`server.admin.token` 明文回显；provider/websearch api_key 不脱敏（子代理 A2 3.1/3.2） | **推翻**。当前代码 `pmid_service.rs:971` 的 `secret()` 已覆盖 `server.admin.token` + `providers.registry` + `agent.tools.websearch.providers` 三处；`redact_setting_value`（:977）对 leaf 与数组/对象内的 `api_key` 字段均正确脱敏；并有测试 `secret_setting_views_redact_literal_secret_values`（:1714）。**脱敏已正确实现。** | 降级为 LOW（§3.2 F9：脱敏白名单硬编码、未由 schema 的 `writeOnly` 驱动；`agent.tools.mcp.servers` 未纳入但其 env 为变量名引用非明文） |
| 环境变量与 PMID 双源断开（子代理 A2 4.1） | **确认**。`auth_middleware` 读 `config.admin_api_token`（`auth.rs:23`），其值来自 `SLAB_ADMIN_TOKEN` env（`app_config.rs:156`），与 `server.admin.token` PMID 完全不相通。 | 保留为 HIGH（§3.2 F1） |
| `chat_template` 裸字符串 `"chatml"` 在运行期崩溃（子代理 A1 R2） | **确认字段形态不一致 + 懒校验**。`chat_template` 类型为 `TemplateAssetRef`（对象），`parse_optional_asset_ref`（`runtime_bridge.rs:463`）对非法形态返回 `InvalidBackendConfigAssetRef`；且有显式 legacy 字段拒绝（`reject_legacy_llama_load_fields` :384/:537）。所有测试仅用对象形态（:601），Qwen2.5-0.5B 的裸字符串形态与之冲突，且仅运行期才校验（非导入期）。 | 保留为 HIGH（§3.1 F2） |

### 1.4 健康度矩阵

| 维度 | 评级 | 关键依据 |
|------|------|----------|
| 架构分层与边界纪律 | **A−** | inference/plugin/LSP 边界落地；前端契约代码生成（§2.1）；Tauri vs HTTP 双通道无重叠 |
| `model_pack` 配置契约 | **C** | 字段双词汇表、id 无唯一性、schema 覆盖不全（§3.1） |
| PMID 回显引擎 | **B−** | 脱敏正确；但 env/PMID 双源、热重载过窄、metrics_exporter 类型 laundering（§3.2） |
| API ↔ DB 契约一致性 | **C+** | 类型基本对齐；但多列缺 CHECK、JSON/布尔静默强转（§4） |
| 数据转换鲁棒性 | **C** | json_set 绕过校验、多处 `unwrap_or_default` 吞坏数据、转换逻辑 4 处重复（§2.2/§5） |
| 跨进程可靠性（JSON-RPC/gRPC） | **C+** | pending-map 无界泄漏、gRPC 错误跨边界损失结构（§2.3/§5.3） |
| 冗余控制 | **C** | 3 套路径校验、js/python JSON-RPC 95% 重复、转换镜像对（§5） |

---

## 2. 全栈业务逻辑与数据转换链路分析

### 2.1 数据流拓扑（已核实的代表性流）

```
┌──────── Tauri 桌面宿主 (bin/slab-app/src-tauri) ────────────┐
│  spawn slab-server (loopback 127.0.0.1:3000)                 │
│  仅 3 个 host-only invoke(): plugin WebView 生命周期          │
└───────────────────────┬─────────────────────────────────────┘
                        │ HTTP /v1/* （前端不注入任何 Auth 头）
┌───────────────────────┴─────────────────────────────────────┐
│ Frontend (packages/slab-desktop) openapi-fetch + react-query │
│ 类型全部从 packages/api/src/v1.d.ts 的 paths[...] 派生 ✅     │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│ slab-server /v1  handler.rs → AppCoreError → HTTP (error.rs) │
│ auth_middleware: loopback bind 时免 token 放行                │
└───────────────────────┬─────────────────────────────────────┘
                        │ service.* (slab_app_core::domain)
┌───────────────────────▼─────────────────────────────────────┐
│ slab-app-core domain services                                │
│ ModelService(catalog/runtime/config_document)                │
│ ChatService(mod → local.rs | cloud.rs)                       │
│ AgentService / PluginService / SettingsService                │
└───┬──────────┬──────────┬──────────────┬────────────────────┘
    │ SQLite   │ SQLite   │ SQLite       │ gRPC
    ▼          ▼          ▼              ▼
  models/   chat_*     plugin_states/  GrpcGateway ──► slab-runtime
  model_    agent_*    settings doc    (rpc/runtime_   (slab-runtime-core)
  config_                              gateway.rs)
  state
```

**关键流路径（已核实）：**

- **模型选择**：`PUT /v1/models/{id}/config-selection`（`models/handler.rs:230`）→ `UpdateModelConfigSelectionRequest{selected_preset_id?, selected_variant_id?}` → `ModelService::update_model_config_selection`（`model/catalog.rs:72`）→ 持久化到 `models` + `model_config_state`。**不会自动加载运行时**。加载是独立第二步 `POST /v1/models/load`（`handler.rs:318`）→ `resolve_model_load_target` 重新从 DB 读选择 → gRPC `load_model`（`rpc/client.rs:371`）。
- **聊天**：`POST /v1/chat/completions`（`chat/handler.rs:113`）→ `cloud::should_route_to_cloud`（`chat/cloud.rs:82`）分叉 cloud（genai HTTP）/ local（`chat/local.rs` → runtime gRPC）；消息落 `chat_sessions`/`chat_messages`。
- **Agent**：前端运行期三选一传输（`use-assistant-agent.ts:78` 的 ref `'none'|'sse'|'websocket'`，分支 `:444`/`:513`），对应 `GET /v1/agents/responses`（WS）、`POST`、SSE 三路由；服务端消息形态统一（`AgentResponsesServerMessage`）。
- **设置回显**：`GET/PUT /v1/settings[/{pmid}]` → `PmidService` → 设置文档 / DB。前端类型从 `paths['/v1/settings']` 派生（`pages/settings/types.ts:3-11`），契约干净。

### 2.2 数据转换链路缺陷（DTO ↔ Entity ↔ DB row）

> 详细证据见 §4（DB 侧）、§5（冗余）。此处聚焦"转换链路本身"的风险。

**T1. [HIGH] `json_set` 原地改写 `models.spec` 绕过类型校验——单次坏写可砖掉整模型读取**
- `infra/db/repository/model.rs:153-181` 用 SQLite `json_set(spec, '$.local_path', ?1)` 原地改 JSON 列，**完全绕过 `ModelSpec` serde**。`models` 表无 `json_valid(spec)` CHECK（`20260608000000_add_storage_value_checks.sql:44-61` 仅校验 status/kind）。一次并发或 buggy 更新可让 `spec` 进入 `parse_json_field`（`model.rs:65-71`）拒绝的形态，导致 `get_model`/`list_models` 对该行全部返回 `Store` 错误。
- 修复：在 Rust 端 re-serialize 整个 `ModelSpec` 再写整列；或加 `json_valid()` CHECK。

**T2. [HIGH] 视频 `fps: f64 → f32` 与 `i64 → u32` 静默截断隐藏坏行**
- `infra/db/repository/media_task.rs:427` `fps: row.fps as f32`（行字段 f64，实体 f32）——高帧率如 59.940059… 被舍入，渲染与请求不一致。
- `:394-396,424-426,493-495` `to_u32()` 即 `value.try_into().unwrap_or_default()`：负数或 >u32::MAX 的 width/height/frames/requested_count **静默变 0**，坏行以"0×0 任务"形态存活而非暴露数据完整性错误。
- 修复：实体字段升 f64；越界 log + 拒绝而非 `unwrap_or_default()`。

**T3. [HIGH] `AgentThreadRow.config_json: String` 非可选，但列可空——单行 NULL 炸掉整列表**
- `infra/db/repository/agent.rs:33-34` 声明 `config_json: String`（非 Option），但 SQL 列无 NOT NULL 保证。写入路径（`:116`）绑定 `snapshot.config_json`（内存总存在），但任何经手工/旧迁移/部分写入产生的 NULL 行，会让 `FromRow` 解码失败，`get_thread`/`list_session_threads` 对**整个列表**返回 `Store` 错误而非逐行跳过。
- 同行 `depth: i64 as u32`（`:55`）负值静默回绕成巨大 u32；`created_at`/`updated_at: String`（`:35-36`）无解析校验，畸形时间戳在此层蒙混过关、下游才崩。
- 修复：`config_json: Option<String>` + 读处 `unwrap_or_default()`；`depth: u32` 配 `try_from`；时间戳用 `DateTime<Utc>`。

**T4. [MED] `AgentThreadMessageRow` 内容解析 fallback 破坏性丢失 tool_calls**
- `infra/db/repository/agent.rs:66-77`：`serde_json::from_str::<ConversationMessage>(&self.content).unwrap_or_else(|_| ConversationMessage{ role, content: Text(self.content), … })`。存储 JSON 畸形时，`tool_calls`/`name`/`tool_call_id` 全部清空，原始 JSON 串塞进 `Text`。对带工具调用的 assistant 消息，这是**破坏性、静默、无日志**的数据损失。
- 同型：`ChatMessage.content`（`repository/chat.rs:58-65`）畸形 JSON 静默 fallback 到 `Text`（`domain/models/chat.rs:230-236`）；`media_task.rs:489-491` `decode_string_array` 静默返回 `[]`。
- 修复：fallback 分支至少 `warn!` 并保留 raw 到旁路字段。

### 2.3 全栈跨边界缺陷（gRPC / 热重载 / 认证交接）

**F-Stack-1. [HIGH] 设置热重载范围仅限 `agent.hooks.*` / `agent.memories.*`——其余设置改了不生效且无 UI 信号**
- `domain/services/settings.rs:73-75`：`affects_agent_runtime(pmid)` 仅对这两个前缀返回 true，触发 `agent_runtime.reload()`。**runtime/inference/model/chat/diffusion/server 等设置持久化到 DB 但无任何 live reload**——运行中的 slab-runtime 与推理 backend 不会被通知。
- `sync_runtime_restart_states()` 在 `backend.rs:22`、`model/runtime.rs:67` 是**按需拉取**（查状态时）而非设置变更时推送。用户改了 workers/context_length/device，必须手动重载模型或重启 runtime，UI 无任何提示。
- 修复：扩宽 `affects_*` 或引入"needs-restart"信号回显到设置视图。

**F-Stack-2. [HIGH] 认证 100% 依赖 loopback 旁路，前端无 token 注入路径**
- `packages/api/src/index.ts:21-37` `createSlabApiFetchClient` 只设 `baseUrl`+`fetch`，**全仓库无 Authorization 拦截器**。`auth_middleware` 在 `admin_api_token` 未配置且 bind 为 loopback 时放行（`middleware/auth.rs:42` → `is_loopback_bind_address`，`:61-73`）。
- 陷阱：若用户/配置设 `bind_address=0.0.0.0` 而未设 `admin_api_token`，前端因无 token 可发 → API 返回 401 → 桌面应用"看似启动但全部接口不通"，且前端无任何检测或引导。
- 修复：前端在 401 时给出明确的"需要配置 admin token 或回退 loopback"指引；或让设置页能写入 token 并让前端读取注入。

**F-Stack-3. [MED] gRPC 错误跨 HTTP 边界逐级损失结构**
- `RuntimeMemoryPressure` 被重映射为 `ServerError::BackendNotReady`（`error.rs:294-296`），把 OOM 类故障与"还在加载"合并；网关在 `rpc/runtime_gateway.rs:225` 发射它。
- 结构化 `CoreError` 变体（`QueueFull`/`Busy`/`BackendShutdown`/`UnsupportedOperation`/`DriverNotRegistered`）在 `error.rs:212-251` 被压平为固定人类可读字符串，仅 i18n key 区分少数几个；原始变体 detail 只进日志、不发往客户端。前端拿到 500 + "inference backend error"，无字段可分支。
- `runtime_gateway.rs:216-229` 的 `map_runtime_error` fallback 对未识别 tonic status 一律 `AppCoreError::Internal("grpc {action} failed: {error:#}")` → HTTP 500，仅 `action` 字符串进日志。
- 修复：在错误信封里保留机器可读 `code`，与 agent 协议层的 `Error{code,message,i18n}`（`agent/handler.rs:310`）统一。

**F-Stack-4. [MED] 模型选择 → 加载是两步隐式客户端驱动，服务端不强制顺序**
- `PUT .../config-selection` 持久化选择并返回 `UnifiedModel`（不加载）；随后 `POST /v1/models/load` 重新 `resolve_model_load_target` 读选择。若前端在 config-selection 完成前调 load，或 `same_model_download_source` 把 status 翻成 `NotDownloaded`（`catalog.rs:98-100`），load 以路径/校验错误失败。顺序契约是隐式的。
- 修复：服务端对"先 load 后 select"显式拒绝或排队。

---

## 3. `model_pack` 与 PMID 回显专项审计

### 3.1 `model_pack` 配置体系（字段模糊、层级混乱定位）

**F1. [HIGH] `variant.id` 可重复——schema 不校验、解析器 last-wins 静默丢弃，已发布 pack 带病**
- 证据：`models/llama/Qwen2.5-0.5B-Instruct/manifest.json:49-58` 列了**两个** `"id": "Q4_K_M"` 变体（均 ref `Q4_K_M.json`）；`models/llama/Qwen3.5-9B/manifest.json:72,77` 同样重复 `Q4_K_M`（`grep '"id"'` 两处命中）。
- 解析行为：`crates/slab-model-pack/src/resolve.rs:147` `resolved.insert(entry.id.clone(), …)` 入 `BTreeMap`，**last-wins 静默覆盖**，无错误无告警。
- schema：`docs/public/manifests/v1/slab-manifest.schema.json:469-474` `variants` 为普通数组，**无 `id` 唯一性约束**；Rust `ConfigEntryRef`（`manifest.rs:96-104`）也无检查；`pack.rs:197-258` 的 `validate_manifest_references` 校验 ref 目标，**从不校验数组内 id 唯一**。
- 修复：`validate_manifest_references` 增加变体/预设/组件/适配器数组内 id 唯一性检查（新增 `DuplicateEntryId` 错误），并删除两个数据文件里的重复 `Q4_K_M` 块。

**F2. [HIGH] `chat_template` 字段双形态——Qwen2.5-0.5B 用裸字符串 `"chatml"`，其余用对象 asset-ref，仅运行期才报错**
- 证据：`models/llama/Qwen2.5-0.5B-Instruct/configs/load.json:8` `"chat_template": "chatml"`（裸字符串）；`models/llama/Qwen3.5-9B/configs/load.json:8-11` 为对象形态 `{id,name,$path:"ref://...jinja"}`；所有测试 fixture（`runtime_bridge.rs:601-610`）只用对象形态。
- 解析：`chat_template` 类型为 `TemplateAssetRef`，经 `parse_optional_asset_ref`（`runtime_bridge.rs:463`）反序列化，非法形态返回 `InvalidBackendConfigAssetRef`（`:476/:483/:502`）；并有显式 legacy 拒绝 `reject_legacy_llama_load_fields`（`:384`/`:537-538`，注释明确 "legacy runtime chat-template application was removed; configure 'chat_template' asset refs instead"）。
- **根因是懒校验**：`validate_manifest_references`（`pack.rs:197-258`）只校验 backend_config 的 **scope**，从不校验 payload 形态。故 Qwen2.5-0.5B pack 能成功 `from_bytes`/`resolve()` 并入库展示，直到 `compile_default_runtime_bridge()`（服务端 `mod.rs:103-107` 调用）才以困惑的 asset-ref 错误失败。
- 修复：(a) 把 `Qwen2.5-0.5B-Instruct/configs/load.json:8` 改为对象形态并补 `.jinja` 资产；(b) 把 asset-ref 形态校验前移到 `from_bytes`。

**F3. [HIGH] 已发布 JSON Schema 只覆盖 manifest，不覆盖 variant/preset/backend_config/component/adapter 子文档**
- 证据：`schema.rs:10-36` 仅 `schema_for!(ModelPackManifest)`；checked-in `slab-manifest.schema.json`（489 行）的 `$defs` 全是 manifest 级（`ConfigEntryRef`/`PresetEntryRef`/`PackSourceCandidate`），**没有** `VariantDocument`/`PresetDocument`/`BackendConfigDocument`/`ComponentDocument`/`AdapterDocument` 的 `$defs`。
- 但 `models/**/variants/*.json`、`presets/*.json`、`configs/*.json` **每个文件都带** `"$schema": ".../slab-manifest.schema.json"`——指向一个无法校验它们形态的 schema。Rust 端的 `deny_unknown_fields`（`manifest.rs:435/:484/:521`）仅在反序列化期生效，作者期 / CI 期无保护。
- 修复：为每类文档生成并发布独立 `$defs`（或单一多文档 schema），重指 `$schema`；扩展 `schema.rs:69-74` 的对照测试。

**F4. [MED] 引用字段双词汇表 `$config` vs `$load_config`/`$inference_config`**
- manifest 级 `ConfigEntryRef`/`PresetEntryRef` 用 `$config`（`manifest.rs:102/:114`）；解析后文档内 `VariantDocument`/`PresetDocument` 用 `$load_config`+`$inference_config`（`:450-453/:494-497`）。
- 判定：这是**两层**而非语义分裂——`$config` 是"entry→document"指针（由 `PackDocument` kind tag 派发，`pack.rs:113-195`），`$load_config`/`$inference_config` 是"document→backend_config"指针并附带 scope 期望（`pack.rs:173-195` 的 `resolve_backend_config` 校验 `scope != expected_scope` → `UnexpectedBackendConfigScope`）。Rust 类型确实区分。
- 但**命名混淆**：一个 `$config`（指向 preset）与一个 `$load_config`（指向 backend_config）看起来同族却意义不同，是可读性 footgun。
- 修复（cosmetic）：entry 级字段改 `$document`/`$ref` 以避撞名。

**F5. [MED] `variant_id` 可来自三处，静默覆盖优先级**
- `variant_id` 可出现于 (a) manifest `PresetEntryRef`（`manifest.rs:112-113`，alias `"variant"`）、(b) 解析后 `PresetDocument`（`:489`）、(c) 用作 source file id 匹配（`resolve.rs:247-263`）。`resolve.rs:170-173`：preset 文档无 `variant_id` 时从 manifest entry 回填——**preset-doc 的 `variant_id` 胜出，manifest-entry 是 fallback**，无差异告警。配合 F1 的静默去重，变体解析脆弱。
- 修复：两者并存且不同时 warn/error。

**F6. [MED] `kind` + `scope` 双标记，`scope` 才驱动行为，`kind` 仅结构派发**
- 每个 backend_config 同时带 `kind:"backend_config"`（`manifest.rs:520-532`）和 `scope:"load"/"inference"`（`:504-509`）。`kind` 是 serde tag 派发（`:399`）；`scope` 是唯一驱动运行期行为的字段（`resolve_backend_config` `pack.rs:180-186` 拒绝 scope 不匹配）。
- 判定：**不冗余**（正交用途），但**对作者歧义**——`kind` 已暗示 backend_config，`scope` 看似可推导实则不可（backend_config 文档无其他 load/inference 标记），故 scope 必要。
- 修复：文档明确"scope 是 load/inference 判别式，kind 仅文档类型 tag"。

**F7. [MED] `BackendConfigDocument.id` 必填但无消费者（死重量）**
- `manifest.rs:523` 要求 `id`，但 backend_config 只经 `$load_config`/`$inference_config` 到达，**无 manifest entry 与之匹配**，故 `validate_entry_ref` 的 id 交叉检查（`pack.rs:280-286`）对它不生效。实际值也任意（`configs/inference.json` 是 `"inference-default"`，`configs/load.json` 是 `"load"`）。`runtime_bridge.rs:369/:383` 仅用于错误消息 label。
- 修复：`BackendConfigDocument.id` 改 `Option<String>` 或删除。

**F8. [MED] `status` 写入"已发布"pack 在语义上是错的**
- `status:"ready"` 仅出现在 `models/diffusion/justinpinkney_miniSD/manifest.json:6`；所有 llama/whisper manifest 都省略。`PackModelStatus`（`manifest.rs:52-59`）的值 `Ready/NotDownloaded/Downloading/Error` 是**运行期下载状态**，非静态作者属性。一个刚打包的模型在 fetched 之前应是 `not_downloaded`，与 manifest 声明无关；diffusion pack 的 `status:"ready"` 出厂即谎报。生成 pack 在 `mod.rs:469` 由 `config.status` 设值（合理）；authored pack 读入但 llama 正确地省略并推断 `NotDownloaded`（`tests.rs:184`）。
- 修复：从 `justinpinkney_miniSD/manifest.json:6` 删 `status`；考虑把 `status` 移出 authored-pack schema（归运行期）。

**F9. [LOW] `default_preset` 在 ≥2 预设时事实必需；单预设自动选取是隐式便利**
- `manifest.rs:46-47` `Option<String>`；`resolve.rs:210-226`：0 预设→None；1 预设→自动取；≥2 无声明→`MissingDefaultPresetDeclaration` 错误。diffusion pack（`:31-38`）依赖单预设自动选取而**省略 `default_preset`**，与 llama/whisper（显式 `"default"`）不一致——加第二个预设即脆断。
- 修复：`presets` 非空时 schema 强制 `default_preset`。

**F10. [LOW] manifest `version` 字段无约束且无消费者**
- 所有 authored pack `"version":2`（`Qwen2.5-0.5B-Instruct/manifest.json:3` 等），生成 pack `"version":1`（`mod.rs:466`）。`grep` 确认**无代码读取** `manifest.version`——解析后存储从不分支。对比 `StoredModelConfig.schema_version`/`policy_version` 是真版本门控（`tests.rs:605-610/:654-659`）。
- 修复：接线或删除。

**F11. [LOW] `PackSource` serde 兼容三种 wire 格式 + legacy `hub_provider` 重映射**
- `manifest.rs:251-323` 支持 `SourceOnly`/`Flat`/`Legacy` 三格式并按 legacy `hub_provider` 字段重映射 `hugging_face`↔`model_scope`。schema（`slab-manifest.schema.json:156-279`）仅文档化 canonical `oneOf`，legacy 形态未文档化但仍接受（测试 `manifest.rs:572-606`）。
- 修复：文档化或加 deprecation 日志日落 legacy 形态。

### 3.2 PMID 回显专项（字段模糊、层级混乱定位）

**PMID-F1. [HIGH] 环境变量与 PMID 双源断开——`server.admin.token` PMID 可编辑但对鉴权无效**
- `auth_middleware`（`auth.rs:23`）读 `state.context.config.admin_api_token`（`Config` 结构体）；`Config::from_env()`（`app_config.rs:128`）从 `SLAB_ADMIN_TOKEN` env（`:156`）取值。**`server.admin.token` PMID 与鉴权完全不相通**——用户在设置页改 `server.admin.token` 不改变实际鉴权要求。同理 `SLAB_BIND`（`:139`）vs PMID `server.address`；`SLAB_LOG*`（`:142-146`）vs `logging.*` PMID；`SLAB_QUEUE_CAPACITY`/`SLAB_BACKEND_CAPACITY`（`:150-151`）vs `runtime.capacity.*` PMID。
- 哪个生效取决于消费者走哪条代码路径，**优先级未定义**。
- 修复：建立 env→PMID 单向桥（env 仅作首次种子，运行期以 PMID 为准），或显式文档化两者边界并让设置页标注"此字段由环境变量覆盖"。

**PMID-F2. [HIGH] 五层并行 `logging.{level,json,path}` 覆盖无级联解析器，优先级隐式无文档**
- 存在 5+ 并行 logging 组：根 `logging`（`descriptor.rs:26-28`）、`runtime.logging`（`:117-119`）、`runtime.ggml.logging`（`:142-150`）、`runtime.ggml.backends.{llama,whisper,diffusion}.logging`（`:183-282`）、`runtime.candle.logging`/`runtime.onnx.logging`/`server.logging`。根 `logging` 是 `LoggingConfig`，其余是 `LoggingOverrideConfig = Option<...>`（`document.rs:174-185`）。
- 引擎仅解析**一个** fallback：`runtime_log_dir = settings.runtime.logging.path.or_else(|| settings.logging.path)`（`pmid_service.rs:167-169`）。对 `level`/`json`，`load_config` 内**无任何级联解析**——每个叶字段只经 `setting_value` 读取，消费者各自继承。优先级因此**隐式且无文档**；用户设 `runtime.ggml.backends.llama.logging.level` 无法从回显判断它是否覆盖 `runtime.logging.level`。
- 修复：在 `load_config`/launch 实现显式优先级并反映到 `description_md`，或移除无解析器的 override 层。

**PMID-F3. [HIGH] `telemetry.metrics_exporter` 类型 laundering——判别联合被洗成不透明 Object**
- `OtelExporter` 是内部 tag 联合（`#[serde(tag="type")]`，`slab-otel/src/config.rs:57-74`），变体 `{none, local_file{directory}, otlp_http{endpoint,headers,protocol,tls}}`。但回显层：`value_type = Object`（`pmid_service.rs:821`）、标 `multiline`（`:949`，对字符串 textarea 的语义提示，对类型联合语义错误）、**无 `json_schema`**（`:909-928` 仅给 `span_attributes`/`tracestate` 的扁平 string map）。客户端看到自由形态 object，无 enum 提示，无法验证。
- 修复：为该 PMID 发射 `json_schema`，并把 `value_type` 设为更诚实的 tag。

**PMID-F4. [HIGH] `exporter`/`trace_exporter`/`metrics_exporter` 三字段不对称——仅一个可编辑**
- `OtelSettings` 暴露三个（`slab-otel/src/config.rs:112-120`）；descriptor catalog（`descriptor.rs:35-44`）仅注册 `telemetry.metrics_exporter`。测试（`pmid_service.rs:1868-1871`）显式断言 `slab_home`/`exporter`/`trace_exporter` **不被回显**。`metrics_exporter` 用户可编辑，logs/traces 的 exporter 运行期从 `slab_home` 注入（`config.rs:136-141`）。哪个 exporter 控制哪个信号在 catalog 无文档，trio 不对称。
- 修复：要么三个都暴露并显式标注，要么在 `property_description` 文档化 logs/traces 自动派生自 `slab_home`。

**PMID-F5. [MED] `setting_value_from` 吞掉序列化错误 → 静默 Null**
- `descriptor.rs:434-436` `serde_json::to_value(value).map(SettingValue::from).unwrap_or_default()`。字段 `Serialize` 失败（如 Windows 下非 UTF8 的 `PathBuf`）时静默返回 `Null`；客户端看到 null `effective_value` 可能据此 `Unset`，破坏状态。
- 修复：传播为 `ConfigError::Internal`。

**PMID-F6. [MED] u64 溢出静默降 f64、NaN/Inf → Null**
- `view.rs:42-51`：JSON 数先 `as_i64()`，再 `as_u64().and_then(i64::try_from)`（u64>i64::MAX 落空），再 `as_f64().unwrap_or_default()`——`9_223_372_036_854_775_808` 变 `Number(9.223…e18)`，**静默整数→浮点精度损失**（影响 `models.auto_unload.min_free_*_memory_bytes` 等 u64 字段，`document.rs:918`）。对称地 `view.rs:68-70` NaN/±Inf → Null。
- 修复：溢出报错而非降级；NaN/Inf 在入口拒绝。

**PMID-F7. [MED] `parse_env` 对畸形数值静默回退默认**
- `app_config.rs:196-198` `v.parse().ok().unwrap_or(default)`。`SLAB_QUEUE_CAPACITY=abc` 静默变默认 64，无 warn。`SLAB_ENABLE_SWAGGER` 语义反转（`:152-154` 把除 `"0"`/`"false"` 外一切视为 enabled——`SLAB_ENABLE_SWAGGER=no` 反而启用）。
- 修复：解析失败 warn；truthy/falsy 解析统一。

**PMID-F8. [MED] `minimum` 在写路径声明却不强制**
- `pmid_service.rs:893-907` 为数值 PMID 设 `minimum=0`，但 `descriptor.rs:438-447` 的 `set_setting_value` 仅做"能否反序列化"检查。`runtime.capacity.queue = -5` 不会被拒（除非 `RuntimeTransportMode` 的 Deserialize 失败，但 capacity 是数值无 enum 拒绝）。
- 修复：写路径消费声明的 `minimum`/`maximum`。

**PMID-F9. [LOW] 脱敏白名单硬编码，未由 schema `writeOnly` 驱动**
- `secret()`（`:971`）与 `redact_setting_value()`（`:977`）各维护一份硬编码路径列表（`server.admin.token`/`providers.registry`/`agent.tools.websearch.providers`），需**两处同步**。schema 文档已标 `writeOnly:true`（`document.rs:1171/:1405`，测试 `:1701`）但回显层**不消费** `writeOnly` 来驱动脱敏——是平行维护。`agent.tools.mcp.servers` 不在脱敏列表（其 `env` 据设计为 env-var 名引用 `document.rs:1479`，非明文，故当前安全；但若未来存明文 secret 会漏）。
- 修复：脱敏由 `writeOnly` schema 标注驱动，单源真源。

**PMID-F10. [LOW] 所有数值 `value_type` 都判为 Integer（无 Float 变体）**
- `pmid_service.rs:849-852` 把 `Integer` 与 `Number` 都映成 `SettingValueType::Integer`；枚举无 Float（`view.rs:84-91`）。任何 f64 PMID 会被误标 Integer。
- 修复：加 `Float` 变体。

**PMID-F11. [LOW] `domain/services/pmid.rs` 是 1 行 re-export 的不完整抽象**
- 全文 `pub use slab_config::PmidService;`。非死代码（把 `PmidService` re-export 进 app-core 命名空间），但语义空——无 app-core 特定抽象/行为，所有逻辑在 `slab-config::pmid_service`。`SettingsService`（`settings.rs:6-48`）仅包一层加 agent-runtime reload 副作用。
- 修复：删除 shim 直接 import，或补全 domain 包装。

---

## 4. 接口与数据库表设计缺陷

> 编号 D1.. 数据库列 / API 字段 / 实体字段三向对照，附 migration 文件名 + 行号。

### 4.1 契约一致性

**D1. [HIGH] `tasks.result_data` 信封化只保护主表，三个 media 子表 `result_data` 未信封化且与主表双写**
- `20260530010000_task_payload_envelopes.sql` 把 `tasks.result_data` 重包为 `{kind:"task_result",version:1,data:...}`。但 `image_generation_tasks`/`video_generation_tasks`/`audio_transcription_tasks` 各有独立 `result_data TEXT` 列（`20260421010000_media_tasks.sql:15,41,63`）**从不信封化**。
- `media_task.rs:162` 既 `insert_task_row(..., task.result_data.as_deref())` 写主表信封，又由 `update_*_result`（`:257-314`）写子表裸 result_data。view 查询 SELECT 两者（`:375-382`），`media_state_from_task` 只消费**主表**那个算进度（`:482`）。**单一真源不明**。
- `decode_task_payload`（`task.rs:219-239`）遇非信封 payload 静默返 `None`+warn——未来版本升级后，所有旧任务结果从 API 静默消失，无迁移信号。
- 修复：决定单一真源（建议删子表 result_data 或一并信封化），并加版本化解码注册表。

**D2. [MED] `model_downloads.source_key` 可空但实体非可选——遗留 NULL 行会 panic 反序列化**
- `20260415010000_model_download_source_key.sql` 列 `source_key TEXT` 可空（无 NOT NULL）；实体 `ModelDownloadRecord.source_key: String` 非可选（`entities/model_download.rs:11`）。`20260608000000_add_storage_value_checks.sql:140` 也未补 NOT NULL。`:7-9` 的回填只设非空值，**遗留 NULL 存活**。legacy NULL 行经 sqlx `Option<String>→String` 不匹配会 panic。
- 修复：一次性 NULL 清理后加 NOT NULL。

**D3. [MED] 多个 TEXT 状态/角色列缺 CHECK（兄弟表却有）**
- `chat_messages.role` **有** CHECK（`20260608000000_add_storage_value_checks.sql:66-68`，且 `validate_chat_role` `schemas/validation.rs:89` 同步）；但：
  - `agent_thread_messages.role` **无** CHECK（`20260519000000_agent_thread_messages.sql:9`），repo 存原值（`agent.rs:248`）。
  - `agent_threads.status` **无** CHECK（`20260325000000_agent_tables.sql:14`），repo 静默 fallback `Pending`（`agent.rs:13-22`）。
  - `plugin_states.runtime_status`/`source_kind` **无** CHECK（`20260422010000_plugin_states.sql:9/:3`），domain 只写三个字面量（`plugin.rs:40-42/:37-39`）。
  - `agent_memory_usage_events.source_kind` 无 CHECK（`20260611010000_agent_memory_usage_source_kind.sql:4`），enum 有 5 变体。
- 对比 `tasks.status`/`models.status`/`models.kind` 在 20260608 重建时加了 CHECK——**约束应用不一致**。
- 修复：为上述列补 CHECK，枚举与 Rust enum 同步。

**D4. [MED] `models.spec` JSON 列无 `json_valid()` CHECK，且经 `json_set` 原地改**
- 见 §2.2 T1。`20260608000000_add_storage_value_checks.sql:44-61` 给 status/kind 加了 CHECK 但**漏了 `json_valid(spec)`**。对比 `tasks.result_data` 在信封迁移里用了 `json_valid()`（`20260530010000_task_payload_envelopes.sql:3`）。
- 修复：加 `CHECK (json_valid(spec))` 等约束。

**D5. [MED] 选择状态横跨两表，单次 PUT 跨表无事务包裹**
- `model_config_state`（`20260409000000_model_config_state.sql:1-6`）存 `selected_preset_id`/`selected_variant_id`；`models.selected_download_source`/`materialized_artifacts`（`20260607000000_model_download_state.sql:1/:3`）在模型行存进一步选择/派生状态。`UpdateModelConfigSelectionRequest`（`schemas/models.rs:213`）与 `UpdateModelEnhancementRequest`（`:196`）一次 PUT 写 `selected_*`（→config_state）+ `context_window`/`runtime_presets`（→models 行），**schema 层无事务包裹**。
- 修复：服务层显式事务。

**D6. [MED] `agent_threads.config_json` 持久化但 API 从不返回**
- 列存在（`20260325000000_agent_tables.sql:16`），每次 upsert 写入（`agent.rs:114`），但 `AgentThreadResponse`（`schemas/agent.rs:350-360`）字段为 `id/session_id/parent_id/depth/status/role_name/completion_text/created_at/updated_at`——**无 config_json**。持久化的配置 API 永不回显。
- 修复：暴露或停止写入。

**D7. [MED] 布尔存 INTEGER 无 CHECK，`!= 0` 解码——`2`/`-1` 静默成 true**
- `plugin_states.enabled`（`20260422010000_plugin_states.sql:8`）、`agent_memory_phase1_outputs.selected_for_phase2`（`20260611000000_agent_memories.sql:32`）、`audio_transcription_tasks.detect_language`（`20260421010000_media_tasks.sql:59`）均 INTEGER 无 `IN (0,1)` CHECK。`plugin.rs:87/198`、`media_task.rs:454` 用 `value != 0` 解码——手工写入的 `2`/`-1` 静默成 true。
- 修复：加 `CHECK (col IN (0,1))`。

### 4.2 命名与语义

**D8. [LOW] "kind"/"status"/"source" 多语义过载**
- `models.kind`（`20240408000000_model_kind_and_backend.sql:5`）仅 `local|cloud`，却叫 kind——与 `task_type`/`source_kind`/`runtime_status`/`ModelConfigValueType` 的 "kind" 撞名。建议 `deployment_type`/`host`。
- "status" 跨 5 表语义不同（task/model/thread/plugin/session），允许值集与 CHECK 有无各异（见 D3）。建议各自限定命名（`task_state`/`model_lifecycle_state`/`thread_state`）。
- "source" 三义：`model_downloads.source_key`（不透明 slug）、`models.selected_download_source`（JSON 描述符）、`plugin_states.source_kind`/`source_ref`（enum+locator）。命名不消歧。

**D9. [LOW] 时间戳 TEXT 不一致——部分表有 strftime 默认，老表无默认**
- `agent_memories`（`20260611000000_agent_memories.sql:37,54`）用 `DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))`；`20240101000000_initial.sql:17` 等老表**无默认**，写者必须始终提供时间戳。手写 `strftime`（毫秒，如 `agent.rs:164`）与 `to_rfc3339()`（亚纳秒）混用，同列不同精度。

**D10. [LOW] `tasks.core_task_id` 无唯一约束却被文档化为 1:1**
- `entities/task.rs:16-17` 注释 "slab-core runtime TaskId (u64)"，暗示 1:1；但 `20240101000000_initial.sql:10`（重建 `20260608000000...:31`）无 UNIQUE。两行可共享 core_task_id。
- 另：`core_task_id: Option<i64>` 装不下完整 u64，>i64::MAX 会回绕。

**D11. [LOW] `chat_sessions.name NOT NULL DEFAULT ''`——空名与"未命名"不可区分**
- `20240101000000_initial.sql:30-31` `name NOT NULL DEFAULT ''`，自动创建路径（`chat.rs:24-27`）总插 `name=''`。`ChatMessage` 流可静默创建孤儿会话，`name=''` 无法区分"未命名"与"未设"。

**D12. [LOW] 跨表删除语义不统一（无统一软删）**
- `models` 删硬删（`model.rs:148`）；`plugin_states` 硬删（`plugin.rs:167`）但 API 有 `DeletePluginResponse{deleted:bool}`；`chat_sessions` 级联。无表用统一软删模式，"deleted" 纯运行期概念。

**D13. [LOW] 文档腐烂——实体/仓库注释引用已删除的表与列**
- `entities/model.rs:15` 注释描述 `models.provider`（已在 `20260530000000_remove_models_provider.sql:3` 删除）；`repository/task.rs:11-12` `model_id` 注释引用旧表名 `model_catalog`（`20240101000000_initial.sql:3` 已删）；`entities/chat.rs:10` chat role 注释只列 `user|assistant|system`（实际 CHECK 已扩展）。

---

## 5. 重复点与边界隐患（Duplications & Gaps）

### 5.1 跨模块冗余

**R1. [HIGH] 路径包含校验 3 套实现，语义不一致（安全相关）**
- `domain/services/model/catalog.rs:581-585` `validate_path`：纯词法（`Path::components().any(ParentDir)`），**不 canonicalize**——symlink/绝对分量绕过。
- `crates/slab-plugin` 的 `is_path_within_root`（用于 `plugin/assets.rs:40`）：canonicalize 式。
- `domain/services/plugin/package.rs:202-218` `ensure_path_within`：第三套，`root.canonicalize()?` 后 `path.starts_with(&root)`——**canonicalize root 但不 canonicalize path**，非规范 path（`root/./../root/evil`）可绕过。
- 修复：抽 `slab_utils::path::ensure_within_root(root, path)` 对**两侧** canonicalize，单一真源。

**R2. [HIGH] `RuntimePresets` 组装在 4 处重复（漂移几乎必然）**
- `infra/model_packs/command.rs:36-61`（`build_runtime_presets`，读 `JsonOptions`）与 `:163-183`（`build_runtime_presets_from_manifest`，读 `PackRuntimePresets`）——同一 `RuntimePresets` 结构 + 同一"至少一字段 set"门控；`build_local_model_command`（`:89-90`）以 `.or_else` 串联。再加 `schemas/models.rs:657-669`（响应映射）与 `:954-966`（请求→domain）。`RuntimePresets` 加一字段须改 **4 处**。
- 修复：抽 `RuntimePresets::from_optional_fields(...)` 单一构造器。

**R3. [HIGH] `manifest_status` ↔ `pack_status_from_unified` 是无共享测试的逆 twin**
- `command.rs:150-157`（`PackModelStatus → UnifiedModelStatus`，4-arm）与 `mod.rs:652-659`（逆 4-arm）手写同 4 变体。加一个 `UnifiedModelStatus` 变体（如 `Archived`）能编译但**一条路径静默丢状态**。
- 修复：`slab_model_pack` 内单一 `match` 或 `From` 宏。

**R4. [HIGH] `TaskStatus` 未知→Failed vs agent 未知→Pending——同一语义两种默认**
- `domain/models/task.rs:45-54`（unknown→`Failed`，logged）vs `repository/agent.rs:13-22`（unknown→`Pending`，logged）。同一"DB 未知状态"概念，任务变 Failed、agent 线程变 **Pending**（agent 运行期可能尝试 resume）。损坏的 agent 线程被静默当作可恢复——危险。
- 修复：统一策略（Failed 更安全）。

**R5. [HIGH] js-runtime vs python-runtime JSON-RPC 宿主骨架 ~95% 重复**
- `bin/slab-js-runtime/src/api/jsonrpc/mod.rs`（196 行）与 `bin/slab-python-runtime/src/api/jsonrpc/mod.rs`（224 行）：struct+`PendingMap`（`:21-28`）、`resolve_response`（`:43-56`）、`send_response`/`send_notification`/`send_serialized`（`:58-79`）、`impl RuntimeHost`（`:82-96` 逐字节同）、parse 循环（`:142-175` vs `:154-177`）、`drain_outbound`（`:181-196` vs `:209-224`）几乎全同。两者已 import 共享 `slab_jsonrpc`，但**只用于信封原语**，宿主态管道零共享。
- 修复：把 `JsonRpcRuntimeHost`/`drain_outbound`/`serve_reader` 移入 `slab_jsonrpc`（或新 `slab-jsonrpc-host`），参数化 `RequestHandler` trait。消除 ~150 行重复。

**R6. [MED] gRPC handler boilerplate ×80**
- `bin/slab-runtime/src/api/handlers/{candle_diffusion,candle_transformers,ggml_diffusion,ggml_llama,ggml_whisper,onnx}.rs` 共 **80** 处 `map_err(application_to_status|proto_to_status)?`（grep 核实）。映射函数本身已在 `handlers/mod.rs:36-82` 正确集中，故**非逻辑重复**而是样板。
- 修复：抽 `fn forward<Req,Resp,S>(req, decode_fn, service_fn)` 泛型。

### 5.2 逻辑死角与边界隐患

**G1. [HIGH] JSON-RPC pending-map 无界泄漏——丢响应永久残留**
- `bin/slab-js-runtime/src/api/jsonrpc/mod.rs:88`（≡ py）：`self.pending.insert(key, tx)` 仅在匹配响应到达时（`:47`）移除。**无超时、无上限、无清理任务**。host 进程不回 `host-N`（崩溃/挂起/协议 bug）时，`oneshot::Sender`+key 永驻 HashMap。长跑 runtime 缓慢内存泄漏 + `Mutex<HashMap>` 无界增长锁竞争加剧。`request()`（`:91-94`）在 `RecvError` 返错但**不删 key**。
- 修复：`rx.await` 包 `tokio::time::timeout`，超时/err 删 key。

**G2. [MED] 每请求 `tokio::spawn` 无背压/无监督**
- `bin/slab-js-runtime/src/api/jsonrpc/mod.rs:172-175`（≡ py）每个入站请求 spawn detached task，JoinHandle 丢弃——无 `JoinSet`、无并发上限、无监督。慢/挂 handler 泄漏 task；`server.handle_request` panic 静默死（响应只在闭包 happy path 发，panic 中途丢 `host.send_response`，对端永久等待）。
- 修复：`JoinSet` + 并发上限 + panic 捕获兜底响应。

**G3. [MED] `process_supervisor` 四个 fire-and-forget task，无 `Drop` abort**
- `crates/slab-app-core/src/infra/process_supervisor.rs:78,99,109,119` writer/stderr/stdout/wait task；监督靠 `AtomicBool alive`（`:114/:134`）+ `exit_handler`（`:135`）。`SupervisedProcess` handle 被 drop 时**四个 task 不 abort**，继续写 captured stdin/stdout 到子进程退出。无 `Drop` impl，泄漏的 supervisor 泄漏四 task + 子进程。
- 修复：`Drop` abort 四 task。
- （注：已核实 `workspace/mod.rs:512-513` 的 spawn 在 `:517-518` 被 await，**非**无监督；`slab-agent/src/control.rs:488` 存 `abort_handle`（`:498`）、TOCTOU 安全自移除（`:503-510`），**正确**——此二处不是 finding。）

**G4. [MED] 数据路径静默吞错（`.ok()`/`unwrap_or_default()`）**
- `encode_task_payload`（`task.rs:209-217`）最终 `serde_json::to_string(...).ok()` 失败返 `None` 无日志（sibling `decode_task_payload` `:222/:227` 却 warn）——编码失败不可见，丢失任务结果数据。
- `media_task.rs:490` artifact JSON 畸形 → `[]` 无日志；`:426` `frames` 溢出 → `unwrap_or_default()` 静默 0。
- `app_config.rs:196-197` `parse_env` 畸形 → 默认无诊断（见 PMID-F7）。
- 修复：数据/配置路径的 `.ok()`/`unwrap_or_default()` 至少 `warn!`。

**G5. [MED] `redact_secret_leaf`/`redact_api_key_fields` 等脱敏白名单硬编码两处须同步**
- 见 PMID-F9。

**G6. [LOW–MED] `unreachable!()` 标记承重且脆弱**
- `infra/rpc/client.rs:209,422,519` 重试循环末尾 `unreachable!("... retry loop should always return")`。已核实 `:209` 当前确不可达（`for 1..=N` 末轮 `attempt<MAX` 为 false 命中 `:204` return）。但**重构脆弱**（`continue`→fall-through 或 `1..N` 排他界即变可达 → 生产 panic）。`config_document.rs:543` `unreachable!("json payload should have been normalized to an object")` 承重于上游 normalizer。
- 修复：改 `return Err(...)` "exhausted retries" 或注释证明不变式。

**G7. [LOW] `workspace_info` 把不可 canonicalize 的 `workspace_root` 静默当"无 overlay"**
- `bin/slab-server/src/api/v1/workspace/handler.rs:534` `root.canonicalize().ok()` 失败 → `None` → `:538` 静默回退默认 settings 路径。配置错的 root 与"用默认"不可区分，无日志。对比 `ensure_path_within`（`package.rs:203`）**会**传播 canonicalize 错误——同一失败模式处理不一致。

**G8. [LOW] 硬编码 OpenAI base URL ×3、运行期端口常量**
- `pmid_service.rs:1685,1723,1777` 三处硬编码 `api_base:"https://api.openai.com/v1"`，应为单一 `const` 或 PMID。`:29/:31` 的 `DEFAULT_SERVER_RUNTIME_BASE_PORT=3001`/`DEFAULT_DESKTOP_RUNTIME_BASE_PORT=50051` 至少是 const 但非可配 PMID。

**G9. [LOW] `manifest_sha256` 不匹配静默重置下载状态**
- `infra/model_packs/mod.rs:274-277` `read_persisted_model_config_from_pack_bytes` 当 manifest SHA 与存储不符时返 `Ok(None)`；caller（`build_model_command_from_pack_bytes` `:129-131`）继续构建无 `selected_download_source`/`local_path`/`status` 的命令——**静默把已下载模型重置为 `not_downloaded`**。无 warn。
- 修复：warn 让运维知道投影态被丢弃。

---

## 6. 行动指南（Action Items）

### 6.1 P0（阻断性 / 安全 / 数据完整性，立即修）

| # | 行动 | 涉及 finding | 文件 |
|---|------|-------------|------|
| P0-1 | **统一路径包含校验**：抽 `slab_utils::path::ensure_within_root`，两侧 canonicalize；删除 `catalog.rs:581`/`package.rs:202` 两套 | R1 | `crates/slab-utils`、`catalog.rs`、`plugin/package.rs` |
| P0-2 | **修 `json_set` 绕过校验**：`models.spec` 改 Rust re-serialize 整列写 + 加 `json_valid()` CHECK | T1/D4 | `repository/model.rs:153-181`、迁移 |
| P0-3 | **`variant.id` 唯一性**：`validate_manifest_references` 增数组内 id 唯一检查 + 删两个 pack 的重复 `Q4_K_M` | F1 | `slab-model-pack/src/pack.rs`、两个 manifest |
| P0-4 | **asset-ref 校验前移**：`chat_template` 形态在 `from_bytes` 校验；修 Qwen2.5-0.5B 的裸字符串 | F2 | `pack.rs`、`Qwen2.5-0.5B/configs/load.json:8` |
| P0-5 | **`tasks.result_data` 单一真源**：删 media 子表 result_data 或一并信封化 + 版本化解码器 | D1 | `repository/media_task.rs`、`task.rs`、迁移 |
| P0-6 | **JSON-RPC pending-map 加超时清理**：`rx.await` 包 timeout，超时/err 删 key | G1 | `js-runtime` & `python-runtime` `jsonrpc/mod.rs` |
| P0-7 | **`AgentThreadRow.config_json` 非 Option 改 Option**（防 NULL 炸列表）；`depth` 改 u32+try_from | T3 | `repository/agent.rs:33-55` |

### 6.2 P1（系统性设计缺陷，本迭代治理）

| # | 行动 | 涉及 finding |
|---|------|-------------|
| P1-1 | **建立 env→PMID 单向桥或显式文档化边界**：`SLAB_ADMIN_TOKEN` vs `server.admin.token` 等 | PMID-F1、F-Stack-2 |
| P1-2 | **设置热重载扩宽或加 "needs-restart" 信号**：当前仅 agent 生效，runtime/inference 改了无提示 | F-Stack-1 |
| P1-3 | **5 层 logging override 加级联解析器或移除无解析器层 + 文档优先级** | PMID-F2 |
| P1-4 | **`telemetry.metrics_exporter` 发射 `json_schema` + 修 value_type；三 exporter 对称化或文档化** | PMID-F3/F4 |
| P1-5 | **为缺 CHECK 的状态/角色/布尔列补约束**：agent_threads.status、agent_thread_messages.role、plugin_states.runtime_status/source_kind、agent_memory_usage_events.source_kind、布尔列 `IN (0,1)`、`source_key` NOT NULL | D2/D3/D7 |
| P1-6 | **统一未知状态默认**：TaskStatus 与 agent thread 统一（Failed） | R4 |
| P1-7 | **抽共享 JSON-RPC 宿主到 `slab_jsonrpc`**（消除 ~150 行重复 + 顺带补 dispatch 测试） | R5 |
| P1-8 | **`RuntimePresets` 单一构造器 + `manifest_status`/`pack_status_from_unified` 单一 match** | R2/R3 |
| P1-9 | **发布子文档 schema**（variant/preset/backend_config/component/adapter 的 `$defs`）+ 重指 `$schema` | F3 |
| P1-10 | **gRPC 错误跨边界保结构**：错误信封保留机器可读 `code`，统一 agent 协议 Error | F-Stack-3 |

### 6.3 P2（清理 / 可维护性 / 命名）

| # | 行动 | 涉及 finding |
|---|------|-------------|
| P2-1 | 数据路径 `.ok()`/`unwrap_or_default()` 至少 `warn!`（encode_task_payload / media artifact / parse_env） | G4、PMID-F7 |
| P2-2 | `process_supervisor` 加 `Drop` abort 四 task；JSON-RPC per-request spawn 加 `JoinSet` + panic 兜底 | G2/G3 |
| P2-3 | 脱敏由 schema `writeOnly` 驱动（单源）；`agent.tools.mcp.servers` 评估纳入 | PMID-F9 |
| P2-4 | `unreachable!()` 改 `return Err` 或注释证明不变式 | G6 |
| P2-5 | 命名治理：`models.kind`→`deployment_type`；status/source 各表限定；`BackendConfigDocument.id` 改 Option | D8、F7 |
| P2-6 | 文档腐烂清理：`entities/model.rs:15`、`repository/task.rs:11`、`entities/chat.rs:10` 注释 | D13 |
| P2-7 | `agent_threads.config_json` 暴露或停写；`UnifiedModel.materialized_artifacts`/`selected_download_source` 加进 DTO 或文档省略 | D6、§2.2 |
| P2-8 | manifest `version` 接线或删；`default_preset` 多预设时 schema 强制；`status` 移出 authored schema | F8/F9/F10 |
| P2-9 | `$config`→`$document` 命名去歧义；OpenAI base URL 抽 const | F4、G8 |

### 6.4 验证策略建议

- **P0-2/P0-5**：迁移加 CHECK 后，补 round-trip 测试（坏 JSON 写入应失败而非砖读）。
- **P0-3/P0-4**：扩展 `slab-model-pack` 测试，断言重复 id 报 `DuplicateEntryId`、裸字符串 `chat_template` 在 `from_bytes` 即报错。
- **P1-5**：每个新 CHECK 配一条"违反约束的写应被 DB 拒"测试。
- **P1-1**：加集成测试断言"改 `server.admin.token` PMID 后鉴权行为改变"（当前会 fail，是 red-test 驱动修复）。

---

*本报告由 6 路并行深度审计（model_pack 配置 / PMID 回显 / API↔DB 契约 / 数据转换 / 冗余与死角 / 全栈数据流）+ 主审计员对全部 High 级发现直接读源码落地核实综合而成。被核实推翻的发现（PMID secret "纯装饰" 论）已剔除并记录于 §1.3。所有结论附 `path:line` 证据。评级 B−（弱 B，倾向 C+）：架构分层与契约生成是显著长板，model_pack 字段语义模糊、PMID 双源陷阱、持久层 CHECK 缺位与静默强转是系统短板。*
