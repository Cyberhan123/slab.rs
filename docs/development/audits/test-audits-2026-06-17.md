# Test Capability Audit Report (2026-06-17)

> **审计方法**: 15 个子系统并行审计（10 个本轮新审 + 5 个首轮已完成且经对抗式验证的 carry-forward），覆盖 Rust 与 TypeScript 双栈，新增**冗余维度 (Redundancy)** 与**对抗式验证 (Adversarial Verification)** 两个维度。24 项 High-severity 发现经独立 verifier 核实，被推翻的剔除、被高估的降级。用户当前打开的 `bin/slab-server/tests/smoke/server-api/todos.ts` 所在的 smoke 子系统，因两次 rate-limit 失败，由主审计员**直接读源码落地核实**（见 §3.1 G3）。

## 1. 执行摘要 (Executive Summary)

仓库规模：约 42 个 Rust crate（最大 `slab-app-core` ~42.8k LOC、`slab-proto` ~18.8k、`bin/slab-js-runtime` ~15.6k、`bin/slab-runtime` ~12.4k）；TS 侧最大为 `packages/slab-desktop` ~30.5k LOC。测试基础设施：8 个 vitest project、cargo workspace、v8 coverage（**未设置阈值门槛**）、CI 矩阵 ubuntu/macos/windows、54 个视觉基线、`bin/slab-server/tests` 结构化 smoke registry。

**相对于 2026-06-08（评级 C+）的关键变化：**

。
- 新增冗余维度后，确认 **Rust 与 TS 双侧的测试脚手架 (test scaffolding) 大面积复制**，且**跨 crate 缺乏共享 test-support crate** 是几乎所有冗余的根因。
- **smoke registry 被正名**：`bin/slab-server/tests` 的 `executableSmokeOperations / todoSmokeOperations / futureCompatibilityScenarios` 不是"占位冒充覆盖"，而是一套**有 drift-enforcement 测试守护的、纪律良好的覆盖率地图**（68 executable / 0 todo / 16 future，详见 G3）。
- 上轮部分发现经对抗式验证后被**修正/降级**（如 migration 覆盖率被高估、`slab-server` harness 重复被高估、auth timing-attack 被高估）。

**三大首要风险（各一句）：**


1. **安全边界（路径越界 / auth fail-open / 风险分类）回归保护极弱**：`resolve_path` 仅 1 个 escape 用例、`auth_middleware` `None` token fail-open 无测试、`BasicToolRiskAnalyzer` 零直接测试。
2. **Repository 层 + 6 个 gRPC handler 文件 + RPC retry/backoff 几乎零测试**，而 `rpc/client.rs` 是模型运行时可靠性的承重路径。

**评级（保留字母制，便于与 06-08 对比）：**

| 维度 | 06-08 | 06-17 | 说明 |
|------|-------|-------|------|
| Rust 单元覆盖（核心 crate） | C | **C+** | `slab-agent` 仍为标杆；app-core/desc/rpc 仍是黑洞 |
| TS 单元覆盖（desktop/api/plugin） | 未评 | **C-** | 量不小但 hooks/页面/store 覆盖空洞 |
| 冗余控制 | 未评 | **C-** | 跨 crate 无共享 test-support 是系统性的 |
| 安全/关键路径回归保护 | C | **C** | carry-forward 的 gRPC/auth 问题依旧，新增 risk/path 边界 |

---

## 2. 重复与冗余发现 (Identified Redundancies)

> 编号 R1.. 按影响面排序：先列**跨 crate 根因型**，再列 **package 内**，最后是低 ROI 的局部重复。路径为仓库根相对路径。

### 2.1 跨 crate / 跨 package 根因型（优先治理）

**R1. [已修复] 7 个 repository 测试手写 `CREATE TABLE IF NOT EXISTS` DDL，而非运行真实 migration —— 存在 schema drift 风险 [High]**
- 涉及文件：
  - `crates/slab-app-core/src/infra/db/repository/chat.rs:91,103`（chat_sessions, chat_messages）
  - `crates/slab-app-core/src/infra/db/repository/plugin.rs:287`（plugin_states）
  - `crates/slab-app-core/src/infra/db/repository/ui_state.rs:81`（ui_state）
  - `crates/slab-app-core/src/infra/db/repository/model_config_state.rs:98,119`（models, model_config_state）
  - `crates/slab-app-core/src/infra/db/repository/{model,model_download,task}.rs`（CREATE TABLE）
- 对比正解：`crates/slab-app-core/src/infra/db/repository/mod.rs:39,78`（`SqlxStore::connect` → `migrate!`），以及 `model.rs:196` 已存在的 `migrated_pool()` 半成品 helper。
- 根因 & 风险：这些测试断言的行为所依赖的表结构**不是生产结构**——任何 migration 增加列/CHECK 约束后，仓库测试仍按陈旧 schema 通过。子系统审计只点到"样板重复"，**跨切面升级点是 schema drift**。
- 当前已在 `src/test_support.rs` 增加 `migrated_test_store()` / `migrated_test_pool()`，普通 repository CRUD 测试已改为跑完整 migrations；仅保留两类**故意构造旧 schema**的单迁移测试（`remove_models_provider`、`task_payload_envelopes`）。切换后暴露并修复了一个真实 drift：`model_config_state` 的外键在 `models` rebuild 后仍指向 `models_old`，已通过 append-only migration `20260617000000_rebuild_model_config_state_fk.sql` 重建外键。

**R2. `run_git` 测试 helper 在两个 Rust crate 逐字符重复 [Low]**
- `crates/slab-git/src/repository.rs:621` vs `crates/slab-agent-tools/src/git.rs:267`
- 二者除 `Output` 导入路径外完全一致（`Command::new("git").arg("-C").arg(root).args(args).output().ok()`）。
- 建议：从 `slab-git`（已是 `slab-agent-tools` 依赖）以 `#[cfg(any(test, feature="test-utils"))]` 暴露 `run_git_test` 并 re-export，删除后者副本。

**R3. JS-Runtime 与 Python-Runtime 的 JSON-RPC server 骨架 ~170 行重复，仅 ~25 行不同 [Medium]**
- `bin/slab-js-runtime/src/api/jsonrpc/mod.rs`（196 LOC）vs `bin/slab-python-runtime/src/api/jsonrpc/mod.rs`（224 LOC）
- 整个 reader-loop / notification-dispatch / `serve_reader` 骨架重复；差异仅在 runtime 类型别名、`ready_payload()` 构造、import 路径。carry-forward R6 已点到，本轮验证：**重复率 ~85%，不是少数行**。
- 建议：抽取 `crates/slab-plugin-jsonrpc`（参数化为 `RuntimeReady` trait 或 `ready_payload()` 闭包），两个 binary 退化为薄入口。**顺带同时闭合两个 binary 的 "JSON-RPC dispatch 无 mod 测试" 盲区**。

**R4. `bin/slab-server` smoke harness 与 `slab-desktop` e2e harness 部分原语重复 [Low，已从 High 降级]**
- `bin/slab-server/tests/support/slab-server.ts`（350 LOC）vs `packages/slab-desktop/tests/e2e/support/fullstack-dev.ts`（905 LOC）
- 对抗式验证结论：6 个声称的"重复原语"中**仅 2 个是真重复**（free-port 分配 `listen(0)`、`sqliteUrlForPath`），1 个小模式（stdout ring buffer，但 cap 不同 200 vs 500）。`killProcessTree` 仅部分共享（posix 分支不同）；`writeTestSettings` 与 `spawnSlabServer` **服务于不同 scope**（smoke 写极简 V2 doc，e2e 写 ~90 行全量 doc），不可合并。
- 建议：仅抽取 `reserveTcpPort / sqliteUrlForPath / 日志 ring buffer` 到共享 `scripts/test/harness.ts`。**不要按原 High severity 强行合并 harness**——会破坏 scope 隔离。

**R5. Vitest `resolve.alias` 块在 desktop jsdom 与 browser config 逐字重复 [Low]**
- `packages/slab-desktop/vitest.config.ts:25-57` vs `packages/slab-desktop/vitest.browser.config.ts:38-69`（`dedupe:["react","react-dom"]` + 5-entry alias 数组）。`slab-components` browser config 同型。
- 建议：抽取 `packages/slab-desktop/vitest.aliases.ts`，3 个 config 引用。

### 2.2 Package 内 / 单 crate 内

**R6. `build_pack` / `build_pack_bytes` ZIP 构造在同 crate 内 3 处重复 [Low]**
- `crates/slab-app-core/src/test_support.rs:416`（`CompressionMethod::Stored`）
- `crates/slab-app-core/src/infra/model_packs/tests.rs:23`（与上面**逐字相同**）
- `crates/slab-app-core/src/infra/model_packs/archive.rs:44`（prod，`Deflated` + 真实错误处理）
- 建议：删除 `tests.rs:build_pack`，统一调用 `test_support::build_pack_bytes`。

**R7. `make_request` / `make_model` / `make_message` schema fixture builder 跨模块重复 [Low]**
- `crates/slab-app-core/src/schemas/chat.rs:1146,1178`、`domain/services/chat/cloud.rs:1233`、`domain/services/model/tests/mod.rs:373`
- 建议：在 `test_support.rs` 集中 `chat_request() / unified_model(spec) / assistant_message(text)`。

**R8. `temp_settings_path()` helper 在 3 个 slab-config 测试模块重复 [Low]**
- `crates/slab-config/src/app_config.rs:328-332`、`provider.rs:672-676`、`pmid_service.rs:1507-1510`
- 加上 `write_json`（`app_config.rs:334-337`）与 prod `slab-utils cab/fsops.rs:71-75` 的重复；以及 12+ 处 `let _ = fs::remove_dir_all(...)` 清理尾（**panic 时会泄漏 temp dir**）。
- 建议：在 `slab-utils` 已有的 `test_support` 下加 `test_settings() -> (TempDir, PathBuf)`，Drop 自动清理。

**R9. `fn ctx() -> ToolContext` 在 9 个 slab-agent-tools 模块逐字复制 [Low]**
- `crates/slab-agent-tools/src/{shell.rs:137, git.rs:173, glob.rs:161, grep.rs:251, plan.rs:171, apply_patch.rs:82, fs.rs:253, fs_watch.rs:129, web_search.rs:507}`；mcp.rs:218、subagent.rs:308/350 内联同型。
- 建议：`slab-agent-tools` 内加 `#[cfg(test)] pub(crate) mod test_support`。

**R10. `temp_root()` 手写 tempdir 在 5 个 tool 模块重复（且 panic-leak）[Low]**
- `apply_patch.rs:125, fs.rs:313, git.rs:256, glob.rs:252, grep.rs:445`
- 与 `memories` crate 已用的 RAII `tempfile::tempdir()`（`fs.rs:180, git.rs:81`）形成对比。
- 建议：全部改用 `tempfile::tempdir()`（已是依赖），**同时消除重复与 panic 泄漏风险**。

**R11. Mock `AgentStorePort`/`ApprovalPort`/`NoopNotify` 在两 crate 重复 [Low]**
- `crates/slab-agent/src/tests.rs:581-874` vs `crates/slab-agent-tools/src/subagent.rs:171-288`
- 因 `slab-agent` 的 test doubles 是 `tests.rs` 私有的。
- 建议：以 `#[cfg(any(test, feature="test-support"))]` 或独立 `slab-agent-test-support` crate 暴露。

**R12. `createUiStateStorage` vi.mock factory 在 4 个 desktop store 测试逐字重复 [Medium]**
- `packages/slab-desktop/src/store/__tests__/{useAssistantUiStore,useHeaderUiStore,useAudioUiStore,useImageUiStore}.test.ts:5`
- 7 行 no-op factory + 每个 `beforeEach` 的 `setState` reset。
- 根因：repo 范围内 **TS 包没有任何 `src/test/` 或 `__mocks__/`**（已 glob 验证）。
- 建议：`packages/slab-desktop/src/store/__tests__/_helpers.ts` 导出 `mockNoopUiStateStorage()` + `resetStore(store, partial)`。

**R13. `vi.mock('@slab/api', ...)` stub 在 6 个 browser/visual/e2e 测试文件重复 [Low]**
- `packages/slab-desktop/tests/browser/visual/{assistant-page,settings-page,plugins-page}.browser.test.tsx`、`tests/browser/e2e/{assistant-core-flow,settings-core-flow}.browser.test.tsx`、`src/lib/__tests__/model-config.test.ts:12`
- `apiClient:{GET,POST,PUT,DELETE}` 四元组 + `default:{useQuery,useMutation}` 骨架是主流形状。
- 建议：`packages/slab-desktop/tests/browser/support/mock-slab-api.ts` 导出 `makeSlabApiMock(overrides?)`。

**R14. 错误消息 fallback 表在 `lib` 与 `assistant` 测试中重复断言 [Low]**
- `src/lib/__tests__/error-description.test.ts:6-18` vs `src/pages/assistant/lib/__tests__/assistant-request-errors.test.ts:76-85`
- 后者（`getAssistantErrorDescription`）只是前者的薄包装。
- 建议：`assistant-request-errors.test.ts` 改为 spy 验证委托，不重测全表。

**R15. carry-forward：6 个 gRPC handler 文件 forward boilerplate 重复 [Medium]**
- `bin/slab-runtime/src/api/handlers/{ggml_llama,ggml_whisper,ggml_diffusion,candle_diffusion,candle_transformers,onnx}.rs`（~119 处 `application_to_status`/`proto_to_status`/`extract_request_id`）

**R16. carry-forward：`make_worker()` test stub 每个 backend worker 各重写一遍 [Medium]**
- `slab-runtime-core/runner.rs:374-403` 的 `TestHandler/Observed` 私有未导出；`bin/slab-runtime/Cargo.toml` 无 `[dev-dependencies]`。

**R17. carry-forward：3 个 slab-server handler 单测只断言 `#[openapi]` macro 形状 [Low]**；`slab-agent-tools/apply_patch.rs:83-122` 重测 `slab-apply-patch` golden（与 R6 类同型问题，独立于 R6）。

**R18. oxlint vitest-env override 与 package.json lint script glob 双重维护 [Low]**
- `oxlint.config.ts:46-62` 与 `package.json:31-32`

---

## 3. 测试缺失与盲区 (Missing Details & Gap Analysis)

> 编号 G1.. 按域分组。严重级已根据 verify verdict 调整（confirmed/partial/refuted 影响 weight）。**Confirmed 项保留**；**refuted 项移除/弱化**；**partial 项带 caveat**。

### 3.1 强制执行（最高优先级）

**G1. [正名/纠正] `bin/slab-server/tests` 的 smoke TODO registry 是纪律良好的"覆盖率地图"，而非占位冒充覆盖 —— 但有两处真实盲区 [Medium]**
- 本子系统两次 rate-limit 失败，下列结论由主审计员**直接读源码核实**（用户当前打开的 `todos.ts` 即其中一环）。
- **量化（落地）**：
  - `executableSmokeOperations`（`shared.ts:47-116`）= **68 条**，全部为当前 `/v1/*` + `/health` 的真实操作。
  - `todoSmokeOperations`（`shared.ts:118`）= **0 条**（`[] as const`）—— **当前 API 边界零占位**。
  - `futureCompatibilityScenarios`（`shared.ts:120-137`）= **16 条**，**全部**为 Slab 尚未实现的 llama-server 兼容端点（`/v1/embeddings`、`/v1/rerank`、`/v1/responses`、`/v1/messages`、`/v1/slots`、`/v1/lora-adapters` 等）。`todos.ts` 把它们注册成 `it.todo` —— 这是**合法的 roadmap 标记**，不是假覆盖。
- **正名（重要纠正）**：`core-and-setup.ts:44-48` 有一条**真实 enforcement 测试**——
  ```ts
  const covered = [...executableSmokeOperations, ...todoSmokeOperations]
    .map(operationKey).toSorted();
  expect(new Set(covered).size).toBe(covered.length);          // 无重复
  expect(documentedOperationKeys(openapi.body)).toEqual(covered); // registry == 实时 OpenAPI 文档
  ```
  即 registry 与**活的** `/api-docs/openapi.json` 必须**逐项相等**——任何人新增 `/v1` 路由而不更新 registry，此测试即 fail。README "Smoke policy"（`tests/README.md:47-56`）也把这条写成显式契约。**这与多数代码库的"占位冒充覆盖"恰恰相反**，是该子系统最值得肯定之处。首轮审计把 `it.todo` 模式判为"弱点/未强制"的猜测**被推翻**。
- **真实盲区 1（Low-Medium）**：enforcement 测试只校验 *registry == doc*，**不校验每条 `executableSmokeOperation` 真的被某个 `it()` flow 调用过**。一条操作可被声明（满足 doc-equality）却从未被任何 flow 实际请求。建议补一条 meta-test：对每条 `executableSmokeOperation` 断言其 `operationKey` 出现在某次 flow 的请求记录里。
- **真实盲区 2（已纠正）**：重新按当前代码核实后，`admin-auth.ts` 已有 `it.skipIf(Boolean(externalBaseUrl))("requires bearer auth for management routes", ...)`，本地 harness 会用 `SLAB_ADMIN_TOKEN=vitest-admin-token` 覆盖未授权/已授权两条路径；原先 "`it()`=0 / admin 鉴权 smoke 为空壳" 的描述已过时。剩余风险不在 admin-auth flow 是否存在，而在外部目标模式会跳过该 smoke、以及非 loopback 且未配置 token 的失败路径需要直接单测守护（见 G3）。
- **真实盲区 3（设计性，非缺陷）**：smoke 按 README 设计即"stable validation / not-found / error-envelope，而非真实下载模型或推理"——它是 **presence/shape 层**，不是行为/推理覆盖。这是 smoke 的合理天花板，不是 bug；但意味着它**不能替代** §3.3/§3.7 中那些缺失的深集成测试。

### 3.2 安全 / 关键路径回归保护（本轮新增的核心盲区）

**G2. [High → Medium, 已补核心边界测试] Workspace 路径越界测试——主安全边界**
- `crates/slab-file/src/system.rs:149` `resolve_path` 仅 1 个 escape 测试（`resolve_path_rejects_workspace_escape` @ line 461，只喂 `../outside.txt`）。未测：symlink 越界、Windows 绝对路径、UNC `\\?\`、write-to-new-file 路径（`existing_ancestor(parent)` 仅校验、未最终 canonicalize，line 170-175）、parent-of-parent 越界、`..` 在 normalize 后的残留。
- `crates/slab-app-core/src/domain/services/workspace/file_system.rs` `LocalExecutorFileSystem`（170 LOC，实际接到 Tauri `WorkspaceService`）**0 测试**，且 write 时不查 `policy`/`writable_roots`/`denied_paths`。
- 当前已补 `resolve_path` 边界测试：绝对路径、Windows extended path（`\\?\`）、任何 `..` 段拒绝、symlink 指向 workspace 外部时 existing-file 与 new-file 两条路径都拒绝。实测发现当前实现比原审计假设更严格：`dir/../inside.txt` 也会被拒绝，而不是 normalize 后放行。
- 当前已为 `LocalExecutorFileSystem` 补基本 round-trip/escape 测试：write 自动建目录、read、metadata、read_directory、`../` escape 拒绝。
- 剩余建议：单独定义并测试 `FileSystemSandboxContext.policy`、`writable_roots`、`denied_paths` 的语义；当前 `LocalExecutorFileSystem` 仍主要依赖 workspace root containment，而不是完整 sandbox policy evaluator。

**G3. [High → Low-Medium, 已部分修复] Auth middleware `None` token fail-open**
- 当前代码已从原先 `admin_api_token == None` 直接放行，改为**仅 loopback bind address 允许未配置 token 的本地管理访问**；`0.0.0.0`、`[::]`、局域网 IP 等非 loopback bind 在未配置 token 时 fail-closed。显式配置空串/whitespace token 也 fail-closed。直接单测已覆盖 loopback 例外、非 loopback 拒绝、空/whitespace token 拒绝、匹配/不匹配 bearer token。
- 纠正：原审计把 "`None` token 必须一律拒绝" 写成唯一正确修复，但当前 repo 文档和桌面本地开发契约仍把未配置 token 的 loopback 管理访问视为允许行为；因此真实安全边界应是**远程/非 loopback 不得无 token 开放**，而不是破坏本地默认流。
- 注意：carry-forward 把"timing-attack [Medium-High]"列为 lead concern，本轮**降级** timing-attack 为 **Low-Medium**（localhost/loopback 单用户 Tauri sidecar token，timing 攻击 largely 理论）；lead concern 已收窄为 remote bind fail-open 与空白 token 配置。
- 剩余建议：如后续产品决定所有管理路由都必须强制 token，需要先补齐桌面/前端 token 注入链，再移除 loopback 例外；也可补非 loopback no-token 的集成 smoke。

**G4. [High → Low, 已补直接测试] `BasicToolRiskAnalyzer`（唯一 risk impl，gatekeep host approval）**
- `crates/slab-agent/src/risk.rs:13-45`：`rm `/`remove-item`/`git reset`/`del ` → High/`destructive_command`；其他 shell → Medium；非 shell → Low。lowercase 处理。
- `tests.rs:840/854/870` 的 approval mock 全部 `_risk:` 丢弃；唯一相关断言 `tests.rs:1786-1794` 只对 echo 命令断言 `risk: Some(_)`（非 null 即过，不断言 level/label）。
- 当前已在 `risk.rs` 内补 `#[tokio::test]` 直接套件，覆盖 destructive shell 命令 High、普通 shell Medium、非 shell Low、大小写触发、label/reason 形状。
- 剩余 edge：当前实现仍是字符串包含启发式，不是 shell parser；`rm`/`del` 单独 token 明确保持 Medium，`git reset` 子串匹配仍偏保守。若后续要降低误报/漏报，应作为行为设计变更单独处理。

**G5. [High → Low, 已补表驱动测试] Secret redaction 覆盖不足**
- `crates/slab-agent-memories/src/redaction.rs:14-23`：3 个 regex。唯一测试 `redacts_common_secret_shapes` @ :29 只测 `api_key=` 与 `Authorization: Bearer`。未测：`sk-` 前缀（regex 3）、`token=`/`password=`/`passwd=`/`secret:` 变体、大小写、**12 字符下限边界**（11 字符 token 不应 redact 的 false-positive guard）、引号风格、一行多 secret、benign 字符串透传。
- 风险面：`redact_secrets` 在写 `raw_memories.md` 前运行（`phase1.rs:72,112-114`），under-coverage = 数据外泄面。
- 当前已补表驱动测试覆盖 keyword 变体、Bearer、`sk-`、短值误报保护、一行多 secret、benign 字符串透传，并修复带引号 secret 被替换后丢闭合引号的问题。
- 剩余策略边界：regex redaction 仍是启发式，不保证覆盖所有供应商 token 形态；如后续引入 secret store，应把具体 credential 类型投影到结构化 redaction，而不是继续扩 regex 表。

**G6. [High → Low, 已修复主要泄漏路径] slab-config secret-redaction contract**
- `crates/slab-config/src/pmid_service.rs:265` 设 `secret: secret(pmid)` 但 :269 原样返回 `effective_value`——flag 纯装饰；`secret()` (:940-942) 仅匹配 `server.admin.token`，故 `providers.registry`（`auth.api_key` @ 1469-1470 逐字 clone）、websearch/MCP key 从不标记 secret。
- 严重级纠正：High→**Medium**。`/v1/settings` 路由在 `auth_middleware` 之后（需 admin bearer token），能读 api_key 的攻击者已持有 admin token，可直接读原始 settings 文件——属 defense-in-depth/correctness 缺口，非未授权披露。
- 当前已修 `server.admin.token`、`providers.registry`、`agent.tools.websearch.providers` 的 property view：`schema.secret=true`，`effective_value`/`override_value` 中 literal secret 使用 `[REDACTED_SECRET]`，且 `PUT /settings/{pmid}` 收到占位值时会恢复当前真实 secret，避免结构化 autosave 覆盖密钥。已补测试断言 view 序列化不含明文、占位更新不会污染持久化文件。
- 纠正：MCP server 配置当前只保存 `env_var` 引用名，不保存 secret 明文，因此不应把 `agent.tools.mcp.servers` 整体标成 secret。

**G7. [High → Medium, confirmed] Config descriptor get/set 类型校验对 ~100 PMID 无直接测试**
- `crates/slab-config/src/descriptor.rs`（447 行）+ `view.rs`（185 行）均无 `#[cfg(test)]`。`set_setting_value`（descriptor.rs:438-447）包装 `serde_json::from_value`；`view.rs:42-52` u64→i64 `try_from` 对 >i64::MAX 静默落到 f64；`:68-70` NaN/Infinity → Null。
- 严重级纠正：High→**Medium**。核心类型拒绝委托给 serde（久经考验），真正未测的是两个**静默强转** edge case + 缺 round-trip，不是"每个 PMID 都有未测校验"。
- 建议：表驱动测试 type-mismatch→BadRequest、round-trip、u64-overflow、NaN。

**G8. [High, confirmed] MCP 工具 `execute()` 错误/参数校验路径完全未测**
- `crates/slab-agent-tools/src/mcp.rs:47-66`（McpCallTool::execute）、:141-156（McpProxyTool::execute）。4 个 mcp.rs 测试仅断言 name/description/schema；唯一 execute 测试（:215）只跑空 server 列表的 happy path。未测：缺 `server_name`/`tool_name`、remote call_tool 失败→`AgentError::ToolExecution`、McpProxyTool::execute。
- 严重级纠正：High→**Medium**。`string_arg` 的 AgentError 逻辑本身在 `args.rs:18-28` 已单测；底层 `McpClient::call_tool` happy/ServerNotFound 在 `slab-mcp/src/client.rs:257-279` 已测。未测的是 mcp.rs 薄包装层（机械 `.map_err()` + serde_json::to_string，serde 错误分支基本不可达）。
- 建议：加 fake McpClient（或 trait seam）驱动缺参/未知 server/remote 错误/proxy 成功。

**G9. [High, confirmed] `infra/rpc/client.rs` gRPC retry/backoff/timeout 零测试**
- `LOAD_MODEL_MAX_ATTEMPTS=3`/250ms、`UNARY_RPC_MAX_ATTEMPTS=2`/100ms、`RPC_REQUEST_TIMEOUT=30min`、`is_transient_runtime_status` 字符串匹配 predicate、各 loop 末尾 `unreachable!()`。`gateway.rs`/`mod.rs` 同样 0 测试；`codec.rs` 仅 encode 测试。
- 鉴于 3 个文件全无测试 + 承重可靠性路径，维持 High。
- 建议：提取 retry 决策为纯函数单测，或用 fake tonic channel；最低限度测 RequestIdInterceptor 与 retry-count 数学。

### 3.3 核心业务逻辑 / 持久化

**G10. [High, confirmed] Repository 层 ~110 CRUD 方法仅 19 测试**
- `config.rs=0, media_task.rs=0（24 trait 方法全无测）, session.rs=0, chat.rs=1, agent.rs=1, plugin.rs=1, ui_state.rs=1, model_config_state.rs=1`。`model.rs=2, model_download.rs=5, task.rs=6` 是亮点模板。
- 域服务测试 43 个大多用 mock，仅 3 个真正打 SQL（catalog.rs=2, download.rs=1）。
- caveat：已有单测并非全 trivial（chat.rs 验 UNIQUE 回滚、agent.rs 验字段保留、ui_state.rs 验 round-trip）——"CRUD round-trip 大多未验"略过述。
- 建议：按 `model_download.rs` 模板补 round-trip（insert→get→update→delete→get-none）+ unique-violation + 并发写。

**G11. [High → Low-Medium, partial, 已补 post-apply 约束/级联断言] Migration 覆盖率**
- 真相（已 verify）：19 个 migration 文件**全部有 forward-apply smoke 覆盖**（`storage_value_checks` @ model.rs:275、`turn_state_upsert` @ agent.rs:313、`connect_configures_sqlite_concurrency_pragmas` @ mod.rs:77 均 `migrate!` 全量）。仅 **~3-4 个**有 post-apply schema/constraint 断言；**ZERO rollback** 测试。
- **纠正 06-08 与首轮原始发现的"2 of 19 forward-tested"——错误**。准确表述：全部 forward-apply-tested，~3-4 有 post-apply 断言，0 rollback。
- 严重级：High→**Medium**（单用户 SQLite 桌面应用，blast radius 是本地 DB，可 re-init 恢复）。
- 当前 repository 测试已改用真实 migrations，并新增 `20260617000000_rebuild_model_config_state_fk.sql` 修复 `model_config_state` 外键漂移；这证明 R1 的手写 DDL 确实遮蔽了 schema drift。
- 当前已补真实 post-apply 断言：`infra::db::repository::tests::migrations_apply_expected_constraints_and_indexes` 校验 `model_config_state`、media task、agent memory 相关 FK/default/index/CHECK 约束；`migrations_preserve_media_and_agent_memory_cascades` 插入真实 parent/child 行并验证 media tasks 与 agent memory phase1 级联删除、`agent_memory_usage_events.source_kind` 默认值。
- 纠正：当前 `migrations/` 目录全部是 SQLx simple `.sql` migration；SQLx 0.9 禁止 simple 与 reversible `.up.sql`/`.down.sql` 混用，因此不能在不重组整套 migration 的前提下只补某几个 rollback 脚本。剩余建议改为：若产品需要 rollback gate，先规划一次全量 reversible migration 迁移；短期继续扩大 post-apply schema/constraint/data-preservation 矩阵。

**G12. [Medium] model_auto_unload.rs 只测纯 helper，async eviction state machine 未测**
- 786 LOC，4 个测试仅覆盖 `is_under_memory_pressure` 等纯函数。`ModelAutoUnloadManager` loop（轮询、触发 eviction、与 supervisor 协调、`max_pressure_evictions_per_load` cap、`RuntimeMemoryPressure` 恢复）未测。
- 建议：用 `RecordingRuntimeGateway` + fake memory snapshot 驱动一整轮 pressure→evict→recover。

**G13. [Medium] 大型 service 文件零直接测试**
- `domain/services/chat/local.rs`（671 LOC，0）、`plugin/validation.rs`（655 LOC，0，**安全相关输入边界**）、`model/runtime.rs`（681）、`model/download.rs`（766）、`model/catalog.rs`（640）。
- 建议：优先 `plugin/validation.rs`（输入校验安全边界）与 `chat/local.rs` 错误路径。

**G14. [Medium] `agent/memory.rs`（920 LOC）仅 3 测试**
- recall/ranking、source-kind routing、与 `agent_memories`/`agent_memory_usage_source_kind` 新 migration 的持久化交互薄。

### 3.4 并发 / 状态机 / 取消安全

**G15. [Medium-High → Low-Medium, 已补运行期 panic 可观测性] Supervisor panic/cancellation 安全**
- 当前 `crates/slab-app-core/src/infra/runtime/supervisor.rs` 已把 `supervise_backend`/`supervise_backend_startup_retry` 包在 `spawn_supervisor_task` monitor 中：非 shutdown 场景下的 task panic/cancel 会把该 child 的 service backends 标为 `Unavailable`，并通过 `consecutive_failures` 记录 crash-loop 信号。
- 已新增 panic fake child 直接测试：`supervisor_task_panic_marks_backend_unavailable` 驱动 `wait_for_exit()` panic，断言状态从 `Ready` 转为 `Unavailable`、`consecutive_failures == 1`、`last_error` 记录 supervisor task panic。
- 剩余风险：当前修复选择 fail-closed 标记 unavailable，不自动重建已经 panic 的 supervisor task；如果后续产品要求 supervisor 自愈，需要单独设计 task-level restart 和 backoff，而不是把它混入 child restart 路径。

**G16. [Medium-High, 本轮新盲区] 跨进程 JSON-RPC/gRPC 传输恢复：mid-stream drop、partial framing、backpressure**
- tonic streaming 响应 mid-stream drop（chat token 流）、WS JSON-RPC（`bin/slab-js-runtime`/`bin/slab-python-runtime` mod.rs 各 0 测试）partial-frame/reconnect、`infra/plugin_runtime/sidecar.rs`（619 LOC，仅 2 测试）method-call 中途断连。
- 无测试模拟传输 mid-RPC drop 并断言调用方拿到确定性错误而非 hang。
- 建议：加 fake transport 注入 mid-stream drop。

**G17. [Medium] memories fs/git 持久化无并发写/部分失败测试**
- `crates/slab-agent-memories/src/fs.rs:19-49` 多个非原子 `fs::write` + `remove_stale_summaries`；中断→半写状态。`git.rs:23-42` `git add -N .` 后 `git diff`，无 git-not-installed 或 mid-sequence 失败测试。

**G18. [Medium] ToolRouter unregister / name-collision 路径**
- `crates/slab-agent/src/tool.rs:93` unregister 仅 :1570 测一次且无 overwrite 断言；`slab-agent-tools/src/lib.rs:103-109` MCP proxy 重名 `continue` 分支无测试。

**G19. [Medium] slab-agent-tracing 14 pub fn 仅 5 测试**
- 多 sink fan-out、嵌套 depth context、`tool_call_output`/`system_prompt_injected` payload JSON shape、敏感字段 redaction 未测。tracing 记录完整 tool args/outputs，payload 处理未测有泄漏敏感 args 入日志风险。

### 3.5 Frontend（TS）盲区

**G20. [High, confirmed] `ui-state-storage.ts`（debounce/coalescing/404/error）零直接测试——到处被 mock 掉**
- `packages/slab-desktop/src/store/ui-state-storage.ts:1-110`：`scheduleWrite`（250ms debounce + `pending.resolve` 数组合并）、`flushWrite`（PUT 失败 try/catch + finally drain）、`getItem`（404→null）、`removeItem`（cancel pending timer）。4 个 store 测试全 no-op mock 掉。
- 严重级纠正：High→**Medium-High**（concurrency/coalescing 面确实复杂且 bug-prone，但单用户 desktop 影响面有限；本轮因"回归无任何保护"维持 Medium-High 倾向）。
- 建议：`ui-state-storage.test.ts` 用 `vi.useFakeTimers()` + mock apiClient，断言 N 次 setItem 合并为 1 次 PUT、404→null、removeItem 取消 pending、PUT/DELETE 失败被吞。

**G21. [High, confirmed] `useWorkspaceUiStore.ts`（102 LOC，3-way merge + partialize + onRehydrateStorage）零测试**
- `packages/slab-desktop/src/store/useWorkspaceUiStore.ts:62-102`。是 7 个 store 中唯一无 co-located 测试的。
- 建议：镜像 sibling store 测试，覆盖 patchWorkspaceState create/update/default-merge、empty-rootPath no-op、多 workspace 隔离、onRehydrateStorage error→hasHydrated(true)。

**G22. [High → Low, 已补当前 hooks 直接测试] `src/hooks/`（6 hooks, 324 LOC）核心状态 hooks 与薄包装 hooks 均已有直接覆盖**
- `use-global-header-meta.ts`(154)、`use-persisted-header-select.ts`(73)、`use-file.ts`(41)、`use-desktop-platform.ts`(34)、`use-tauri.ts`(13)、`use-mobile.ts`(9)。
- 当前已新增 `packages/slab-desktop/src/hooks/__tests__/use-persisted-header-select.test.ts`，覆盖 hydration 后 fallback、stale persisted value、disabled-option clearing、setter→store；已新增 `use-global-header-meta.test.tsx`，通过真实 `GlobalHeaderProvider` 覆盖 page meta/control/search 的注册、更新与清理。
- 当前已新增 `use-file.test.ts`、`use-desktop-platform.test.ts`、`use-tauri.test.ts`、`use-mobile.test.ts`，覆盖 web/Tauri 文件选择、platform/userAgent 分支、Tauri internals 检测、mobile breakpoint 与 effect-time 初值契约。
- 剩余：本项按当前 hook surface 已收口；若后续这些 hooks 承载更多环境分支，应继续 co-located 补测试。

**G23. [Medium → Low, 已补直接测试] `assistant-agent-state.ts`（158 LOC）核心状态辅助函数已有覆盖**
- 当前已新增 `assistant-agent-state.test.ts`，覆盖 `toAgentConfig` 的条件 preset 展开与 `reasoning_effort`、`updateLastAssistantMessage` 仅更新最后一个 unfinished assistant message 的行为、`withThoughts` 的 last-assistant attach 与 fallback append loading shell。
- 同一套测试还覆盖了 `agentResponsesWebSocketUrl` / `agentResponsesSseUrl` 的协议改写，以及 `agentEventKey` malformed JSON/null 路径与 `serverMessageThreadId` 的 thread id 提取。
- 剩余：本项当前主要剩集成级风险，即这些 helper 与 `use-assistant-agent.ts` 的联动是否被更高层测试覆盖；helper 自身的纯逻辑盲区已基本收口。

**G24. [Medium] Page index.tsx 组件（~1797 LOC）无单元覆盖**
- 仅 `assistant-markdown.tsx`（叶子）被 render。重逻辑内联处抽到 lib（如 `assistant-page-state.ts` 153 LOC，亦未测）是可测 seam。

**G25. [Medium-High → Low, 已补 locale parity 与 placeholder 验证] i18n 完整性已有基础监管**
- 当前已为 `packages/slab-i18n` 新增 `vitest.config.ts`、`src/__tests__/locale-parity.test.ts` 与 package-level `test`/`test:run` 脚本；root `vitest.config.ts` 与 `package.json:test:frontend` 也已纳入 `i18n` project。
- `locale-parity.test.ts` 会递归展开 `en-US` / `zh-CN` 的运行时 locale tree，断言 leaf path 集合一致，并逐项校验 placeholder 形状一致（如 `{{name}}` 不得漂移成 `{{名称}}`），从而捕获缺 key、嵌套 drift、stale key 与 ICU placeholder 不匹配。
- 纠正：`zh-CN` 原本已经通过 `satisfies LocaleSchema` 继承 `en-US` 的编译期结构约束，因此真正缺的是**运行时 guard** 与测试入口，而不是完全没有任何 shape protection。
- 剩余：当前覆盖的是 locale 完整性，不是组件级翻译渲染/回退策略；若后续语言切换逻辑继续扩展，仍应在 `@slab/desktop` 补端到端或集成级断言。

### 3.6 Tauri / Desktop IPC 层（本轮新盲区）

**G26. [High, 本轮新盲区] Tauri command 层 31 个 `#[tauri::command]` 几乎未测**
- `bin/slab-app/src-tauri/src/workspace.rs`（16 command, 1 test）、`workspace_file_ops.rs`（4, 0）、`terminal.rs`（1, 0）。仅 `plugins/mod.rs`（6）与 `paths.rs`/`setup/api_endpoint.rs` 有意义覆盖。
- 31 个 command 是 renderer 调用的特权 IPC 面（读写删文件、起终端、跑 git、管 plugin），其 wrapper 本身无测试，且委托的 `WorkspaceService::*` 路径包含保证**在此层未测**。
- 建议：command 层加 round-trip 测试，断言路径包含与权限。

**G27. [Medium-High → Low-Medium, 已补关键契约测试] `run_console_command` shell-out 已有 cwd / quoted path / 相对写入守护**
- 当前已在 `crates/slab-app-core/src/domain/services/workspace/mod.rs` 补 async 单测，直接断言 shell 进程的 `cwd` 落在 workspace root、带空格的 quoted relative path 能在当前平台 shell 下被正确读取、相对写入会落到 workspace 目录内。
- 纠正：这里的 `run_console_command` 设计本来就是**显式 shell passthrough**，允许 first-party workspace UI 执行任意 shell 命令；因此“command-injection”不是与实现意图独立的 bug 类别，真正需要守护的是 `cwd` 契约、平台 quoting 行为与调用边界。
- 剩余：当前仍没有更高层的 Tauri/HTTP wrapper 测试来证明 command surface 的权限边界，也没有对超长输出/timeout 的端到端断言；若要把风险继续压低，应在 `G26/G31` 层补 wrapper 级回归。

**G28. [Medium, 本轮新盲区] Test harness 自身的 flakiness/race 面未审计**
- `bin/slab-server/tests/support/slab-server.ts` 与 `fullstack-dev.ts` free-port `listen(0)`、process-tree kill、log ring buffer **无 retry、无 hermetic temp 隔离**；`fileParallelism:false` 仅 browser/server project；TS 侧无 per-test timeout 分层（除 slab-server 120s）；无 flaky marker 约定（grep `@flaky|retry` 在真实测试上零命中）。
- 风险：首个 port-race/process-kill race 会阻塞整个套件。
- 建议：引入 flaky 隔离约定 + per-project timeout 分层。

### 3.7 协议 / 契约（carry-forward）

**G29. [High → Medium, 已补 apply_patch 契约核心覆盖] slab-proto 协议测试仍偏单向**
- 当前已修正并覆盖 `apply_patch` 核心契约：`ApplyPatchOperation`/`ApplyPatchOperationParam` 从错误的 Rust variant-name tagged enum 改为真实 wire shape 的 untagged enum，使用 payload 内部 `type: create_file/delete_file/update_file` 作为判别；新增 create/delete/update round-trip、unknown operation、missing field、type mismatch 负例测试。
- 4 个 apply_patch status `Display` 映射已补 `status_display_matches_wire_values`。
- `skills.rs` 的 `SKILL_CONTENT_RAW==b"string"` / `SKILL_VERSION_CONTENT_RAW==b"string"` 假测试已删除；当前模型层没有 raw-body 类型可做有意义反序列化断言。
- 纠正：crate-wide `deny_unknown_fields` 不适合作为 OpenAI 兼容模型的默认策略；OpenAI API 会增量扩展字段，模型层应默认容忍 unknown fields。若某个内部-only 契约需要 strict unknown-field rejection，应在该具体类型上单独加并测试。
- 剩余建议：继续按高风险 API 面补 round-trip 与 negative/missing-field/type-mismatch 测试，优先 Responses event union、tool call union、chat message content union。

**G30. [High] 6 个 gRPC handler 文件零测试（carry-forward confirmed）。**

**G31. [High] 13/16 v1 HTTP route handler 无 inline 单测；14 个 schema.rs 零测试；crate 无 Rust integration-tests dir。**

**G32. [High → Low-Medium, 已补 transport glue 直接测试] WebSocket JSON-RPC plugin dispatch（两个 `api/jsonrpc/mod.rs`）已有 mod 测试**
- 当前已在 `bin/slab-js-runtime/src/api/jsonrpc/mod.rs` 与 `bin/slab-python-runtime/src/api/jsonrpc/mod.rs` 新增直接测试，覆盖：`runtime.ready` 通知形状、`plugin.call` 请求经 `serve_reader` 分发并返回 JSON-RPC success response、以及 malformed JSON payload 的错误响应。
- 这一步闭合的是此前“**transport adapter 本身零测试**”的盲区；现有测试不再只依赖更高层的 runtime/integration 间接覆盖。
- 剩余：当前仍未逐项覆盖 no-id request ignore、`jsonrpc != 2.0`、`drain_outbound` writer error/flush error，以及 UDS/stdout wiring 的更细分支；结构性重复（R3，抽 `crates/slab-plugin-jsonrpc`）也仍未处理。

**G33. [High] runtime_to_status 仅覆盖 5/20 RuntimeError variant**（06-08 "3/13 CoreError" 双重错误，已纠正）。Scheduler admission 仅 2 测试；inference_lease vs management_lease mutex、poison/Busy/Timeout/next_seq 并发未测。Orchestrator 状态机 + ResultStorage ~1 测试；`wait_stream` 5-branch match 未测。

### 3.8 TS 测试基础设施自测（carry-forward）

**G34. [Medium → Resolved] `vitest-rust-reporter` unit project 已纳入默认前端测试入口**
- 当前 root `vitest.config.ts` 已注册 `packages/vitest-rust-reporter/vitest.unit.config.ts`，`package.json:test:frontend` 也已加入 `--project vitest-rust-reporter-unit`，因此 reporter unit tests 不再是“默认不跑”的孤岛。

**G35. [High → Low-Medium, 已补核心 unit coverage] Reporter 核心模块已有直接测试**
- 当前已新增 `command.unit.test.ts`、`runtime/report.unit.test.ts`、`project.unit.test.ts`、`utils.unit.test.ts`，覆盖：
  - `command.ts`：stdout/stderr 收集、non-zero exit、timeout、missing executable / `exitCode=null`
  - `report.ts`：`infrastructureError` heuristic、`if-available` 下的 missing `llvm-cov`、fatal coverage failure、成功 coverage parse、缺失 reporter options
  - `project.ts`：默认 project option 解析与显式 override
  - `utils.ts`：`trimOutput`、`buildCoverageGroupName`、`createCoverageMetric(count=0)`、format helpers
- 剩余：runtime 投影测试 `rust.test.ts` 仍主要依赖整体验证而非更细粒度的 task registration 断言，但最核心的 command/report/project/utils 逻辑已不再裸奔。

**G36. [Low → Resolved] `slab-plugin-ui` 已有 ABI smoke test 与 test script**
- 当前已为 `packages/slab-plugin-ui` 新增 `vitest.config.ts`、`src/index.test.ts` 与 package-level `test` / `test:run` 脚本；测试会验证稳定 re-export surface 和 `cn` helper 的基本可用性。

**G37. [Medium → Low-Medium, 已清理历史基线噪音] Visual regression 仍主要依赖 snapshot-PNG equality，但 orphan/legacy 基线已收敛**
- 当前已删除两类历史遗留基线：没有对应测试文件的 `chat-page.browser.test.tsx` 截图目录，以及旧的自动命名 `*-1.png` 基线；当前仓库只保留现行 `toMatchScreenshot(...)` 命名约定下仍被测试引用的 PNG。
- 剩余高权重问题仍在：baselines 依旧明显偏向 Win32（darwin 仅 setup-page，linux 仍空白）、无 diffThreshold、CI 默认不跑 browser，因此 visual regression 仍主要是 local-only 信号。
- `a11y` 覆盖仍是空白：当前前端测试源里依旧没有成体系的 `role` / `aria-*` / 可访问性断言。

**G38. [Medium] `slab-windows-full-installer`（1118 LOC）零测试。**

**G39. [Low → Low-Medium, 已消除 package.json 级别双写漂移] `test:frontend` 仍是有意收窄的子集，但前端 project 列表已集中到共享 Vitest config**
- 当前 `package.json:test:frontend` 已改为 `vitest run --config vitest.frontend.config.ts`；`vitest.frontend.config.ts` 与 root `vitest.config.ts` 共同复用 `vitest.projects.ts` 中的 `frontendVitestProjects` / `allVitestProjects`，不再在 `package.json` 里硬编码 7 个 project 名。
- 纠正：此前问题的核心不是“子集本身错误”，而是**子集列表分散在脚本字符串里、易与 root workspace projects 漂移**；这一层双写现在已收敛到单一项目清单。
- 剩余：`test:frontend` 依然是有意收窄的入口，仍不覆盖 browser/server/rust runtime projects；若后续默认前端测试边界要扩展，仍需显式调整 `frontendVitestProjects`。

### 3.9 通用 infra 缺口

**G40. [Medium → Low-Medium, 已补 header/error guards] `slab-utils/lsp.rs` 关键 framing 错误路径已有覆盖**
- 当前已补 `read_lsp_stdio_message` 的 header 超 8192 字节、UnexpectedEof partial header、缺失 `Content-Length`、非数字 `Content-Length`、非 UTF-8 header 这些错误路径测试；happy round-trip 与大小写 `Content-Length` 也仍有覆盖。
- 剩余：尚未直接覆盖 body 非 UTF-8、body 提前 EOF、以及更细粒度的 header 变体组合，但最关键的 8KB guard 与基础 framing 错误面已不再裸奔。

**G41. [Medium] `slab-utils/loader.rs` native-library 加载从未端到端测**
- 13 个测试全用 closure mock。`open_native_library` 的 Windows `LOAD_LIBRARY_SEARCH_*` flags（DLL planting 防护，line 62-67）零覆盖。

**G42. [Medium → Low-Medium, 已补主要 env parsing guards] `slab-config/app_config.rs from_env` 关键 override 与解析 edge 已有直接测试**
- 当前已补 `SLAB_DATABASE_URL` override、`SLAB_LOG_JSON` / `SLAB_CLOUD_HTTP_TRACE` truthiness、`SLAB_ENABLE_SWAGGER` 反语义、`SLAB_QUEUE_CAPACITY` / `SLAB_BACKEND_CAPACITY` 的非数字/overflow fallback、以及 `SLAB_ADMIN_TOKEN` / `SLAB_CORS_ORIGINS` / `SLAB_TRANSPORT` 覆盖。
- 纠正：此前审计里把 `"TRUE"` 也归入“静默 false”是错误的；当前实现对 `"true"` 使用 `eq_ignore_ascii_case`，因此 `"TRUE"` 会被接受，而 `"yes"` / `"on"` 仍会解析为 false。`SLAB_QUEUE_CAPACITY=0` 当前也会被接受为合法值，而不是回退默认值。
- 剩余：现有实现对 malformed capacity 仍是 silent fallback 到 default，这个行为现在已有测试但是否应改成显式报错，仍是产品/配置契约层面的后续决策；此外 `SLAB_BIND` 以外的更多 endpoint/path env 仍未逐项做 exhaustiveness 覆盖。

**G43. [Medium → Resolved] `Config::from_env` 测试脚手架已移除进程全局 `unsafe` env 变更**
- 当前 `crates/slab-config/src/app_config.rs` 已将解析逻辑下沉到可注入的 `from_env_source` 路径，生产代码仍走真实 `ProcessEnv`，测试则通过局部 `HashMap<String, String>` fixture 驱动，不再依赖 `env_lock` / `EnvGuard`、`unsafe { std::env::set_var/remove_var }` 或可观察的全局串行。
- 同一轮调整还把 `temp_settings_path()` 的手写临时目录改成 RAII `tempfile::tempdir()` fixture，避免 panic 时遗留 test temp 目录。
- 纠正：此前审计对这个问题的风险判断成立，但当前代码已不再保留该脚手架，因此本项应从“仍存”改为已闭合。

---

## 4. 行动指南与下一步计划 (Action Items)

### 4.1 优先级 Top 10（按 ROI / 风险排序）

| # | 行动 | Owner | Effort | 闭合的 finding |
|---|------|-------|--------|----------------|
| 1 | **修 auth fail-open**：`auth.rs` 非 loopback 无 token fail-closed、空/whitespace token fail-closed + 直接单测；保留现有 loopback 本地例外 | slab-server | S | G3 |
| 2 | **`resolve_path` + `LocalExecutorFileSystem` 表驱动越界测试**（symlink/UNC/绝对路径/`..` 残留） | slab-file + app-core | M | G6, G31 |
| 3 | **`redact_secrets` + slab-config secret 表驱动测试/修复**（`BasicToolRiskAnalyzer` 直接测试已补） | slab-agent + memories + config | M | G4, G5, G6 |
| 4 | **提取 `migrated_pool()` helper，删除 7 处手写 DDL**（已完成，并修复 `model_config_state` 外键 drift） | slab-app-core | M | R1, G11 |
| 5 | **`rpc/client.rs` retry/backoff 决策提取为纯函数并单测**（已完成：transient/status/attempt 边界覆盖） | slab-app-core | M | G13 |
| 6 | **Repository round-trip 套件**（已完成首批：`media_task.rs` image/video/audio insert→update→get/list） | slab-app-core | L | G14 |
| 7 | **Supervisor panic-safety 测试/修复**（已完成：panic fake child → unavailable 标记 + crash-loop 计数） | slab-app-core | M | G15 |
| 8 | **TS hooks/store 测试**（已完成：`ui-state-storage` fake timers + `useWorkspaceUiStore` store 行为；当前 hooks surface 直接覆盖已补齐） | slab-desktop | M | G20, G21, G22 |
| 9 | **Coverage threshold + `lint-staged` 配置**（核心包阈值，pre-commit 真正生效）；smoke 补"每条 executable 必被 flow 实调"meta-test，并明确外部目标模式下 admin-auth smoke 的跳过策略 | release/CI + slab-server | S | G2, G4, G3 |

### 4.2 分阶段路线图

**Phase 1安全边界 + 持久化加固**
- 已完成行动 #2（路径越界）、#3（risk/redaction/secret）、#4（migrated_pool）、#7（supervisor panic）。
- 已补 G11 migration post-apply 断言矩阵（优先覆盖 agent_memories/media_tasks）；rollback round-trip 因当前 SQLx simple migration 目录不能混用 reversible down 脚本，改为架构性剩余项。
- 已补 G29 apply_patch 核心 negative/round-trip 测试并删除 skills 假测试；crate-wide `deny_unknown_fields` 已纠正为不适合 OpenAI 兼容模型的默认策略，剩余为 Responses/tool/chat unions 的扩大覆盖。

**Phase 2覆盖深度 + 冗余根因治理**
- 已完成行动 #5（RPC retry）、#6 首批 repository round-trip（media_task image/video/audio）、#8 首批 TS store 覆盖（ui-state-storage/useWorkspaceUiStore）、当前 hooks surface 覆盖（usePersistedHeaderSelect/useGlobalHeaderMeta/useFile/useDesktopPlatform/useTauri/useMobile），以及 `assistant-agent-state.ts` 的纯逻辑覆盖。
- 治理**冗余根因**：建立共享 test-support——Rust 侧 `slab-test-utils`（含 `migrated_pool`/`run_git_test`/`build_pack_bytes`/fixture builders）；TS 侧每个 package 建 `src/test/` + `__mocks__/`（R12/R13/R5/R18 收敛）。
- 已为两个 `api/jsonrpc/mod.rs` 补 transport glue 直接测试（G32）；剩余是抽取 `crates/slab-plugin-jsonrpc` 收敛重复（R3）。
- 6 gRPC handler boilerplate 抽取（R15/R16）。
- 已完成 i18n parity 脚本与 `test:frontend` 接线（G25），并清理 G37 的 orphan/legacy PNG 基线；剩余为 visual/browser 覆盖结构升级与 a11y 起步。

**Phase 4（持续，工具链升级）**
- 已完成 `Config::from_env` 的 env-source 注入改造，消除 `env_lock` / `EnvGuard` unsafe（G43）。
- proptest/criterion 推广到 parser/config/migration/state-machine crate。
- TS 测试 harness 加 flaky 隔离约定 + per-project timeout 分层（G28）。

### 4.3 与 06-08 审计的关系总结
- **确认并升级**：slab-agent 标杆地位维持；gRPC handler / auth middleware / proto 契约问题全部 confirmed 并延续。
- **纠正 06-08**：migration 覆盖率（"2/19 forward" 错，实为"全 forward-apply-tested，~3-4 post-assert，0 rollback"，High→Medium）；runtime_to_status（"3/13 CoreError" 双错，实为 5/20 RuntimeError）；**smoke registry（首轮臆测的"it.todo 占位冒充覆盖"被推翻——实为有 drift-enforcement 测试守护的纪律良好覆盖率地图）**。
- **06-08 的最大盲区（本轮补齐）**：TS 侧 CI 断层（G1）、安全边界回归保护（G2/G3/G4/G5）、Tauri IPC 层（G30/G31）、冗余的系统根因（无共享 test-support crate / 无 TS `src/test/`）。
- **整体评级 C+ → C（弱 C+）**：Rust 核心小幅改善被 TS/CI/安全的系统性盲区抵消。Phase 1 完成后即可稳定回到 C+，Phase 2 完成后冲击 B。

---

*本报告由 15 路子系统并行审计（首轮 5 路完成 + 第二轮限流重审 9 路 + smoke 子系统主审计员直读源码落地）+ 24 项 High-severity 对抗式验证 + 2 路完整性/跨包重复 critique 综合而成。所有结论均要求引用具体 file:line 证据；被对抗式验证推翻或高估的发现已剔除/降级。*
