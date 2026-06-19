# 综合收尾与保障计划 (Closure & Guardrails)

| 字段 | 值 |
|---|---|
| Plan ID | F |
| 角色 | 最后一道防线 / 综合收尾 / 风险兜底 |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) |
| 依赖 | 承接 Plan A–E 全部任务；自身横切，不产出业务功能 |
| 状态 | Draft / Pending Review |
| 预估总工作量 | M（持续贯穿 A–E 全周期） |

---

## 1. 定位与目标

本计划**不产出任何单一业务功能**，而是作为 A–E 五个专项计划之上的**横切保障层**与**逻辑闭环**：

- **承接边缘任务**：跨计划但不属于任何单一域的事项（契约同步、CI 闸门、跨平台验证、无障碍）。
- **风险兜底**：识别 A–E 的跨计划依赖链与单点阻塞，给出回滚/开关/缓解。
- **逻辑闭环**：把 A–E 的执行整合为一条可交付的主干路线图，并定义"完成（Definition of Done at Release）"的统一判据。

> **北极星**：确保五个专项计划的产出在合并到 `main` 时**契约一致、回归覆盖、风险可控、文档同步**，不因集成产生新缺陷。

---

## 2. 跨计划集成总览

### 2.1 主干路线图（Waves）

| Wave | 时间 | 并行任务 | 关键交付 |
|---|---|---|---|
| **W0 止损** | Wk1 | Plan A 全部 P0（T-A-1/2/3/4）‖ T-E-4 ‖ T-C-1/T-C-2 | 安全越权闭环、reduced-motion、流式可取消且不毁内容 |
| **W1 地基** | Wk2 | T-B-7（错误契约，阻塞源）· T-E-1（Token）· T-A-5 | 解锁 T-B-5/6、T-C-6、T-E-2/3/7 的下游 |
| **W2 能力+可靠性** | Wk3–4 | T-B-1/2/3/4 ‖ T-C-3/4/5/7/8 ‖ T-E-2/3/6 | 进度可视化、agent 工具体系、SSE resume、Token 落地 |
| **W3 交互闭环** | Wk5–6 | Plan D（Track 1+2）‖ T-E-7 | 审批/重发/工作区 IDE 体验、magic-px 清除 |
| **W4 打磨** | Wk7 | T-D-4/7/10/12 ‖ T-E-5 | 代码块复制、终端多标签、Skeleton shimmer |
| **Gate** | 持续 | F-1 ~ F-9 | 契约/回归/CI/i18n/perf/文档/回滚/平台/a11y |

### 2.2 关键跨计划交接（逻辑闭环的"接缝"）

| 产出方 → 消费方 | 接缝内容 | 闭环要求 |
|---|---|---|
| **T-A-1**（插件 caller-id 鉴权）→ **T-B-5 / T-B-6** | 安全通道（`X-Slab-Plugin-Caller` / Tauri label） | T-B-5/6 必须在 T-A-1 合并后方可启用卸载/更新与 rpc/events 调用 |
| **T-B-7**（统一错误层）→ **T-C-6 / T-B-1~6 / T-D-5,8,12** | `getLocalizedErrorMessage` / `isServerError` / `ErrorDataDetail` | T-B-7 是**单点阻塞**（6 卡依赖），W1 严守不 slip；T-C-6 在 T-B-7 前 Decoupled via mock 可先动 |
| **T-C-1**（AbortController）→ **T-C-3 / T-D-2** | 流式取消底层 | T-D-2（pendingAbort）必须与 T-C-1 同 PR 或紧随其后 |
| **T-E-1**（Token）→ **T-E-2/3/7 / T-B-1 / T-B-7 / T-C-6 / Plan D** | `.glass-surface` / `--focus-ring` / `<StateSurface>` | Plan D 各卡在 Token 交付前用"临时实现 + TODO"，交付后统一替换 |
| **T-D-9**（Monaco 懒启动）→ **T-D-6 / T-D-10** | 编辑器选区与配置 | T-D-6（AI 解释）依赖 Monaco 选区就绪 |

> 任一接缝的产出方 slip，须立即在 §4 风险登记册登记并通知所有消费方。

---

## 3. 横切保障门（Task Cards）

### T-F-1 · 契约同步门（`gen:api` / `gen:schemas`）
- **严重度** P0 · **类型** infra · **预估** S（持续）
- **证据** AGENTS.md L26「Backend API shape changes require `bun run gen:api`」；涉及任务：T-A-1/2（插件路由加固）、T-B-5/6（插件端点已存在，需核对）、T-D-8（新增 `/v1/workspace/watch`）、T-B-3（load/unload 已存在）
- **问题**：A–E 中凡改动 OpenAPI / `slab-types` / `slab-proto` / 新增端点的任务，若忘记重生成 `packages/api/src/v1.d.ts`，前端类型与后端漂移。
- **方案**：
  1. 在 PR 模板加 checklist：「本 PR 是否改动后端 schema/route？→ 若是，已运行 `bun run gen:api`（必要时 `bun run gen:schemas`）并提交 `v1.d.ts` diff」。
  2. CI 增加一道"契约一致性"校验：`bun run gen:api --check`（若脚本支持 dry-run；否则对比生成结果与暂存区是否一致），不一致即失败。
  3. 汇总本计划集所有"触约"任务清单，作为唯一的事实来源。
- **验收标准 (AC)**：
  - [ ] PR 模板含契约同步 checklist
  - [ ] CI 含 `gen:api` 一致性门并红过一次验证有效
  - [ ] 所有触约任务在合并前确认 `v1.d.ts` 已同步
- **依赖**：无（横切）

### T-F-2 · 回归与 E2E 覆盖（关键流闭环）
- **严重度** P1 · **类型** infra · **预估** L
- **证据** 仓库已有 `bun run test:browser`、`test:components`、`.playwright-mcp/`；当前 E2E 未覆盖本次修复流
- **问题**：A–E 修复的流（流式中断/重连、媒体进度/取消、Workspace 未保存守卫、插件生命周期、错误 toast）缺自动化回归，回归风险高。
- **方案**：为以下 5 条关键流补 Playwright/browser 用例：
  1. Assistant：发起→流式中断（AbortController）→内容保留；断网→SSE resume 重连续上。
  2. Media：提交→进度推进（断言 `progress` 渲染）→取消→后端确认后状态归位。
  3. Workspace：编辑未保存→关闭标签/切换文件→弹 AntD Modal（非原生 confirm）→丢弃/保留分支。
  4. Plugins：导入→启用→（T-B-5）卸载；越权调用 `/api-request` 被拒（T-A-1）。
  5. Errors：触发 `Conflict/TooManyRequests/NotImplemented` → 断言 toast 文案为本地化（非"unexpected error"）且可重试态正确。
- **验收标准 (AC)**：
  - [ ] 5 条 E2E 用例入库并绿
  - [ ] `bun run test:browser` 在 CI 中运行
- **依赖**：依赖 T-A-1/T-B-5/T-B-7/T-C-1/T-C-3 等 P0/P1 产出

### T-F-3 · CI 闸门收口（最小充分集）
- **严重度** P1 · **类型** infra · **预估** S
- **证据** AGENTS.md「Use the narrowest validation command that covers the change」；根命令 `lint / check / check:frontend / check:rust / test`
- **问题**：无统一 CI 准入判据，可能漏跑 Rust 校验（A/B/D 触及 Tauri/server）或前端类型校验。
- **方案**：
  1. 按"改动面→校验集"矩阵定义 CI job：触 Rust→`check:rust`+`test:rust`；触前端→`check:frontend`+`test:frontend`+`lint`；触 schema→叠加 F-1。
  2. 主分支合并门槛：`lint` + `check` + 对应 `test` 全绿。
- **验收标准 (AC)**：
  - [ ] CI 矩阵文档化（写入本文件或 CI 配置注释）
  - [ ] 至少一个触及 Rust 的 PR 验证 `check:rust` 被触发
- **依赖**：无

### T-F-4 · i18n parity 守卫
- **严重度** P1 · **类型** infra · **预估** S
- **证据** [locale-parity.test.ts](packages/slab-i18n/src/__tests__/locale-parity.test.ts) 已强制 zh-CN↔en-US 对称
- **问题**：A–E 新增大量用户可见文案（reasoning 标签、进度阶段、T-B-7 翻译后的错误文案、插件权限提示、setup checklist、状态原语），易漏 i18n。
- **方案**：
  1. 所有新增文案强制走 i18n key（禁止硬编码中英文，dev-only 页除外）。
  2. CI 运行 locale-parity 测试；PR 模板加「新增文案是否双 locale」勾选。
  3. 特别核对 T-B-7：`getLocalizedErrorMessage` 覆盖 zh/en 两套。
- **验收标准 (AC)**：
  - [ ] locale-parity 测试在 CI 绿
  - [ ] 抽查 5 处新增文案均有双 locale key
- **依赖**：T-B-7（错误文案翻译）

### T-F-5 · Bundle / Perf 预算与度量
- **严重度** P1 · **类型** infra · **预估** M
- **证据** `package.json` 含 52 个 `@codingame/monaco-vscode-*`；审计 §3.3.5 标注 Monaco eager boot
- **问题**：新增功能 + Monaco 懒启动改造（T-D-9）若不度量，可能净增体积或引入 LSP 初始化竞态。
- **方案**：
  1. 在 `build:app` 后采集主 bundle 与 workspace chunk 体积，建立基线。
  2. 设预算：主 bundle 不超过基线 +5%；workspace（Monaco）chunk 懒加载后首屏不加载。
  3. T-D-9 落地前后对比 LCP/TTI（Playwright trace 或 manual）。
  4. T-D-9 风险：LSP 初始化竞态 → 监听 `editor.onDidChangeModel` 而非轮询（审计已标注）。
- **验收标准 (AC)**：
  - [ ] 体积基线与预算文档化
  - [ ] T-D-9 前后度量数据记录
  - [ ] 无 LSP 初始化回归
- **依赖**：T-D-9

### T-F-6 · 文档所有权同步
- **严重度** P2 · **类型** infra · **预估** S
- **证据** AGENTS.md「README Ownership」「When adding/removing workspace members, plugin surfaces, sidecars, package responsibilities, update this file and the affected local README files in the same change」
- **问题**：T-B-5/6（插件 rpc/events 消费）、T-D-8（workspace watch 端点）、T-A-1/2（插件鉴权变更）触及 plugin/workspace surfaces，按规须同步 README/AGENTS。
- **方案**：维护"文档触发表"——凡触及 plugin surface / workspace 端点 / package 职责的任务，合并时同步更新对应 README 与 AGENTS.md 相关条目。
- **验收标准 (AC)**：
  - [ ] 文档触发表列出所有需同步项
  - [ ] 发布前核对 README/AGENTS 无遗留
- **依赖**：T-A-1/2、T-B-5/6、T-D-8

### T-F-7 · 特性开关与回滚策略
- **严重度** P1 · **类型** infra · **预估** M
- **证据** 高风险改造：SSE resume（T-C-3）、Monaco 懒启动（T-D-9）、错误包络统一（T-B-7）
- **问题**：这三项一旦回归影响面大，需快速回滚而不发版。
- **方案**：
  1. 为 SSE resume / Monaco lazy / 统一错误包络加 feature flag（前端读取 settings 或编译期 env）。
  2. 错误包络统一（T-B-7）保留旧 OpenAI-shape 适配作为 fallback 路径，灰度切换。
  3. 记录每项的"回滚开关位"与回滚 SOP。
- **验收标准 (AC)**：
  - [ ] 3 个 flag 就位并有默认关闭/灰度策略
  - [ ] 回滚 SOP 文档化
- **依赖**：T-B-7、T-C-3、T-D-9

### T-F-8 · 跨平台与无障碍验证
- **严重度** P1 · **类型** infra · **预估** M
- **证据** T-A-1/2 触及 Tauri（win/mac/linux）；T-D-7 终端 shell 选择平台差异；T-E-4 reduced-motion
- **问题**：Tauri 鉴权/终端行为在平台间可能漂移；a11y 除 reduced-motion 外（焦点环 T-E-3、键盘可达）需验证；新组件需 dark/light 双主题走查。
- **方案**：
  1. T-A-1/2、T-D-7 在 Windows（主）+ mac/linux smoke 验证。
  2. a11y：键盘流（Tab 顺序、焦点环可见 T-E-3）、屏幕阅读器对 StateSurface/进度/错误的朗读。
  3. dark/light 双主题人工走查清单（hub/assistant/image/plugins/settings/workspace）。
- **验收标准 (AC)**：
  - [ ] 跨平台 smoke 通过
  - [ ] a11y 走查清单完成
  - [ ] 双主题无破相（含 raw-hex 清除 T-E-7 后）
- **依赖**：T-A-1/2、T-D-7、T-E-3、T-E-4、T-E-7

### T-F-9 · 边缘收口与遗留登记
- **严重度** P2 · **类型** infra · **预估** S
- **证据** 审计中未单列但相关：全局键盘快捷键一致性、可选的可观测性（结构化错误上报，便于 T-B-7 新错误码线上观测）
- **问题**：零散边缘项无归属易遗漏。
- **方案**：
  1. 维护"遗留/超出本轮"登记表（如 git push/pull、MCP 工具更深度集成、marketplace 完整版），明确不在本轮范围。
  2. （可选）为 T-B-7 新错误码加最小埋点，便于线上观测可重试/致命分布。
- **验收标准 (AC)**：
  - [ ] 遗留登记表就位并经评审
  - [ ] （可选）错误码埋点上线
- **依赖**：T-B-7

---

## 4. 风险登记册（Risk Register）

| ID | 风险 | 概率×影响 | 来源 | 缓解 |
|---|---|---|---|---|
| RISK-1 | **T-B-7 错误契约 slip** → 6 卡级联阻塞 | 中×高 | Plan B | W1 严守；T-C-6 先用本地 mock Decoupled 前进；设独立验收节点 |
| RISK-2 | **SSE resume**：原生 `EventSource` 无法设 `Last-Event-ID` header | 高×中 | Plan C | 采用 `fetch`+`ReadableStream` 重写 SSE 客户端（T-C-3 已标注） |
| RISK-3 | **T-E-2 软分割线**：topbar 复合渐变背景被覆盖 | 高×高 | Plan E | 用 inset shadow fade（非 `background-image`），保留 topbar 渐变 |
| RISK-4 | **Monaco 懒启动**：LSP 初始化竞态/选区失效 | 中×高 | Plan D/C | 监听 `editor.onDidChangeModel` 而非轮询；T-D-9 度量（F-5） |
| RISK-5 | **错误包络统一**回归 assistant 错误 UX | 中×高 | Plan B | feature flag + 保留旧适配 fallback（F-7）灰度 |
| RISK-6 | **插件鉴权加固**（T-A-1）误拒合法调用 | 中×高 | Plan A | 先补 caller-id 单测再改；T-B-5/6 在其后启用 |
| RISK-7 | **新增功能致 bundle 膨胀** | 中×中 | Plan B/D | F-5 预算门；Monaco chunk 懒加载 |
| RISK-8 | **跨平台 Tauri 行为漂移**（鉴权/终端） | 中×中 | Plan A/D | F-8 跨平台 smoke |

---

## 5. 发布检查清单（Release Checklist）

合并至发布分支前，逐项绿：

- [ ] **止损闭环**：Plan A 全部 P0（T-A-1/2/3/4）合并并经安全复核
- [ ] **契约同步**：所有触约任务已 `bun run gen:api`，`v1.d.ts` 与后端一致（F-1）
- [ ] **校验全绿**：`bun run lint` + `check` + `check:frontend`（+ 触 Rust 时 `check:rust`）（F-3）
- [ ] **测试**：`test:frontend` + `test:components` + 关键流 5 条 E2E 绿（F-2）
- [ ] **i18n**：locale-parity 测试绿；抽查文案双 locale（F-4）
- [ ] **Perf**：bundle 在预算内；T-D-9 度量无回归（F-5）
- [ ] **特性开关**：SSE resume / Monaco lazy / 错误包络 flag 就位且有回滚 SOP（F-7）
- [ ] **平台/a11y**：跨平台 smoke + a11y 走查 + 双主题无破相（F-8）
- [ ] **文档**：README/AGENTS 同步（F-6）；遗留登记表评审（F-9）
- [ ] **五大最高杠杆动作**完成度：①T-B-1 进度 ②T-E-1/2 Token+软线 ③T-A-1 越权止损 ④T-B-7 错误层 ⑤T-B-3 Hub 闭环

---

## 6. 指挥官收尾备注

- **节奏**：W0 止损必须最先落地（安全 P0 不可拖）；W1 地基（T-B-7/T-E-1）是后续并行能展开的前提，资源优先向其倾斜。
- **闭环原则**：任一"接缝"（§2.2）产出方变更，须同步通知消费方并更新本文件 §2.2 与风险登记册——这是本计划作为"逻辑闭环"的核心动作。
- **不做的事**：本计划不为赶进度而放宽任一保障门；若 F-1/F-2/F-8 任一红线未过，发布推迟。
- **与审计的关系**：本计划集是对 [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) 的直接响应；审计中的 9 个 P0 全部映射到 W0/W1 任务卡，无遗漏。
