# Test Capability Audit Report (2026-06-17)

> **审计方法**: 15 个子系统并行审计（10 个本轮新审 + 5 个首轮已完成且经对抗式验证的 carry-forward），覆盖 Rust 与 TypeScript 双栈，新增**冗余维度 (Redundancy)** 与**对抗式验证 (Adversarial Verification)** 两个维度。24 项 High-severity 发现经独立 verifier 核实，被推翻的剔除、被高估的降级。用户当前打开的 `bin/slab-server/tests/smoke/server-api/todos.ts` 所在的 smoke 子系统，因两次 rate-limit 失败，由主审计员**直接读源码落地核实**（见 §3.1 G3）。

## 1. 执行摘要 (Executive Summary)

仓库规模：约 42 个 Rust crate（最大 `slab-app-core` ~42.8k LOC、`slab-proto` ~18.8k、`bin/slab-js-runtime` ~15.6k、`bin/slab-runtime` ~12.4k）；TS 侧最大为 `packages/slab-desktop` ~30.5k LOC。测试基础设施：8 个 vitest project、cargo workspace、v8 coverage（**未设置阈值门槛**）、CI 矩阵 ubuntu/macos/windows、54 个视觉基线、`bin/slab-server/tests` 结构化 smoke registry。

**相对于 2026-06-08（评级 C+）的关键变化：**

- 上一轮过度聚焦 Rust 覆盖率，**严重低估了 TS 侧盲区**——本轮纠正：TS 侧最大的风险不是覆盖率本身，而是 **CI 根本不运行任何 Vitest 套件**。
- 新增冗余维度后，确认 **Rust 与 TS 双侧的测试脚手架 (test scaffolding) 大面积复制**，且**跨 crate 缺乏共享 test-support crate** 是几乎所有冗余的根因。
- **smoke registry 被正名**：`bin/slab-server/tests` 的 `executableSmokeOperations / todoSmokeOperations / futureCompatibilityScenarios` 不是"占位冒充覆盖"，而是一套**有 drift-enforcement 测试守护的、纪律良好的覆盖率地图**（68 executable / 0 todo / 16 future，详见 G3）。真正的风险是这套 TS smoke 不在 CI 里跑。
- 上轮部分发现经对抗式验证后被**修正/降级**（如 migration 覆盖率被高估、`slab-server` harness 重复被高估、auth timing-attack 被高估）。

**三大首要风险（各一句）：**

1. **CI 仅 gate `cargo test`，全部 ~78 个 TS 测试文件 / ~430 个 test block 在 push/PR 上零执行**——任何 TS 回归可静默并入 main（包括上面那套优秀的 smoke drift-enforcement 测试）。
2. **安全边界（路径越界 / auth fail-open / 风险分类）回归保护极弱**：`resolve_path` 仅 1 个 escape 用例、`auth_middleware` `None` token fail-open 无测试、`BasicToolRiskAnalyzer` 零直接测试。
3. **Repository 层 + 6 个 gRPC handler 文件 + RPC retry/backoff 几乎零测试**，而 `rpc/client.rs` 是模型运行时可靠性的承重路径。

**评级（保留字母制，便于与 06-08 对比）：**

| 维度 | 06-08 | 06-17 | 说明 |
|------|-------|-------|------|
| Rust 单元覆盖（核心 crate） | C | **C+** | `slab-agent` 仍为标杆；app-core/desc/rpc 仍是黑洞 |
| TS 单元覆盖（desktop/api/plugin） | 未评 | **C-** | 量不小但 hooks/页面/store 覆盖空洞 |
| CI 强制执行 | C | **D+** | cargo test 跑、TS 全不跑（下调）|
| 冗余控制 | 未评 | **C-** | 跨 crate 无共享 test-support 是系统性的 |
| 安全/关键路径回归保护 | C | **C** | carry-forward 的 gRPC/auth 问题依旧，新增 risk/path 边界 |
| **综合** | **C+** | **C（弱 C+）** | TS 侧盲区与 CI 断层拉低；Rust 核心小幅改善 |

---

## 2. 重复与冗余发现 (Identified Redundancies)

> 编号 R1.. 按影响面排序：先列**跨 crate 根因型**，再列 **package 内**，最后是低 ROI 的局部重复。路径为仓库根相对路径。

### 2.1 跨 crate / 跨 package 根因型（优先治理）

**R1. 7 个 repository 测试手写 `CREATE TABLE IF NOT EXISTS` DDL，而非运行真实 migration —— 存在 schema drift 风险 [High]**
- 涉及文件：
  - `crates/slab-app-core/src/infra/db/repository/chat.rs:91,103`（chat_sessions, chat_messages）
  - `crates/slab-app-core/src/infra/db/repository/plugin.rs:287`（plugin_states）
  - `crates/slab-app-core/src/infra/db/repository/ui_state.rs:81`（ui_state）
  - `crates/slab-app-core/src/infra/db/repository/model_config_state.rs:98,119`（models, model_config_state）
  - `crates/slab-app-core/src/infra/db/repository/{model,model_download,task}.rs`（CREATE TABLE）
- 对比正解：`crates/slab-app-core/src/infra/db/repository/mod.rs:39,78`（`SqlxStore::connect` → `migrate!`），以及 `model.rs:196` 已存在的 `migrated_pool()` 半成品 helper。
- 根因 & 风险：这些测试断言的行为所依赖的表结构**不是生产结构**——任何 migration 增加列/CHECK 约束后，仓库测试仍按陈旧 schema 通过。子系统审计只点到"样板重复"，**跨切面升级点是 schema drift**。
- 建议：在 `src/test_support.rs` 暴露 `pub(crate) async fn migrated_pool() -> AnyStore`，删除所有手写 DDL 块。此修复同时**免费闭合 migration 覆盖盲区**（见 G15）。

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

### 3.1 CI 与强制执行（最高优先级）

**G1. [High, confirmed] CI 不运行任何 Vitest 套件——整个 TS 测试金字塔未强制**
- 证据：`.github/workflows/ci.yml` 仅 `cargo-check`（3 OS）+ `frontend-check`（`gen:api` drift + `build:desktop` typecheck）。grep `.github/workflows/` 对 `vitest`/`bun.*test`/`test:frontend|server|browser|rust` 零命中。
- 影响：~78 个 TS 测试文件 / ~430 个 test block（api, plugin-sdk/cli, desktop jsdom, slab-server smoke/integration, browser visual/e2e）全部 local-only。slab-server smoke 是 TS（`server-api.smoke.test.ts`、`health.integration.test.ts`），故 cargo test 也不覆盖。carry-forward 中"CI 不跑 browser 测试"严重**低估**——是**全 TS 不跑**。
- 建议：新增 CI job 跑 `bun run test:frontend` + `bun run test:server`（server 依赖 cargo job 缓存的 binary）；browser/e2e 拆慢 job。

**G2. [High → Medium, confirmed] Coverage 生成但从不强制——无 threshold gate**
- 证据：根 `vitest.config.ts:16-29` 有 provider/reporter/exclude，**无 `thresholds`**；8 个 project config 全无；`vitest-rust-reporter` 的 `cargo llvm-cov` 只输出 summary，不比较阈值；`ci.yml` 不调用 `test:coverage`。
- 严重级纠正：High→**Medium**（这是 CI/process 加固缺口，不是 active correctness/security 缺陷；本地 coverage 报告仍可用）。
- 建议：核心包（api, slab-plugin-sdk）加 `coverage.thresholds`，CI 失败。

**G3. [正名/纠正] `bin/slab-server/tests` 的 smoke TODO registry 是纪律良好的"覆盖率地图"，而非占位冒充覆盖 —— 但有两处真实盲区 [Medium]**
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
- **真实盲区 2（Low）**：`admin-auth.ts` 注册了 describe 但 `it()`=0（grep 确认），即 admin 鉴权 smoke 为空壳；而 `/v1/settings`、`/v1/setup/*` 等 admin 路由已在 68 条 executable 中声明。建议落实 admin-auth flow。
- **真实盲区 3（设计性，非缺陷）**：smoke 按 README 设计即"stable validation / not-found / error-envelope，而非真实下载模型或推理"——它是 **presence/shape 层**，不是行为/推理覆盖。这是 smoke 的合理天花板，不是 bug；但意味着它**不能替代** §3.3/§3.7 中那些缺失的深集成测试。
- **首要风险（已在 G1 覆盖）**：整套 smoke（含上面那条宝贵的 drift-enforcement 测试）是 TS、且**不在 CI 跑**。一旦入 CI，这套机制才算真正生效。

**G4. [Medium, partial] Pre-commit hook 是 no-op**
- `.husky/pre-commit` 跑 `bun run lint-staged`，但 lint-staged 是 devDep 而仓库**无 lint-staged 配置**（无 `.lintstagedrc`、package.json 无顶层 key）。无配置时 lint-staged 立即 exit 0。`.husky/` 无 pre-push。
- caveat：与 G1/G2 一致地表明**前端质量门形同虚设**。
- 建议：加 `.lintstagedrc`（staged TS 走 oxlint --fix，staged .rs 走 rustfmt）+ 可选 pre-push 跑 `test:frontend`。

**G5. [Medium] 无 nextest / 无 CI 侧 llvm-cov 阈值 / proptest 仅 1 crate 用**
- `scripts/cargo/validate.ts` 跑 plain `cargo test --workspace`；`proptest` 仅 `slab-types/Cargo.toml` 声明；无 criterion benches。

### 3.2 安全 / 关键路径回归保护（本轮新增的核心盲区）

**G6. [High, 本轮新盲区] Workspace 路径越界测试极薄——主安全边界**
- `crates/slab-file/src/system.rs:149` `resolve_path` 仅 1 个 escape 测试（`resolve_path_rejects_workspace_escape` @ line 461，只喂 `../outside.txt`）。未测：symlink 越界、Windows 绝对路径、UNC `\\?\`、write-to-new-file 路径（`existing_ancestor(parent)` 仅校验、未最终 canonicalize，line 170-175）、parent-of-parent 越界、`..` 在 normalize 后的残留。
- `crates/slab-app-core/src/domain/services/workspace/file_system.rs` `LocalExecutorFileSystem`（170 LOC，实际接到 Tauri `WorkspaceService`）**0 测试**，且 write 时不查 `policy`/`writable_roots`/`denied_paths`。
- 建议：表驱动越界用例（symlink/UNC/绝对路径/`..` 残留），并为 `LocalExecutorFileSystem` 加 round-trip 测试。

**G7. [High, 本轮新盲区 + carry-forward 升级] Auth middleware `None` token fail-open 无测试**
- `bin/slab-server/src/api/middleware/auth.rs:24-26`：`admin_api_token` 为 `None` 时 `match` 返回 `true`——**所有管理路由 (`/backend`,`/settings`,`/workspace`) 对任意调用者开放**。无测试断言空/whitespace token fail-closed。整个文件 0 测试。
- 注意：carry-forward 把"timing-attack [Medium-High]"列为 lead concern，本轮**降级** timing-attack 为 **Low-Medium**（localhost/loopback 单用户 Tauri sidecar token，timing 攻击 largely 理论）；**lead concern 应改为 fail-open-on-None**。
- 建议：加测试覆盖 None token 拒绝、空串/whitespace 拒绝；token 比较改 `subtle::ConstantTimeEq`（防御性，非首要）。

**G8. [High, confirmed] `BasicToolRiskAnalyzer`（唯一 risk impl，gatekeep host approval）零直接测试**
- `crates/slab-agent/src/risk.rs:13-45`：`rm `/`remove-item`/`git reset`/`del ` → High/`destructive_command`；其他 shell → Medium；非 shell → Low。lowercase 处理。
- `tests.rs:840/854/870` 的 approval mock 全部 `_risk:` 丢弃；唯一相关断言 `tests.rs:1786-1794` 只对 echo 命令断言 `risk: Some(_)`（非 null 即过，不断言 level/label）。
- 未覆盖：trailing-space 要求（`rm`/`del` 单独 token 会落到 Medium）、`git reset` 子串匹配副作用、大小写、Medium/Low fallback。
- 建议：risk.rs 内加 `#[tokio::test]` 套件，覆盖每个 High 触发短语 + fallback。

**G9. [High, confirmed] Secret redaction 仅 1 个测试**
- `crates/slab-agent-memories/src/redaction.rs:14-23`：3 个 regex。唯一测试 `redacts_common_secret_shapes` @ :29 只测 `api_key=` 与 `Authorization: Bearer`。未测：`sk-` 前缀（regex 3）、`token=`/`password=`/`passwd=`/`secret:` 变体、大小写、**12 字符下限边界**（11 字符 token 不应 redact 的 false-positive guard）、引号风格、一行多 secret、benign 字符串透传。
- 风险面：`redact_secrets` 在写 `raw_memories.md` 前运行（`phase1.rs:72,112-114`），under-coverage = 数据外泄面。
- 建议：表驱动测试每个 keyword + sk- + 12-char 边界 + no-op 用例。

**G10. [High → Medium, confirmed, 严重级纠正] slab-config secret-redaction contract 实现错误且未测**
- `crates/slab-config/src/pmid_service.rs:265` 设 `secret: secret(pmid)` 但 :269 原样返回 `effective_value`——flag 纯装饰；`secret()` (:940-942) 仅匹配 `server.admin.token`，故 `providers.registry`（`auth.api_key` @ 1469-1470 逐字 clone）、websearch/MCP key 从不标记 secret。
- 严重级纠正：High→**Medium**。`/v1/settings` 路由在 `auth_middleware` 之后（需 admin bearer token），能读 api_key 的攻击者已持有 admin token，可直接读原始 settings 文件——属 defense-in-depth/correctness 缺口，非未授权披露。
- 建议：修 `secret()` 覆盖所有 credential PMID，`build_property` 真正消费 flag 做 mask；加测试断言 view 序列化不含明文。

**G11. [High → Medium, confirmed] Config descriptor get/set 类型校验对 ~100 PMID 无直接测试**
- `crates/slab-config/src/descriptor.rs`（447 行）+ `view.rs`（185 行）均无 `#[cfg(test)]`。`set_setting_value`（descriptor.rs:438-447）包装 `serde_json::from_value`；`view.rs:42-52` u64→i64 `try_from` 对 >i64::MAX 静默落到 f64；`:68-70` NaN/Infinity → Null。
- 严重级纠正：High→**Medium**。核心类型拒绝委托给 serde（久经考验），真正未测的是两个**静默强转** edge case + 缺 round-trip，不是"每个 PMID 都有未测校验"。
- 建议：表驱动测试 type-mismatch→BadRequest、round-trip、u64-overflow、NaN。

**G12. [High, confirmed] MCP 工具 `execute()` 错误/参数校验路径完全未测**
- `crates/slab-agent-tools/src/mcp.rs:47-66`（McpCallTool::execute）、:141-156（McpProxyTool::execute）。4 个 mcp.rs 测试仅断言 name/description/schema；唯一 execute 测试（:215）只跑空 server 列表的 happy path。未测：缺 `server_name`/`tool_name`、remote call_tool 失败→`AgentError::ToolExecution`、McpProxyTool::execute。
- 严重级纠正：High→**Medium**。`string_arg` 的 AgentError 逻辑本身在 `args.rs:18-28` 已单测；底层 `McpClient::call_tool` happy/ServerNotFound 在 `slab-mcp/src/client.rs:257-279` 已测。未测的是 mcp.rs 薄包装层（机械 `.map_err()` + serde_json::to_string，serde 错误分支基本不可达）。
- 建议：加 fake McpClient（或 trait seam）驱动缺参/未知 server/remote 错误/proxy 成功。

**G13. [High, confirmed] `infra/rpc/client.rs` gRPC retry/backoff/timeout 零测试**
- `LOAD_MODEL_MAX_ATTEMPTS=3`/250ms、`UNARY_RPC_MAX_ATTEMPTS=2`/100ms、`RPC_REQUEST_TIMEOUT=30min`、`is_transient_runtime_status` 字符串匹配 predicate、各 loop 末尾 `unreachable!()`。`gateway.rs`/`mod.rs` 同样 0 测试；`codec.rs` 仅 encode 测试。
- 鉴于 3 个文件全无测试 + 承重可靠性路径，维持 High。
- 建议：提取 retry 决策为纯函数单测，或用 fake tonic channel；最低限度测 RequestIdInterceptor 与 retry-count 数学。

### 3.3 核心业务逻辑 / 持久化

**G14. [High, confirmed] Repository 层 ~110 CRUD 方法仅 19 测试**
- `config.rs=0, media_task.rs=0（24 trait 方法全无测）, session.rs=0, chat.rs=1, agent.rs=1, plugin.rs=1, ui_state.rs=1, model_config_state.rs=1`。`model.rs=2, model_download.rs=5, task.rs=6` 是亮点模板。
- 域服务测试 43 个大多用 mock，仅 3 个真正打 SQL（catalog.rs=2, download.rs=1）。
- caveat：已有单测并非全 trivial（chat.rs 验 UNIQUE 回滚、agent.rs 验字段保留、ui_state.rs 验 round-trip）——"CRUD round-trip 大多未验"略过述。
- 建议：按 `model_download.rs` 模板补 round-trip（insert→get→update→delete→get-none）+ unique-violation + 并发写。

**G15. [High → Medium, partial, 严重级纠正 + 头条措辞纠正] Migration 覆盖率**
- 真相（已 verify）：19 个 migration 文件**全部有 forward-apply smoke 覆盖**（`storage_value_checks` @ model.rs:275、`turn_state_upsert` @ agent.rs:313、`connect_configures_sqlite_concurrency_pragmas` @ mod.rs:77 均 `migrate!` 全量）。仅 **~3-4 个**有 post-apply schema/constraint 断言；**ZERO rollback** 测试。
- **纠正 06-08 与首轮原始发现的"2 of 19 forward-tested"——错误**。准确表述：全部 forward-apply-tested，~3-4 有 post-apply 断言，0 rollback。
- 严重级：High→**Medium**（单用户 SQLite 桌面应用，blast radius 是本地 DB，可 re-init 恢复）。
- 建议：migration 测试矩阵（PRAGMA table_info 断言每个 migration 后的列/约束）+ 每个 migration 一条 forward→rollback round-trip，优先 `agent_memories`/`media_tasks`（含 CHECK 约束）。

**G16. [Medium] model_auto_unload.rs 只测纯 helper，async eviction state machine 未测**
- 786 LOC，4 个测试仅覆盖 `is_under_memory_pressure` 等纯函数。`ModelAutoUnloadManager` loop（轮询、触发 eviction、与 supervisor 协调、`max_pressure_evictions_per_load` cap、`RuntimeMemoryPressure` 恢复）未测。
- 建议：用 `RecordingRuntimeGateway` + fake memory snapshot 驱动一整轮 pressure→evict→recover。

**G17. [Medium] 大型 service 文件零直接测试**
- `domain/services/chat/local.rs`（671 LOC，0）、`plugin/validation.rs`（655 LOC，0，**安全相关输入边界**）、`model/runtime.rs`（681）、`model/download.rs`（766）、`model/catalog.rs`（640）。
- 建议：优先 `plugin/validation.rs`（输入校验安全边界）与 `chat/local.rs` 错误路径。

**G18. [Medium] `agent/memory.rs`（920 LOC）仅 3 测试**
- recall/ranking、source-kind routing、与 `agent_memories`/`agent_memory_usage_source_kind` 新 migration 的持久化交互薄。

### 3.4 并发 / 状态机 / 取消安全

**G19. [Medium-High, 本轮新盲区] Supervisor panic/cancellation 安全——panic 的 supervised task 被静默吞**
- `crates/slab-app-core/src/infra/runtime/supervisor.rs:597-601`：shutdown 时 `if let Err(error) = handle.await { warn!(...) }`——panic 的 `supervise_backend`/`supervise_backend_startup_retry`（spawn @ 531/543）产生 `JoinError` 仅被 log，**不标记 backend unavailable、不重启、不升级**。该 backend 从 supervision 静默消失。无测试驱动 panic fake child 断言 supervisor 状态。
- 这是 carry-forward "tokio::select! 在 6 个 runtime 文件无 interleaving/cancel/drop-order 测试"在 app-core 侧的具体 failure mode。
- 建议：用 panic 的 fake child 驱动，断言 supervisor 后续状态（unavailable 标记、crash-loop 计数）。

**G20. [Medium-High, 本轮新盲区] 跨进程 JSON-RPC/gRPC 传输恢复：mid-stream drop、partial framing、backpressure**
- tonic streaming 响应 mid-stream drop（chat token 流）、WS JSON-RPC（`bin/slab-js-runtime`/`bin/slab-python-runtime` mod.rs 各 0 测试）partial-frame/reconnect、`infra/plugin_runtime/sidecar.rs`（619 LOC，仅 2 测试）method-call 中途断连。
- 无测试模拟传输 mid-RPC drop 并断言调用方拿到确定性错误而非 hang。
- 建议：加 fake transport 注入 mid-stream drop。

**G21. [Medium] memories fs/git 持久化无并发写/部分失败测试**
- `crates/slab-agent-memories/src/fs.rs:19-49` 多个非原子 `fs::write` + `remove_stale_summaries`；中断→半写状态。`git.rs:23-42` `git add -N .` 后 `git diff`，无 git-not-installed 或 mid-sequence 失败测试。

**G22. [Medium] ToolRouter unregister / name-collision 路径**
- `crates/slab-agent/src/tool.rs:93` unregister 仅 :1570 测一次且无 overwrite 断言；`slab-agent-tools/src/lib.rs:103-109` MCP proxy 重名 `continue` 分支无测试。

**G23. [Medium] slab-agent-tracing 14 pub fn 仅 5 测试**
- 多 sink fan-out、嵌套 depth context、`tool_call_output`/`system_prompt_injected` payload JSON shape、敏感字段 redaction 未测。tracing 记录完整 tool args/outputs，payload 处理未测有泄漏敏感 args 入日志风险。

### 3.5 Frontend（TS）盲区

**G24. [High, confirmed] `ui-state-storage.ts`（debounce/coalescing/404/error）零直接测试——到处被 mock 掉**
- `packages/slab-desktop/src/store/ui-state-storage.ts:1-110`：`scheduleWrite`（250ms debounce + `pending.resolve` 数组合并）、`flushWrite`（PUT 失败 try/catch + finally drain）、`getItem`（404→null）、`removeItem`（cancel pending timer）。4 个 store 测试全 no-op mock 掉。
- 严重级纠正：High→**Medium-High**（concurrency/coalescing 面确实复杂且 bug-prone，但单用户 desktop 影响面有限；本轮因"回归无任何保护"维持 Medium-High 倾向）。
- 建议：`ui-state-storage.test.ts` 用 `vi.useFakeTimers()` + mock apiClient，断言 N 次 setItem 合并为 1 次 PUT、404→null、removeItem 取消 pending、PUT/DELETE 失败被吞。

**G25. [High, confirmed] `useWorkspaceUiStore.ts`（102 LOC，3-way merge + partialize + onRehydrateStorage）零测试**
- `packages/slab-desktop/src/store/useWorkspaceUiStore.ts:62-102`。是 7 个 store 中唯一无 co-located 测试的。
- 建议：镜像 sibling store 测试，覆盖 patchWorkspaceState create/update/default-merge、empty-rootPath no-op、多 workspace 隔离、onRehydrateStorage error→hasHydrated(true)。

**G26. [High, confirmed] `src/hooks/`（6 hooks, 324 LOC）零测试**
- `use-global-header-meta.ts`(154)、`use-persisted-header-select.ts`(73)、`use-file.ts`(41)、`use-desktop-platform.ts`(34)、`use-tauri.ts`(13)、`use-mobile.ts`(9)。
- grep 全 test 套件：所有引用都是 `vi.mock(...)` 替换或 type-only import；`use-desktop-platform`/`use-mobile` 从未被任何测试触及。
- 建议：用 `renderHook()` 测 `use-persisted-header-select`（hydration race、stale selection、disabled-option clearing、fallback）与 `use-global-header-meta`（effect timing、subscription、cleanup）。

**G27. [Medium] `assistant-agent-state.ts`（158 LOC）未测**
- `toAgentConfig`（条件展开 7 preset + reasoning_effort）、`updateLastAssistantMessage`/`withThoughts`（fallback append loading message）、`agentEventKey`（malformed JSON null）、ws/sse URL 协议改写。

**G28. [Medium] Page index.tsx 组件（~1797 LOC）无单元覆盖**
- 仅 `assistant-markdown.tsx`（叶子）被 render。重逻辑内联处抽到 lib（如 `assistant-page-state.ts` 153 LOC，亦未测）是可测 seam。

**G29. [Medium-High, 本轮新盲区] i18n 完整性完全无监管**
- `packages/slab-i18n` 2 locale × ~15 namespace（~1800 LOC）**零 `.test.ts`**、package.json 无 `test` script。无 locale-parity 脚本。spot-check 当前 key 对齐（settings 101/101, workspace 141/141），但无机制捕获：缺 key、嵌套 drift、ICU placeholder 不匹配（`{{name}}` vs `{{名称}}`）、stale key。近期 commit `dfc0b8ba` 表明此面在动。
- 建议：加 parity 脚本对比两 locale 的 key 集合 + placeholder 形状。

### 3.6 Tauri / Desktop IPC 层（本轮新盲区）

**G30. [High, 本轮新盲区] Tauri command 层 31 个 `#[tauri::command]` 几乎未测**
- `bin/slab-app/src-tauri/src/workspace.rs`（16 command, 1 test）、`workspace_file_ops.rs`（4, 0）、`terminal.rs`（1, 0）。仅 `plugins/mod.rs`（6）与 `paths.rs`/`setup/api_endpoint.rs` 有意义覆盖。
- 31 个 command 是 renderer 调用的特权 IPC 面（读写删文件、起终端、跑 git、管 plugin），其 wrapper 本身无测试，且委托的 `WorkspaceService::*` 路径包含保证**在此层未测**。
- 建议：command 层加 round-trip 测试，断言路径包含与权限。

**G31. [Medium-High, 本轮新盲区] `run_console_command` shell-out 无注入/转义/cwd 测试**
- `crates/slab-app-core/src/domain/services/workspace/mod.rs`（5 测试）。在 workspace root 内 shell out，无 command-injection / arg-escaping / cwd containment 测试。仅 desktop E2E（CI 不跑）覆盖。

**G32. [Medium, 本轮新盲区] Test harness 自身的 flakiness/race 面未审计**
- `bin/slab-server/tests/support/slab-server.ts` 与 `fullstack-dev.ts` free-port `listen(0)`、process-tree kill、log ring buffer **无 retry、无 hermetic temp 隔离**；`fileParallelism:false` 仅 browser/server project；TS 侧无 per-test timeout 分层（除 slab-server 120s）；无 flaky marker 约定（grep `@flaky|retry` 在真实测试上零命中）。
- 风险：首个 port-race/process-kill race 会阻塞整个套件。
- 建议：引入 flaky 隔离约定 + per-project timeout 分层。

### 3.7 协议 / 契约（carry-forward）

**G33. [High] slab-proto 反序列化单方向，无 round-trip；零 negative/missing-field/type-mismatch/unknown-field 测试；crate 级无 `deny_unknown_fields`。** `apply_patch` 契约（`ApplyPatchOperation` tagged enum，4 手写 Display）未测。skills.rs 的 `SKILL_CONTENT_RAW==b"string"` 占位比较**不可能失败**（假测试，应删）。

**G34. [High] 6 个 gRPC handler 文件零测试（carry-forward confirmed）。**

**G35. [High] 13/16 v1 HTTP route handler 无 inline 单测；14 个 schema.rs 零测试；crate 无 Rust integration-tests dir。**

**G36. [High] WebSocket JSON-RPC plugin dispatch（两个 `api/jsonrpc/mod.rs`）无 mod 测试。**

**G37. [High] runtime_to_status 仅覆盖 5/20 RuntimeError variant**（06-08 "3/13 CoreError" 双重错误，已纠正）。Scheduler admission 仅 2 测试；inference_lease vs management_lease mutex、poison/Busy/Timeout/next_seq 并发未测。Orchestrator 状态机 + ResultStorage ~1 测试；`wait_stream` 5-branch match 未测。

### 3.8 TS 测试基础设施自测（carry-forward）

**G38. [Medium] `vitest-rust-reporter` 单测默认 CI/test 脚本不跑**（需 `--project vitest-rust-reporter-unit`）。

**G39. [High] Reporter 核心模块未测**：`command.ts`（timeout/kill/exitCode=null）、`report.ts`（infrastructureError heuristic、missingTool 双拼写、if-available vs fatal）、`project.ts` 默认解析、`utils.ts`（trimOutput/count=0 edge）。

**G40. [Low] `slab-plugin-ui`（17 LOC re-export barrel）零测试、无 test script。**

**G41. [Medium, 本轮新盲区] Visual regression 仅 snapshot-PNG equality；Win32-only baselines（1/14 darwin、0 linux）；无 diffThreshold；CI 不跑 browser（影响 local-only）。** 14 个 legacy auto-named `*-1.png` 与当前共存；orphan chat-page baselines（test 已删，4 PNG 残留）。**a11y 全前端零覆盖**（test 源无 `aria-`/`role=`/a11y）。

**G42. [Medium] `slab-windows-full-installer`（1118 LOC）零测试。**

**G43. [Low] `test:frontend` 项目列表手维护子集，drift 于 root projects**（`package.json:34` 硬编码 4 project，漏 server-tests/desktop-browser/components-browser/rust）。

### 3.9 通用 infra 缺口

**G44. [Medium] `slab-utils/lsp.rs` 错误/滥用分支与 8KB header guard 未测**
- 3 个 lsp 测试仅覆盖 happy round-trip 与大小写 Content-Length。未测：header 超 8192 字节（DoS guard）、UnexpectedEof partial header、缺 Content-Length、非数字 Content-Length、非 UTF-8。

**G45. [Medium] `slab-utils/loader.rs` native-library 加载从未端到端测**
- 13 个测试全用 closure mock。`open_native_library` 的 Windows `LOAD_LIBRARY_SEARCH_*` flags（DLL planting 防护，line 62-67）零覆盖。

**G46. [Medium] `slab-config/app_config.rs from_env` 大部分 env var 与解析 edge 未测**
- 20+ env var，7 测试覆盖 5。未测：`SLAB_DATABASE_URL` override、`SLAB_LOG_JSON`/`SLAB_CLOUD_HTTP_TRACE` truthiness（仅 "1"/"true"，"yes"/"TRUE"/"on" 静默 false）、`SLAB_ENABLE_SWAGGER` 反语义、`SLAB_QUEUE_CAPACITY`/`SLAB_BACKEND_CAPACITY` 非数字/0/overflow、`SLAB_ADMIN_TOKEN`/`SLAB_CORS_ORIGINS`/`SLAB_TRANSPORT`。无 negative 测 malformed capacity（当前静默用 default——silent-failure footgun）。

**G47. [Medium] `env_lock` + `EnvGuard` unsafe env-var 脚手架进程全局且不可验证**
- `crates/slab-config/src/app_config.rs:303-326`，`unsafe { std::env::set_var(...) }` 串行化于单 `OnceLock<Mutex<()>>`。Rust 2024 下 set_var/remove_var 是 unsafe 正因线程不安全；强制串行、test order 可观察、不兼容未来并行 runner。06-08 已点，**本轮未修，仍存**。
- 建议：`Config::from_env` 改接受 env-source trait（或 `&HashMap`），消除 mutex + unsafe + 串行要求。

---

## 4. 行动指南与下一步计划 (Action Items)

### 4.1 优先级 Top 10（按 ROI / 风险排序）

| # | 行动 | Owner | Effort | 闭合的 finding |
|---|------|-------|--------|----------------|
| 1 | **CI 新增 job 跑 `test:frontend` + `test:server`**（server binary 复用 cargo job 缓存）；browser/e2e 拆慢 job | release/CI | S | G1（最高杠杆，同时让 G3 的 smoke drift-enforcement 真正生效） |
| 2 | **修 auth fail-open**：`auth.rs` None/空 token fail-closed + 测试；token 比较改 `ConstantTimeEq` | slab-server | S | G7 |
| 3 | **`resolve_path` + `LocalExecutorFileSystem` 表驱动越界测试**（symlink/UNC/绝对路径/`..` 残留） | slab-file + app-core | M | G6, G31 |
| 4 | **`BasicToolRiskAnalyzer` + `redact_secrets` + slab-config secret 表驱动测试/修复** | slab-agent + memories + config | M | G8, G9, G10 |
| 5 | **提取 `migrated_pool()` helper，删除 7 处手写 DDL**（同时闭合 migration 断言盲区） | slab-app-core | M | R1, G15 |
| 6 | **`rpc/client.rs` retry/backoff 决策提取为纯函数并单测** | slab-app-core | M | G13 |
| 7 | **Repository round-trip 套件**（按 `model_download.rs` 模板），优先 `media_task.rs`（24 方法 0 测） | slab-app-core | L | G14 |
| 8 | **Supervisor panic-safety 测试**（panic fake child → 断言 unavailable 标记 + crash-loop） | slab-app-core | M | G19 |
| 9 | **TS hooks + `useWorkspaceUiStore` + `ui-state-storage` 测试**（`renderHook` + fake timers） | slab-desktop | M | G24, G25, G26 |
| 10 | **Coverage threshold + `lint-staged` 配置**（核心包阈值，pre-commit 真正生效）；smoke 补"每条 executable 必被 flow 实调"meta-test + 落实 admin-auth flow | release/CI + slab-server | S | G2, G4, G3 |

### 4.2 分阶段路线图

**Phase 1（0–2 周，止血）—— 投入小、风险降幅最大**
- 行动 #1（CI 跑 TS）、#2（auth fail-closed）、#10（lint-staged 配置 + smoke meta-test）。
- 删除 R10 panic-leak 的 `temp_root()`、R6/R7 重复 builder（机械改动，零设计成本）。
- 修 G33 中 `skills.rs` 的 `SKILL_CONTENT_RAW==b"string"` 假测试。

**Phase 2（2–6 周，安全边界 + 持久化加固）**
- 行动 #3（路径越界）、#4（risk/redaction/secret）、#5（migrated_pool）、#8（supervisor panic）。
- 补 G15 migration post-apply 断言矩阵 + 至少 consolidated rollback round-trip（优先 agent_memories/media_tasks）。
- carry-forward：slab-proto 加 negative/round-trip 测试 + crate 级 `deny_unknown_fields`（G33）。

**Phase 3（6–12 周，覆盖深度 + 冗余根因治理）**
- 行动 #6（RPC retry）、#7（Repository round-trip）、#9（TS hooks/store）。
- 治理**冗余根因**：建立共享 test-support——Rust 侧 `slab-testkit`（含 `migrated_pool`/`run_git_test`/`build_pack_bytes`/fixture builders）；TS 侧每个 package 建 `src/test/` + `__mocks__/`（R12/R13/R5/R18 收敛）。
- 抽取 `crates/slab-plugin-jsonrpc` 闭合 R3 + G36。
- 6 gRPC handler boilerplate 抽取（R15/R16）。
- i18n parity 脚本（G29）、visual baselines 清理（G41 orphan/legacy PNG）、a11y 起步。

**Phase 4（持续，工具链升级）**
- 采用 `cargo-nextest`（更快、retry、partitioning）；CI 装 `cargo-llvm-cov` + 阈值（G5）。
- `Config::from_env` 改 env-source trait，消除 `env_lock` unsafe（G47）。
- proptest/criterion 推广到 parser/config/migration/state-machine crate。
- TS 测试 harness 加 flaky 隔离约定 + per-project timeout 分层（G32）。

### 4.3 与 06-08 审计的关系总结
- **确认并升级**：slab-agent 标杆地位维持；gRPC handler / auth middleware / proto 契约问题全部 confirmed 并延续。
- **纠正 06-08**：migration 覆盖率（"2/19 forward" 错，实为"全 forward-apply-tested，~3-4 post-assert，0 rollback"，High→Medium）；runtime_to_status（"3/13 CoreError" 双错，实为 5/20 RuntimeError）；**smoke registry（首轮臆测的"it.todo 占位冒充覆盖"被推翻——实为有 drift-enforcement 测试守护的纪律良好覆盖率地图）**。
- **06-08 的最大盲区（本轮补齐）**：TS 侧 CI 断层（G1）、安全边界回归保护（G6/G7/G8/G9）、Tauri IPC 层（G30/G31）、冗余的系统根因（无共享 test-support crate / 无 TS `src/test/`）。
- **整体评级 C+ → C（弱 C+）**：Rust 核心小幅改善被 TS/CI/安全的系统性盲区抵消。Phase 1 完成后即可稳定回到 C+，Phase 2 完成后冲击 B。

---

*本报告由 15 路子系统并行审计（首轮 5 路完成 + 第二轮限流重审 9 路 + smoke 子系统主审计员直读源码落地）+ 24 项 High-severity 对抗式验证 + 2 路完整性/跨包重复 critique 综合而成。所有结论均要求引用具体 file:line 证据；被对抗式验证推翻或高估的发现已剔除/降级。*
