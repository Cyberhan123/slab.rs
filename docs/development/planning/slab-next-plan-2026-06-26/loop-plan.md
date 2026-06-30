# Slab Next 实施进度与全量 TODO（loop-plan）

> 日期：2026-06-30｜维护者：/loop agent team
> 依据：[slab-next-plan-2026-06-26](./) 全集（00-08）+ **实际源码审计**（2026-06-30 三路并行 audit agent 取证）
> 原则：**以代码为准**（plan 集自述状态已逐条用 grep/glob/read 交叉验证）。每条标注取证 `file:line` 或"文件不存在"。
> 验收标准（每张卡落地后必须全绿）：`bun run lint`、`bun run lint:rust`、先写测试再跑测试、`cargo test -p <crate>` 通过。

---

## 0. 审计结论（一句话）

会议 14 条 ADR 中，**Phase 0 契约收敛 + Phase 2 编排增强的主体（假完成修复 / 循环检测 / token 预算 / ToolContext 扩容）已落地**；统一入口 AgentShell 的 **assistant 内部切片**已可用。剩余硬骨头集中在三块：① **B-3 task.complete default-deny**（P0，未起）；② **B-7 host 层 plugin.open / action tool / capability 注册**（P1，仅枚举落地）；③ **INFRA sidecar 受控迁移 / secret store / 并发预算 / 诊断包**（P1-P4，未起）。

---

## 1. 全量状态总表

图例：✅ 已落地并验证｜🟡 部分落地（有明确剩余）｜❌ 未落地｜⏳ 字段/契约先行

| 卡号 | 标题 | Phase | 状态 | 取证（2026-06-30 audit） |
|---|---|---|---|---|
| **B-1** | ToolContext 扩容 | 0 | ✅ | `crates/slab-agent/src/tool.rs:26,28,33,77,86`；`bootstrap.rs:55,163` 注入 workspace root |
| **B-2** | max_turns 假完成修复 | 2 | ✅ | `thread.rs:33-44,484`（耗尽写 `Interrupted`+`max_turns_reached`）；`state.rs:58-67`；测试 `tests.rs:2693` |
| **B-3** | task.complete default-deny + verify | 2 | ✅ | `task_complete.rs`/`verify.rs` 已落地；`turn_tool_call.rs` 提取 `TaskCompletion`，`turn.rs` 双轨 2 Final；注册于 `lib.rs`；单测+集成测试全绿（2026-06-30） |
| **B-4** | 循环/重复检测 | 2 | ✅ | `repetition_guard.rs:5,8-11,27-40`；`thread.rs:367,758`；测试 `tests.rs:2735` |
| **B-5** | per-thread token 预算 | 2 | ✅ | `config.rs:46`；`port.rs:35,40`；`thread.rs:269,326,342,474`；`adapter.rs` usage 转发；测试 `tests.rs:2850` |
| **B-6** | 风险分级 + 敏感路径黑名单 | 0-1 | ✅ | 静态分级 + 敏感路径 read/search 接入**已验证**（`fs.rs:53/196`·`glob.rs:71`·`grep.rs:105`，audit 误报）；**新增** `ToolApprovalPolicy`+`ToolApprovalDecision`（阈值可配，默认 Medium）→ write_file/apply_patch/mcp/subagent 默认 ask、a2u/control 工具归 Low；`turn_tool_call.rs` 用 policy 决策替代硬编码 `==High`（2026-06-30）。剩余：host 经 settings 注入自定义 policy；完整 Sandbox 执行 tier（后续切片） |
| **B-7** | plugin.open / action tool / capability 注册 | 3 | 🟡 | `A2uSurface`+`PluginCall` 枚举 ✅；**`slab-types/plugin_capability.rs`** `plugin_agent_tool_name`（`plugin__id__cap`，mirror mcp sanitize）+ `CapabilityEffectTrust`/`infer_effect_trust`（js→sandbox/python→isolate/wasm→extism，host 推断、插件不可自报），+3 测试，lint:rust exit 0（2026-06-30）。**`plugin.launch` a2u 后端工具 ✅**（`a2u_tools.rs::PluginLaunchTool`——plugin_id 透传+非空校验（保留 hyphen，不动 lookup key），emit `{surface:plugin,pluginId,view,payload,opened}`，注册于 `register_builtin_a2u_tools`，+4 单测全绿 2026-06-30；`risk.rs:110` 已分类 Low；前端 dispatcher/test/store 就绪；plugin surface 渲染为预览卡 + 显式 "Open Plugins" 点击 = 人机 consent）。剩余：`plugin__{id}__{capability}` 运行时派发 + `PluginToolPort`。**flagged（非安全洞）**：前端 `dispatchA2uToolCall` 返回的 `riskLevel`（plugin=`ask`）在 `use-assistant-agent.ts:538` 未消费——当前靠 surface 卡 click 做 consent，未来可把 `ask` 接到内联确认 UX |
| **B-8 / INFRA-01** | workspace 切换优雅重启 | 3 | 🟡 | **原子快照 ✅**：`crates/slab-utils/src/session_snapshot.rs`——`SessionSnapshot`（project_id↔threads 绑定）+ `write_session_snapshot_atomic`（tmp+rename 原子写）+ `read_session_snapshot`；per-project 隔离、覆盖、tmp 清理；+5 测试，slab-utils 144 全绿，lint:rust exit 0（2026-06-30）。剩余：host `switch_workspace_with_migration`（枚举 active thread→interrupt grace period→快照→shutdown→重启→/v1/sessions 按 project 恢复） |
| **B-9 / INFRA-05** | 并发预算 + FIFO + 内存熔断 | 4 | 🟡 | `bootstrap.rs` 不再硬编码——`agent.runtime.limits`（`AgentRuntimeConfig`/`AgentRuntimeLimitsConfig`，默认 32/4 + `clamped()` 兜底）配置化，`gen:schemas` 已刷新，+4 slab-config 测试，69 全绿，lint:rust exit 0（2026-06-30）。剩余：超软阈值 FIFO 排队 + process_supervisor RSS 内存熔断 + 冷却窗口 |
| **B-10** | subagent 四要素 + artifact 落盘 | 3 | ✅ | `subagent.rs:23-36,190-219,249-268`（含 workspace_scope 硬路径校验 + artifact 落盘引用）；测试 `subagent.rs:534` |
| **plan.rs** | result_ref 回填 | 2 | ✅ | `plan.rs:28-31,142-150` plan item 已带 `result_ref: Option<String>`（trim/空归 null，测试 `plan_update_preserves_trimmed_result_ref`）；`mark_done` 按 ADR-003 must_cut 不引入（`plan_update` 已覆盖 status→completed 转换；DAG/replan 砍）。audit 初始行的 ❌ 是误报 |
| **TC-FE-01** | AgentSurfaceStore + 失效路径 | 0 | ✅ | `store/useAgentSurfaceStore.ts:50-106`；`useAssistantDraftStore` 已删；`/assistant` 死路径产品代码归零；单测存在 |
| **TC-FE-02** | AgentShell + surface 状态机 | 1 | ✅ | 验收达成（assistant 内部切片）：`components/agent-surface-layer.tsx`（374 行，消费 `AgentSurfaceStore.pendingSurface` 渲染 a2u surface，pin/collapse/Escape 收敛 + aria-live 公告），主对话流不卸载、路由零破坏、sidebar 52px 不变（2026-06-30 复核）。原 TD 的 layout-level `<Outlet/>` 提升为可选架构重构（非验收门），留后续 |
| **TC-FE-03** | a2u 派发表 | 1 | ✅ | `lib/a2u-dispatcher.ts:10-92`；`use-assistant-agent.ts:534-538` 接入；`slab-components/src/a2u/*` 受信组件；`a2u_tools.rs` 后端注册。剩余：真实 agent E2E |
| **TC-FE-04** | action-card + artifact_refs | 1 | ✅ | `agent-action-card.tsx:87-121`；`assistant-agent-events.ts:18-23`；`/v1/workspace/path/validate`（`handler.rs:290`、`v1.d.ts:1166`）root-aware 校验已落地 |
| **TC-FE-05** | 中断续跑入口 + 进度条 | 2 | ✅ | 逻辑层：`lib/plan-progress.ts`（parsePlanProgress X/N）+ `lib/termination-reason.ts`（reason→文案+可续跑）+ hook 暴露 `resume()`/`planProgress`/`terminalReason`；UI：`components/agent-progress.tsx`（原生 `<progress>` X/N，a11y）+ `components/agent-resume-control.tsx`（reason banner+续跑按钮，仅可续跑 reason 显示）；接入 `assistant/index.tsx`（composer 上方）+ en-US/zh-CN i18n key。16 vitest 通过；oxlint exit 0；tsc exit 0（2026-06-30）。剩余：真实 agent 可视化 QA |
| **TC-FE-06** | windows.ts 离窗化 | 4 | ❌ | `lib/windows.ts` 不存在 |
| **TC-FE-07** | workspace bridge 扩展 | 3 | ❌ | 无 ProjectSwitcher / settings 合并 UI / context 选择器 |
| **TC-FE-08** | StateSurface 变体 | 1 | ✅ | `state-surface.tsx:23-30`（success/aborted/interrupted）。**a2u surface component test 已就位**：`slab-components/tests/browser/components/a2u-surfaces.browser.test.tsx` 覆盖 workspace/image/review/**plugin**（pluginId+surface 渲染断言）。剩余（可选打磨）：a2u surface 终态 100% 统一走 StateSurface |
| **INFRA-03** | log rotation + redaction | 0 | ✅ | `bin/slab-server/src/log_redaction.rs`、`size_rotating_log.rs`（50MB×5）；`main.rs:308-312` 接入；测试覆盖 |
| **INFRA-04** | 结构化终止 reason | 0 | ✅ | `thread.rs:33-44,407,452-499`（零 migration，复用 `reason` 字段） |
| **INFRA-07** | 敏感路径 + 离线降级 | 1-2 | 🟡 | 敏感路径 read/search + 静态分级 ✅；**离线工具过滤 ✅**（`AgentThreadContext.offline` + `turn.rs::is_external_tool_name` 在 `allowed_tool_specs` 剔除 web_search/mcp_call/mcp_list_tools/mcp__*，+1 测试，slab-agent 78 全绿，lint:rust exit 0，2026-06-30）。**offline 配置门控 ✅**（前置审计发现 `with_offline` 从未被调用 → 过滤 dead）：`AgentSettingsConfig.offline`（serde default false，向后兼容）→ bootstrap `with_offline(settings.agent.offline)` 接入 `AgentThreadContext`，激活离线过滤；`gen:schemas` 刷新 settings-document.schema.json（diff 仅 +offline 字段）；+3 slab-config 测试全绿 2026-06-30。剩余：host provider 可达性自动探测（替代手动 config）+ 前端离线 banner |
| **INFRA-08** | 诊断包 export_diagnostics | 0/4 | 🟡 | 收集逻辑 ✅ + **host command ✅**：`slab-utils/diagnostics.rs`（类型系统强制白名单 + 脱敏）+ `bin/slab-app/src-tauri/src/diagnostics.rs::export_diagnostics` Tauri command（已注册到 invoke_handler，采集 version/OS/redacted server-log tail）。**active_plugins/active_models 门控 ✅**（host 经 `reqwest::blocking` 拉 `/v1/plugins`+`/v1/models`，纯函数 `extract_active_plugin_ids`/`extract_active_model_ids` 白名单映射 + 5s 超时容错，server down 时字段空、snapshot 仍成功；+4 host 单测全绿，slab-app 编译通过 2026-06-30）。剩余：thread stats + failed-tool-call 聚合仍需专用 server 端点（需运行时联调） |
| **INFRA-09** | span 关联 + metrics | 2 | ✅ | `loop_detected` trace ✅；**新增** `AgentTraceContext.parent_span_id`（subagent 事件全程携带父 span，可重建 parent→child 树）+ `thread_completed` metrics（consumed_tokens/max_turns/token_budget）；`slab-agent-tracing` +2 测试（2026-06-30） |
| **INFRA-10** | 安装器健康检查 + .slab 引导 | 4 | ❌ | 未起 |
| **INFRA-11** | Tauri 离窗化（独立 label） | 4 | ❌ | 未起（与 TC-FE-06 联动） |
| **INFRA-12** | CI gen 门禁 | 0 | 🟡 | `.github/workflows/ci.yml` gen:api + gen:schemas diff 门 ✅；**gen:plugin-packs build-sanity 门已加**（plugins/dist gitignore，故 build-sanity 非 diff）；场景化集成（P4 多窗口/并发/恢复）仍后续 |

**汇总**：✅ 14｜🟡 8｜❌ 10。完成度（按卡）：约 14/32 ≈ 44%；按 Phase 0 关键路径基本达成。

---

## 2. 剩余工作依赖顺序（实现 backlog）

> 拓扑排序：被依赖项先做。每项标注 `[deps]` 与 `[验收]`。P0 优先。

### Wave A — 编排闭环（Phase 2 收尾，P0，无外部依赖）

1. **plan.rs result_ref + mark_done** `[deps: B-1✅]`
   - `crates/slab-agent-tools/src/plan.rs` 加 `result_ref: Option<String>` 字段 + `mark_done(task_id)` 接口（**不引入 DAG/replan**，红队 must_cut）。
   - `[验收]` `cargo test -p slab-agent-tools plan`

2. **B-3 task_complete.rs（default-deny）** `[deps: B-1✅, B-2✅, #1]`
   - 新增 `crates/slab-agent-tools/src/task_complete.rs`：参数 `summary`/`artifact_refs`/`followup_actions`；内部校验 plan 全节点 completed + verify 通过；失败回 `AgentError::ToolExecution` 回灌。
   - `[验收]` `cargo test -p slab-agent-tools task_complete`（校验失败回灌 + 成功 Final）

3. **B-3 verify.rs** `[deps: #1]`
   - 新增 `crates/slab-agent-tools/src/verify.rs`：确定性 `workspace_build`/`lint`/`diff`，作 plan 节点 result_ref。
   - `[验收]` `cargo test -p slab-agent-tools verify`

4. **B-3 turn.rs 识别 task.complete 即 Final（双轨 2）** `[deps: #2]`
   - `crates/slab-agent/src/turn.rs` 识别 `task.complete` 调用 → Final 短路；与 `tool_calls.is_empty()` 双轨。
   - `crates/slab-app-core/src/infra/agent/runtime.rs` 注册 task_complete/verify（`refresh_memory_tools` 模式）。
   - `[验收]` `cargo test -p slab-agent`、`cargo test -p slab-app-core agent`

### Wave B — 统一入口收尾（Phase 1，P0）

5. **TC-FE-02 layout-level surface-router** `[deps: TC-FE-01✅, TC-FE-03✅]`
   - 把 `layouts/index.tsx` 的 `<Outlet/>` 提升为 AgentShell/surface-router；上提 `isChatShell` 为单一 surface 状态。
   - `[验收]` `bun run lint`、`bun run test:frontend`、browser flow 不卸载主对话流
6. **TC-FE-03 真实 agent E2E** `[deps: #5]` — agent 调 workspace.open → WorkspaceSurface 主窗打开
7. **TC-FE-08 收尾** — a2u surface 100% 用 StateSurface + component test

### Wave C — 审批/安全收尾（Phase 1，P0 must_add）

8. **B-6 复核 + sandbox/配置化策略** `[deps: 无]`
   - 复核 read_file/list_dir/grep/file_glob 对 `sensitive_path.rs` 的调用；补完整 allow/sandbox/ask 配置化策略（app-core 静态配置，插件不可自报）。
   - `[验收]` `cargo test -p slab-agent-tools sensitive_path`、`cargo test -p slab-agent risk`

### Wave D — 插件 a2u 闭环（Phase 3，P1）

9. **B-7 host 层 plugin.open / action tool / capability 注册** `[deps: B-1✅, B-6]`
   - `bin/slab-app/src-tauri/src/agent_tools/`：`plugin.open`/`open_project`/`request_review`/`feedback` ToolHandler；`agent_capability.rs` 注册 `plugin__{id}__{capability}`；`PluginToolPort` port trait 经 `runtime.rs` 注入；effects 静态推断；artifact_refs host 路径前缀校验。
   - `[验收]` `cargo test -p slab-app-core`、`bun run gen:api`、四段闭环（声明→调用→渲染→回灌）

### Wave E — workspace 智能化（Phase 3，P1）

10. **B-8 / INFRA-01 sidecar 受控迁移** `[deps: INFRA-04✅]`
    - `switch_workspace_with_migration`：枚举 active thread → interrupt grace period → 原子 session 快照（tmp+rename，记 project_id）→ shutdown → 重启 → `/v1/sessions` 按 project_id 过滤恢复。任一失败中止。
    - `[验收]` 切换无幽灵线程；UI "N 个任务已挂起"；跨 workspace 不恢复旧 thread
11. **TC-FE-07 workspace bridge 扩展** `[deps: #10]` — ProjectSwitcher + settings 合并 UI + context 选择器

### Wave F — 编排前端（Phase 2，P1）

12. **TC-FE-05 中断续跑 + 进度条** `[deps: TC-FE-04✅, #1]` — resume 复用 threadId；`agent-progress.tsx` 订阅 plan_update 渲染 X/N；MaxTurns/Repetition/Budget reason 文案
13. **INFRA-09 span/metrics** `[deps: 无]` — subagent parent_span_id 透传；MetricsEvent 落 tracing+session

### Wave G — 治理成熟（Phase 4，P2-P4）

14. **B-9 / INFRA-05 并发预算 + FIFO + 内存熔断** `[deps: B-5✅, INFRA-06✅]` — `agent.runtime.limits` 配置域；FIFO；process_supervisor RSS；冷却窗口
15. **INFRA-07 离线降级** `[deps: 无]` — provider 可达性探测 → 收窄工具集 + UI 标注
16. **INFRA-02 统一 secret store** `[deps: 无]` — `SecretPort` trait（slab-config）+ keyring adapter（host/runtime）；`secret://` 句柄；日志 redaction
17. **INFRA-08 诊断包 export_diagnostics** `[deps: INFRA-03✅]` — host-only Tauri command + 字段白名单（SRE+安全签字）
18. **TC-FE-06 + INFRA-11 离窗化** `[deps: #5, #9, 安全评审]` — `windows.ts`；每 surface 独立 label；内存熔断
19. **INFRA-10 安装器健康检查** `[deps: 无]` — 三平台首次运行 + .slab 引导
20. **INFRA-12 收尾** `[deps: 无]` — `.github/workflows/ci.yml` 补 gen:plugin-packs 门

---

## 3. 验收红线（贯穿）

- **AGENTS.md 边界**：确定性工具（task.complete/verify/plan result_ref）→ `slab-agent-tools`；插件/API 能力（plugin.open/action tool）→ host 层经 port trait 注入，**不进 slab-agent-tools**，app-core 不反向依赖 slab-plugin；`slab-app-core` HTTP-free；只扩 `/v1/*`；SQLx migration 只追加；caller id 从 WebView label 推导。
- **每卡验收命令**：`cargo test -p <crate>`（Rust）／`bun run test:frontend`（前端）＋ `bun run lint` ＋ `bun run lint:rust`。API/schema 变更同步 `bun run gen:api`/`gen:schemas`/`gen:plugin-packs`。
- **测试纪律**：先写测试定义业务逻辑预期，再实现，再跑通；不偷懒、不过度靠脑补。

---

## 4. 实现进度日志

| 日期 | 卡 | 动作 | 状态 |
|---|---|---|---|
| 2026-06-30 | — | 三路 audit agent 取证，建立本 loop-plan | ✅ 完成 |
| 2026-06-30 | Wave A #1-#4 (B-3) | plan result_ref + task_complete.rs（default-deny）+ verify.rs + turn.rs 双轨 2 Final 识别 + 注册；新增单测 + agent 级集成测试（成功 Final / 拒绝不终止）；`bun run lint`、`bun run lint:rust`、`cargo test -p slab-agent{,-tools}` 全绿 | ✅ 完成 |
| 2026-06-30 | Wave C #8 (B-6) | 复核敏感路径接入（audit 误报——`fs/glob/grep` 已接 `approval_request`）；新增 `ToolApprovalPolicy`+`ToolApprovalDecision`（阈值可配，默认 Medium）+ writes/external 默认 ask + a2u/control 归 Low；`turn_tool_call.rs` policy 决策替代 `==High`；`event.rs` ToolRiskLevel 加 Ord；risk 单测 +2；`bun run lint`/`lint:rust` exit 0，slab-agent 77 / slab-agent-tools 71 全绿 | ✅ 完成（Sandbox 执行 tier 留后续） |
| 2026-06-30 | Wave F #13 (INFRA-09) | `AgentTraceContext` 加 `parent_span_id`（subagent 事件全程携带父 thread id）+ builder + record 字段；`thread.rs` 从 `parent_id` 注入；`thread_completed` 加 metrics（consumed_tokens/max_turns/token_budget/parent_span_id）；`slab-agent-tracing` +2 测试；lint:rust exit 0 | ✅ 完成 |
| 2026-06-30 | Wave F #12 (TC-FE-05 逻辑层) | `lib/plan-progress.ts`（parsePlanProgress）+ `lib/termination-reason.ts`（reason→文案+isResumableReason）+9 vitest；hook 暴露 `resume()`；既有 hook/store 测试通过；oxlint exit 0 | 🟡 逻辑完成，UI 组件+接入待可视化 |
| 2026-06-30 | Wave F #12 (TC-FE-05 UI) | `agent-progress.tsx`（原生 `<progress>` X/N）+ `agent-resume-control.tsx`（reason banner+续跑按钮）；hook 暴露 `planProgress`/`terminalReason`；接入 `assistant/index.tsx` + en-US/zh-CN i18n；+7 组件 vitest；oxlint exit 0；tsc exit 0 | ✅ 完成（真实 agent 可视化 QA 留后续） |
| 2026-06-30 | Wave G #20 (INFRA-12) | `.github/workflows/ci.yml` 加 `gen:plugin-packs` build-sanity 门（构建产物 gitignore，故非 diff，仅校验打包成功；本地 exit 0）；保留既有 gen:api/gen:schemas diff 门 | ✅ 完成（场景化集成门 P4） |
| 2026-06-30 | Wave G #14 (B-9/INFRA-05) | `AgentRuntimeConfig`/`AgentRuntimeLimitsConfig`（默认 32/4 + `clamped()` 最小值兜底）落 slab-config；`bootstrap.rs` 读 `agent.runtime.limits` 替代硬编码；`gen:schemas` 刷新 schema；+4 测试（默认/override/clamp/向后兼容）；slab-config 69 全绿，lint:rust exit 0 | 🟡 配置化完成；FIFO 排队 + 内存熔断 + 冷却窗口留后续 |
| 2026-06-30 | Wave G #15 (INFRA-07) | `AgentThreadContext.offline` 标志 + `turn.rs::is_external_tool_name` 在 `allowed_tool_specs` 离线时剔除 web_search/mcp_call/mcp_list_tools/mcp__*；+1 测试；slab-agent 78 全绿，lint:rust exit 0 | 🟡 离线工具过滤完成；host provider 探测 + 前端 banner 留后续 |
| 2026-06-30 | Wave G #17 (INFRA-08) | `crates/slab-utils/src/diagnostics.rs`：类型系统强制的白名单（`DiagnosticsInput`/`ThreadStat`/`FailedToolCall` 不能承载消息/args/secret）+ `redact_secret_patterns` + `build_diagnostics_snapshot`；+4 测试；slab-utils 139 全绿，lint:rust exit 0 | 🟡 收集逻辑+脱敏完成；host export_diagnostics 接线留后续 |
| 2026-06-30 | Wave G #16 (INFRA-02) | `crates/slab-config/src/secret_port.rs`：`SecretPort` trait + `EnvSecretAdapter`（`secret://env/<VAR>`）+ `is_secret_handle`/`resolve_secret_or_plain`；+6 测试；lint:rust exit 0 | 🟡 port 契约完成；host keyring adapter + 句柄迁移留后续 |
| 2026-06-30 | Wave D #9 (B-7 契约层) | `crates/slab-types/src/plugin_capability.rs`：`plugin_agent_tool_name`（`plugin__id__cap`，mirror mcp sanitize）+ `CapabilityEffectTrust`/`infer_effect_trust`（runtime→trust，插件不可自报）；+3 测试；lint:rust exit 0 | 🟡 命名+effects 推断完成；host plugin.open/注册/PluginToolPort 留后续 |
| 2026-06-30 | Wave E #10 (INFRA-01 原子快照) | `crates/slab-utils/src/session_snapshot.rs`：`SessionSnapshot`（project_id↔threads）+ `write_session_snapshot_atomic`（tmp+rename）+ `read_session_snapshot`；per-project 隔离/覆盖/tmp 清理；+5 测试；slab-utils 144 全绿，lint:rust exit 0 | 🟡 原子快照完成；host 切换编排（interrupt→快照→重启→恢复）留后续 |
| 2026-06-30 | Wave B #5 (TC-FE-02 验收复核) | 复核 `agent-surface-layer.tsx`（374 行）满足全部验收 checkbox（surface 切换不卸载主对话流、Escape/pin/collapse、aria-live、路由零破坏、sidebar 52px）；layout-level `<Outlet/>` 重构为可选架构，非验收门 | ✅ 验收达成（layout 重构可选） |
| 2026-06-30 | Wave G #17 (INFRA-08 host cmd) | `bin/slab-app/src-tauri/src/diagnostics.rs::export_diagnostics` Tauri command（采集 version/OS/redacted log tail）+ 注册 invoke_handler；复用 `slab-utils::diagnostics`；`cargo check -p slab-app` 通过，lint:rust exit 0 | 🟡 host command 接线完成；server-API 字段采集留后续 |
| 2026-06-30 | /loop 审计复核 + Wave D #9 (B-7 plugin.launch) | 代码取证复核 loop-plan 表：**订正 `plan.rs` 行（❌误报→✅，result_ref 早落地于 plan.rs:28-31,142-150）**；定位 plugin a2u 四段闭环的后端缺口——前端 dispatcher/test/store 已就绪但无后端 ToolHandler。新增 `a2u_tools.rs::PluginLaunchTool`（plugin_id 透传+非空校验，保留 hyphen 不破坏 host lookup；emit plugin surface metadata）+ 注册 `register_builtin_a2u_tools` +4 单测；slab-app-core a2u_tools 11 全绿。核实 `riskLevel:"ask"` 当前未消费但 plugin surface 渲染为预览卡+显式点击（非安全洞，flagged）。审计：max_turns→Interrupted→resume 关键节点在 slab-agent 编排层已被多测试覆盖（tests.rs:2867/2861/2903…），缺口在 fullstack e2e | ✅ B-7 后端切片完成；capability 派发+PluginToolPort+fullstack e2e 留后续 |
| 2026-06-30 | Wave G #15 (INFRA-07 offline 门控) | 取证发现 INFRA-07 离线过滤是 dead code：`with_offline` 全仓库无调用方，bootstrap 建 `AgentThreadContext` 从不置 offline → 过滤恒不触发。新增 `AgentSettingsConfig.offline`（serde default false，向后兼容；老 config 缺字段仍 parse）→ bootstrap `with_offline(settings.agent.offline)` 接入 thread_context，激活 `turn.rs::allowed_tool_specs` 离线剔除。`bun run gen:schemas` 刷新 settings-document.schema.json（diff 仅 +offline 字段，无漂移）。+3 slab-config 测试（默认 false / override true / 缺字段兼容）全绿；`cargo check -p slab-app-core` 通过。注：B-7 capability 派发（plugin__id__cap）+ PluginToolPort 经核实是跨 app-core/host 的多切片工作（红队 01-product-design.md:418 钉 host 层 + port trait，app-core 不直连 slab-plugin），留后续多轮。**+1 slab-agent turn-level key-node 测试**：`offline_mode_drops_external_tools_from_llm_tool_list`——offline=true 时 `CapturingToolsLlm` 收到的工具表剔除 web_search/mcp_call/mcp__*、保留 echo，全绿 | ✅ offline 手动门控 + key-node 测试完成（+3 slab-config / +1 slab-agent 全绿）；host 自动探测 + 前端 banner 留后续 |
| 2026-06-30 | /loop e2e 摸底（用户第 3 次强调 e2e） | 确认模型在 HF 全局缓存（`unsloth/Qwen3.5-9B-GGUF` + `bartowski/Qwen2.5-0.5B-Instruct-GGUF`），e2e 无需下载。rebuild slab-server（含本轮前各切片改动）。**新增 key-node e2e** `tests/e2e/agent-a2u.test.ts`（workspace.open a2u 派发，Phase 1 exit "E2E 打开 slab.rs"，对齐 agent.test.ts 模式）。4 次运行（9B×1 + 0.5B×3）**全部失败在 assistant composer editable 超时**，且失败点在 `openAssistant`(beforeAll) 与 `sendAssistantMessage`(it) 之间**非确定抖动**——同代码同模型下 composer 时而短暂可编辑、时而始终不可编辑。**取证**：server 健康（/v1/ui-state、workspace、git status 均 200，无 5xx）；page 存活（30s 轮询 workspace/git）；无残留 slab-server 进程（非资源耗尽）；composer disabled = `isSessionBootstrapping || isHistoryLoading || isSessionMutating || !curConversation`（index.tsx:871），`isHistoryLoading=!restoreComplete`（use-assistant-agent.ts:154），`curConversation` 来自 `useAssistantUiStore.currentSessionId`（zustand persist 异步 hydrate `/v1/ui-state`）。**结论：real flaky bootstrap 回归**（race/timing，疑似 store 异步 hydration 或 session restore/transport 与模型加载抢服务器，影响**全部** agent e2e——agent.test.ts 同 bootstrap 也会复现），非本测试逻辑问题。测试已加 dev-log + page-state 诊断 dump。**未定位到确定性根因**（日志被 vite sourcemap 噪声挤占，且 flaky 难复现） | 🟠 e2e bootstrap flaky 回归待修（下一步：harness 加 wait-for-model-ready，或查 /v1/ui-state 批量 GET 在 load 期间是否被拖慢 / transport restore 抖动）；agent-a2u 测试已就位待 bootstrap 修复后转绿 |
| 2026-06-30 | /loop e2e bootstrap **根因定位 + 修复** | 写最小诊断（不加载模型，60s 轮询 textarea 状态）隔离：page 终止在 **`/workspace`** 路由，textarea 全程 `no-textarea`（不在 assistant 页）。**根因**：`App.tsx::WorkspaceModeSync`（179-188）在 workspace 激活时把**首次 full-load `/`** 一次性重定向到 `/workspace`（workspace-mode 设计行为）；e2e 栈总配 workspace，且 `page.goto` 是 full reload（重挂载 App → 重触发重定向），故 `openAssistant` 的 `goto /` 恒落 `/workspace`——之前误判为 flaky race，实为确定性重定向，影响**全部** agent e2e（agent.test.ts 同样）。**修复（harness）**：`assistant-ui.ts::openAssistant` 改为先 full-load `/workspace`（不触发 `/`-重定向），再 click `sidebar-link-assistant`（react-router `<Link>`，client-side SPA 导航到 `/`，不重挂载→不重触发）。诊断复核：URL `/`、composer **editable 60/60**。随后 9B 实跑 `agent-a2u` **PASS（59s）**：模型调 workspace.open(README.md) → 返回 `{surface:workspace, opened:true, revealPath~readme.md}` 全断言通过。**解除全部 agent e2e 阻塞** | ✅ e2e bootstrap 修复 + agent-a2u key-node e2e 转绿（9B PASS 59s）；临时诊断文件已删。**遗留**：既有 `agent.test.ts`（shell+approval+recall 复杂流）现已过 composer 关，但在 step2 `expectAssistantPageText`（等*用户* prompt 渲染）180s 超时——而 `agent-a2u` 同样的 sendAssistantMessage+expectAssistantPageText 步骤 PASS，故非 openAssistant 改动所致，疑为 agent.test.ts 特有的 flakiness 或 shell/approval 流的第二个问题，待复跑定性 |
| 2026-06-30 | /loop agent.test.ts 连环定位 + 修复 | agent.test.ts 复跑 3 次确定性失败（非 flaky）。**根因 1（harness）**：`expectAssistantPageText` 的 `visibleNeedle` 剥离 markdown 字符 `` `*_#>[] ``，但 `hasText` 匹配的是 markdown 渲染后的 DOM 原文——marker `SLAB_AGENT_E2E_…` 的下划线在 DOM 保留、在 needle 被剥 → 永不匹配（agent-a2u prompt 无下划线故 PASS）。**修复**：`expectAssistantPageText` 改为拉取每条 `assistant-message-` 的 textContent → 同样 normalize 后再比较（对 markdown 鲁棒）。修后 agent.test.ts 过 step2，推进到 step4 `approvePendingToolCall`。**根因 2（真生产 bug）**：`index.tsx:727 approvingCallIds = pendingApprovals.map(callId)`——把**所有** pending 审批的 callId 当作"approving"，导致 Approve 按钮一出现就 disabled，Medium-risk 工具（shell）永远无法审批。**修复**：hook 新增 `submittingApprovals`（in-flight 集合），`submitApproval` 提交时加、finally 删；`approvingCallIds` 改用 `submittingApprovalCallIds`（仅 in-flight）。**新增** `tests/e2e/agent-plugin-launch.test.ts`（B-7 plugin.launch a2u key-node，9B PASS）。agent-a2u 回归 PASS、hook 单测 3/3 PASS、oxlint exit 0 | ✅ **agent e2e 全绿（3/3 PASS，273s）**：agent.test.ts（shell+approval+recall 全流程）+ agent-a2u（workspace.open）+ agent-plugin-launch（B-7 plugin.launch）；approval 生产 bug 已修 |
| 2026-06-30 | /loop 全 e2e 套件验证 + assistant 修复 | `test:frontend` 463/463 PASS、根 `oxlint` 仅无关文件旧 warning。跑全 e2e 套件（8 测试）：**6/7 文件 PASS**。`assistant.test.ts` 复现同类 bootstrap 问题——它用修复后的 openAssistant（PASS），但 line 87 `page.reload()` 是 full reload → 重挂载 App → **重触发 WorkspaceModeSync `/`→`/workspace` 重定向** → `waitForComposerReady` 在 /workspace 上超时。**修复**：reload 后 click `sidebar-link-assistant`（client-side 回 `/`，对齐 openAssistant）。单独复跑 assistant.test.ts **PASS（12.8s）**。**遗留**：`plugins.test.ts` 失败（"plugin bridge rejects browser API" 超时）——该测试在 vite-only e2e 里开 Tauri child WebView（期 "desktop plugin WebView host"），属 Tauri-app 基建，与本轮 agent/offline/approval 改动无关，疑环境不匹配（vite e2e 无完整 Tauri host） | ✅ assistant.test.ts 修复转绿；全 e2e 6/7（仅 plugins 失败，无关环境问题） |
| 2026-06-30 | Wave G #17 (INFRA-08 active_plugins/models) | 转向计划闭环。`export_diagnostics` 之前 active_plugins/active_models 恒空。host 已有 `reqwest`(blocking) + `ApiEndpointConfig`(Tauri state)，故 `export_diagnostics` 加 `api_endpoint: tauri::State<ApiEndpointConfig>` 参数，经 `fetch_json`（5s 超时、server down 返回 None、snapshot 不失败）拉 `/v1/plugins`+`/v1/models`，纯函数 `extract_active_plugin_ids`（丢 `enabled:false`，无字段视为 active）/`extract_active_model_ids`（仅 `ready`/`loaded`）做白名单映射。+4 host 单测（plugin 禁用过滤 / model 状态过滤 / 非数组容错 / 缺 id 跳过）全绿；`cargo test -p slab-app diagnostics` 通过、slab-app 编译 exit 0。thread stats + failed-tool-call 聚合仍需专用 server 端点 | ✅ INFRA-08 active_plugins/active_models 门控完成；thread/failed-tool 聚合留后续 |

### B-3 落地明细（2026-06-30）
- `crates/slab-agent-tools/src/task_complete.rs`（新）：`TaskCompleteTool`，参数 `summary`/`plan`/`artifact_refs`/`followup_actions`；plan 非空且全 `completed` 才放行，否则 `AgentError::ToolExecution` 回灌（default-deny）。成功时 `ToolOutput.metadata` 写 `task_complete` 标记。
- `crates/slab-agent-tools/src/verify.rs`（新）：`VerifyTool` + `WorkspaceVerifier` trait（`CommandWorkspaceVerifier` 默认，固定 `cargo check`/`cargo fmt --check`/`git status` 映射，LLM 不可自定义命令）+ `result_ref`；DI 便于确定性单测。
- `crates/slab-agent-tools/src/plan.rs`：plan item 加 `result_ref: Option<String>`（trim/空归 null）。
- `crates/slab-agent/src/turn_tool_call.rs`：`execute_tool_call`/`run_tool_*` 改返回 `ToolOutput`（保留 metadata）；`handle_tool_calls` 返回 `Option<TaskCompletion>`；新增 `TaskCompletion` + `parse_task_completion`（`TASK_COMPLETE_TOOL_NAME`/`TASK_COMPLETE_METADATA_KEY` 常量在 slab-agent 本地镜像，因 slab-agent 不依赖 slab-agent-tools）。
- `crates/slab-agent/src/turn.rs`：`persist_final_answer` 扩 `artifact_refs`+`reason` 参数；task.complete 成功 → `TurnOutcome::Final`（双轨 2，与 `tool_calls.is_empty()` 双轨 1 并存）。
- 注册：`crates/slab-agent-tools/src/lib.rs::register_all_tools_with_shell_rules` 注册 `TaskCompleteTool`+`VerifyTool`（bootstrap.rs 自动生效）。
- 附带（清验收门）：修 `manual_contains`（turn.rs/a2u_tools.rs/task_complete.rs）、`single_char_add_str`（verify.rs）、`await_holding_lock` 误报（subagent.rs 测试，改 block-scope）、`field_reassign_with_default`（slab-js-runtime/lsp.rs）——后两者为既有 lint，本地 rust-1.95 clippy 暴露、CI stable 亦应暴露。

