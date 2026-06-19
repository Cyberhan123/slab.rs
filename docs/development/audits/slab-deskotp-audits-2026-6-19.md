# Frontend Audit & Alignment Specification: Slab Desktop

- **Audit Date:** 2026-06-19
- **Status:** Draft / Pending Review
- **Target Location:** `docs/development/audits/slab-deskotp-audits-2026-6-19.md`
- **Audit Scope:** `packages/slab-desktop`（桌面/Web 前端）↔ `bin/slab-server` `/v1/*` ↔ `crates/slab-app-core` / `bin/slab-runtime`
- **Frontend Stack:** React 18 · react-router-dom · Ant Design + `antd-style` + `@ant-design/x` · Tailwind · TanStack React Query · Zustand · Tauri · Monaco · xterm
- **Method:** 7 个领域审计 Agent 并行取证，逐条对照当前代码（非陈旧文档），所有结论附 `file:line` 证据。

---

## 1. 执行摘要与当前痛点概述

### 1.1 总体判断

后端（`slab-server` / `slab-app-core` / `slab-runtime`）在推理调度、任务管道、流式协议、插件运行时、能力面（capability surface）上已显著领先于桌面/Web 前端。前端的**核心症状不是"做错了什么"，而是"没有把后端已经做好的东西释放出来"**：大量后端能力字段、端点、事件流在前端被静默丢弃，导致一个功能强大但交互单薄、反馈缺失、状态二义的产品体验。

### 1.2 五大系统性根因（Root Causes）

| # | 根因 | 表现 | 受影响域 |
|---|---|---|---|
| **R1** | **能力释放失败（Capability Drop）** | 后端已实现的能力字段/端点/事件在前端无入口或被读取后丢弃 | Assistant / Media / Hub / Plugins / Cross-cutting |
| **R2** | **状态对齐脆弱（State Misalignment）** | 异步/流式/持久化状态多源、无恢复、无确认，UI 与后端真实状态脱节 | Assistant / Media / Workspace / Infra |
| **R3** | **安全与正确性 P0** | 跨插件越权、未保存变更可绕过、auth 静默放行 | Plugins / Workspace |
| **R4** | **交互硬伤（Interaction Flaws）** | 反馈缺失、状态二义、原生 confirm、轮询风暴、取消竞态 | 全域 |
| **R5** | **设计系统碎片化（Design Fragmentation）** | Token 缺失、硬分割线、magic-px、无 reduced-motion、空/错/载三态分裂 | UI/UX 全域 |

### 1.3 严重度统计

| Severity | 数量 | 含义 |
|---|---|---|
| **P0** | **9** | 破坏核心链路 / 数据丢失 / 崩溃 / 安全越权 |
| **P1** | **~30** | 显著的能力或体验断层 |
| **P2** | **~30** | 打磨项 |

### 1.4 最关键的 9 个 P0（详见各域）

1. **插件 `/v1/plugins/{id}/api-request` HTTP 路由无 caller-id 派生 → 跨插件 slabApi 越权**（R3）
2. **`authorize_plugin_call_request(None)` 静默放行** → 无 label 调用者特权提升（R3）
3. **Workspace 未保存变更用 `window.confirm`，且不覆盖真实 Monaco 编辑器** → 关闭标签页可静默丢数据（R3/R2）
4. **Workspace HTTP `recent`/`config` 恒为空**（状态源割裂：读 HTTP、写 Tauri）→ 最近工作区永远空白（R2）
5. **Assistant 流式请求无 `AbortController` 取消** → interrupt 帧丢失时 token 持续空烧（R2）
6. **`turn_failed` 用错误字符串整体覆盖已流式输出的部分内容** → 90% 的回答被毁且无恢复（R2）
7. **Task 详情对任意任务类型显示"重启"，但后端仅允许 `model_download`** → 点击必报 400（R1/R4）
8. **媒体生成进度数据后端已提供（`progress.current/total/step/step_count`），前端全量丢弃** → 所有生成过程只剩无意义 spinner（R1）
9. **全域无 `prefers-reduced-motion` 支持** → 无障碍/品牌质量缺陷（R5）

---

## 2. 后端功能对齐与接口映射清单

### 2.1 接口映射矩阵（Backend `/v1/*` → Frontend 暴露状态）

状态图例：✅ 已正确使用 · ⚠️ 使用但有缺陷 · ❌ 后端已实现但前端未暴露 · 🚫 缺失（后端也无）

| 后端端点 / 能力 | 前端落点 | 状态 | 关键缺口 |
|---|---|---|---|
| `POST /v1/agents/responses` (WS+SSE) | assistant | ⚠️ | 无 SSE resume（`Last-Event-ID` 未回传），无 `AbortController`，`turn_failed` 毁部分内容 |
| `agent.shutdown` 命令 | — | ❌ | 会话切换/卸载时从不调用 → 服务端线程泄漏 |
| `POST /v1/chat/completions`、`/v1/completions` | — | ❌ | 全部经 agent/responses 通道，OpenAI 兼容入口未用（可保留） |
| `AgentConfigInput.reasoning_effort`（5 级） | assistant `deepThink` | ⚠️ | 硬编码 `medium`，`none/low/high/minimal` 不可达（[assistant-agent-state.ts:47](packages/slab-desktop/src/pages/assistant/lib/assistant-agent-state.ts#L47)） |
| `AgentConfigInput.{system_prompt,allowed_tools,tool_choice,tool_concurrency,structured_output,verbosity}` | — | ❌ | `toAgentConfig` 仅设 `max_turns`；slash 命令只是文本前缀，未映射到 `allowed_tools` |
| `GET/POST/PUT/DELETE /v1/sessions`、`/messages` | assistant | ✅ | — |
| `GET /v1/models` | hub | ✅ | — |
| `POST /v1/models/download` | hub | ⚠️ | 状态双源（后端 `status` + 本地 `downloadTracking`）→ 重载后闪烁；失败无卡片内重试 |
| `POST /v1/models/{load,unload,switch}` | — | ❌ | Hub 零 load/unload 入口，用户无法手动驻留/驱逐模型 |
| `GET /v1/models/available`、`/v1/backends{,/status}`、`/v1/system/diagnostics` | — | ❌ | 磁盘/VRAM/后端健康度不可见 |
| `POST /v1/images/generations` (+list/get/artifact/reference) | image | ⚠️ | `progress` 字段未读；5 分钟硬轮询上限误杀长任务；取消不等后端确认 |
| `POST /v1/video/generations` | video | ⚠️ | 同上；无"在工作区打开"、无 A/B、无批量 |
| `POST /v1/audio/transcriptions` | audio | ⚠️ | 无取消入口；`segments`/时间戳丢弃，仅纯文本 `<pre>`；无 SRT/VTT 导出 |
| `GET/POST /v1/tasks` (+get/result/cancel/restart) | task | ⚠️ | restart 按钮未按 `task_type==='model_download'` 门控 → 非 download 任务必 400 |
| `GET/POST /v1/workspace`（state/open/close/dir/files/path/search/git/console） | workspace | ⚠️ | HTTP `recent`/`config` 恒空（[handler.rs:563](bin/slab-server/src/api/v1/workspace/handler.rs#L563)）；open 不重启 sidecar |
| `WS /v1/workspace/lsp/{language}` | workspace | ⚠️ | 语言允许表在 TS 硬编码 13 项，与后端漂移（缺 `jsonc` 等） |
| `/v1/workspace/watch`（文件监听） | — | 🚫 | 后端无监听端点，外部改动后 Explorer/Search 失效 |
| `GET /v1/plugins`(+{id}) | plugins | ✅ | — |
| `POST /v1/plugins/install`（packageUrl+sha256） | — | ❌ | 仅本地 `.plugin.slab` 导入，无 URL/商店安装 |
| `POST /v1/plugins/import-pack` | plugins | ⚠️ | 1GB 上限无进度条/取消 |
| `POST /v1/plugins/{enable,disable,start,stop}` | plugins | ⚠️ | stop 时硬编码 `lastError:null` 覆盖诊断态 |
| `DELETE /v1/plugins/{id}` | — | ❌ | 无卸载 UI（`removable` 字段已就绪） |
| `WS /v1/plugins/rpc`（JSON-RPC 2.0） | — | ❌ | 宿主 UI 从不消费，仅 Tauri 子 WebView 可调 |
| `WS /v1/plugins/events` | — | ❌ | 宿主 UI 不订阅，安装/运行态事件无法实时刷新 |
| `POST /v1/plugins/{id}/api-request` | plugin webview | ⚠️ | React 走裸 HTTP，无 caller-id 派生 → 越权（P0） |
| `GET /v1/settings` + `PUT /v1/settings/{pmid}` | settings | ✅ | 但无搜索；autosave 无未保存守卫 |
| `GET /v1/setup/status`、`POST /provision`、`/complete` | setup | ⚠️ | `complete` 未用；provision 自动触发无确认门；组件级（ffmpeg/backends）不可见 |
| `GET /v1/system/gpu` | footer-status-bar | ✅ | 但未与模型 size 关联做 VRAM 适配提示 |
| `GET|PUT|DELETE /v1/ui-state/{key}` | store | ⚠️ | 失败静默 `console.warn`，`hasHydrated` 仍置真 |
| `GET /health` | backend-status | ⚠️ | 每次 refetch 抖动 "Checking"；Offline 无恢复 CTA |

### 2.2 「能力释放失败」清单（Capability Drop，R1）

后端已具备、前端无入口或读取后丢弃的高价值能力：

- **推理深度 5 级** → 仅暴露布尔 `deepThink`。
- **Agent 工具体系** `allowed_tools` / `tool_choice` / `tool_concurrency` / `structured_output` / `system_prompt` → 完全未透出；slash 命令沦为文本前缀。
- **任务进度** `progress.{current,total,step,step_count}` → 三个生成 hook 零读取（[media-task-api.ts](packages/slab-desktop/src/lib/media-task-api.ts)、`use-{image,video}-generation.ts`）。
- **音频分段** `segments[]`（带时间戳）+ `transcript_text` → 仅渲染纯文本。
- **模型 load/unload/switch + capabilities + size** → Hub 无管理动作、无体积/显存可见、分类靠文件名字符串匹配（`inferModelCategory`）而非 `capabilities`。
- **插件 JSON-RPC 派发 + 事件总线 + uninstall + install-from-URL + 贡献的 settings/commands** → 宿主层全部未消费。
- **错误契约** `i18n`（已解析未翻译）、结构化 `data.{suggestion, attempts, code}` → 全域只显示单行英文文案。
- **Workspace LSP 能力枚举** → 前端硬编码语言表，无法感知插件贡献的语言服务器。

### 2.3 状态源与协议层断层（State & Protocol，R2）

- **HTTP ↔ Tauri 状态源割裂（Workspace）**：读走 HTTP（`recent`/`config` 恒空），写走 Tauri（`workspaceUpdatePluginPreference`）→ 偏好永不回显；`WorkspaceInfo.databasePath` TS 有、HTTP schema 无（[schemas/workspace.rs:23](crates/slab-app-core/src/schemas/workspace.rs#L23)）。
- **模型状态双源**：后端 `status==='downloading'` 与本地 `downloadTracking` 并存 → 重载后短暂显示 `ready`/`not_downloaded`。
- **空 `QueryClient`**：`new QueryClient({})` 无默认 retry/staleTime，22 处 `retry:false` 各自为政；无 `MutationCache` 全局 onError → ~30 处手写 toast，一致性差（如 GPU 错误被吞进假快照 [footer-status-bar.tsx:68](packages/slab-desktop/src/layouts/footer-status-bar.tsx#L68)）。
- **错误包络双形态**：服务端 `{code,message,data,i18n}` vs assistant 假定的 OpenAI 形 `{error:{type,code,...}}` → assistant 失败不产生 `ApiError`，丢失可重试性分类与共享 toast 逻辑。
- **`ErrorCodes` 缺 4 类**：`Conflict 4009` / `NotImplemented 5010` / `TooManyRequests 4029` / 全部 `data.code` 子码未映射 → 429/501 落入 `default` 显示"unexpected error"。

---

## 3. 产品与 UI/UX 深度诊断及优化建议

### 3.1 核心交互瑕疵与修复方案

#### 3.1.1 流式/异步状态脆弱（最高优先）

| 瑕疵 | 证据 | 修复方案 |
|---|---|---|
| **[P0] 流式请求不可取消** | assistant `responsesMutation` 无 `AbortController`，[use-assistant-agent.ts:709](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L709) 仅发 `agent.interrupt` | 为 mutation 与 `EventSource`/`WebSocket` 传入 `AbortController`，`abort()` 与 unmount 时 `abort()` 并关闭通道 |
| **[P0] `turn_failed` 毁部分内容** | [use-assistant-agent.ts:250](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L250) 整体覆盖 `content` | 仿 `turn_cancelled` 保留已流式内容，追加独立 error footer/记录 |
| **[P1] SSE 无 resume/重连** | [use-assistant-agent.ts:441](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L441) `error` 仅置 `eventsConnected(false)` | 后端已支持 `Last-Event-ID`；退避后重开 `openSse`，回传最大 seen-id 做服务端 replay |
| **[P1] 媒体 5 分钟硬轮询上限** | image `MAX_POLL_ATTEMPTS=150×2s`（[const.ts:29](packages/slab-desktop/src/pages/image/const.ts#L29)）→ 长任务超时但后端仍在跑 | 依据 task 终态终止而非固定次数；显著上调上限 + 指数退避 |
| **[P1] 取消不等后端确认** | image/video `handleCancel` 无论成败都 `clearGenerationTask()` | await 成功再清状态；失败 toast 并保留 running 视图 |

#### 3.1.2 状态二义与竞态

- **[P1] 审批全局单例**：`pendingApproval` 单对象（[use-assistant-agent.ts:72](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L72)），`tool_concurrency>1` 时第二个 `approval_required` 覆盖第一个 → 改 `Map<callId, PendingApproval>`，按 thought 渲染。
- **[P1] Stop 按钮竞态空操作**：`threadId` 为 null 时 `abort()` 直接返回（[:709](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L709)），但"提交→ack 返回 thread_id"窗口内取消按钮无效 → 引入 `pendingAbort` flag，thread_id 到达后补发 interrupt。
- **[P2] 自动滚动与用户上滑打架**：每次 delta 触发 `scrollIntoView`（[index.tsx:483](packages/slab-desktop/src/pages/assistant/index.tsx#L483)）→ 仅在"近底部"时强制滚动。
- **[P2] Reasoning 折叠态每渲染重置**：`expandedThoughtKeys` 来自 `defaultExpandedKeys`（[assistant-bubble-content.tsx:292](packages/slab-desktop/src/pages/assistant/components/assistant-bubble-content.tsx#L292)）→ 改受控 `expandedKeys`。

#### 3.1.3 反馈缺失与原生控件

- **[P0] Workspace 未保存守卫可绕过**：`handleOpenFile`/`handleSelectFileTab`/`handleSelectGitDiff` 用 `window.confirm`（[use-workspace-page.ts:280,543,687,731](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L280)），且 dirty 仅存于 React state，真实 Monaco-vscode 编辑器关闭路径不触发 → 统一到 `getWorkingCopyService` 的 dirty，并用 AntD Modal 替换原生 confirm。
- **[P1] BackendStatus 每次 refetch 抖动 "Checking"**：`isChecking = isLoading||isRefetching`（[backend-status.tsx:40](packages/slab-desktop/src/components/backend-status.tsx#L40)）→ 仅 `isLoading` 视为 Checking；Offline 加"重试连接"动作，连续失败阈值才翻红。
- **[P1] 无 toast 去重**：视频轮询错误每轮 toast（[use-video-generation.ts:345](packages/slab-desktop/src/pages/video/hooks/use-video-generation.ts)）→ Sonner `dedupe`（key=`code|message`）或轮询错误 ref-count 门控。
- **[P1] ui-state 持久化全静默**：[ui-state-storage.ts:34](packages/slab-desktop/src/store/ui-state-storage.ts#L34) catch 仅 `console.warn`，`hasHydrated` 仍真 → 首次失败 toast"无法加载偏好"，区分 404（新装）与网络失败。
- **[P2] ErrorBoundary 无导航复位**：[error-boundary.tsx:30](packages/slab-desktop/src/components/error-boundary.tsx#L30) 仅"Try Again" → 加 `useLocation` key 路由切换复位。

#### 3.1.4 媒体域专项

- **[P0] 生成进度完全不展示**（详见 §2.2）→ 读 `task.progress` 渲染 `<Progress>` + `step/step_count` + 阶段（加载模型→采样→解码）。
- **[P0] 音频转写无取消**：[audio-workbench.tsx:407](packages/slab-desktop/src/pages/audio/components/audio-workbench.tsx#L407) 全程 spinner → 复用 `cancelTaskMutation`。
- **[P1] Restart 按钮未门控**（P0#7）→ `task.task_type==='model_download'` 才渲染。
- **[P1] 参数无即时校验**：`width|||512` 静默兜底（[use-image-generation.ts:300](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L300)）；`clip_skip`/`eta` 边界未对齐后端 schema → 从 OpenAPI schema 生成 slider min/max。

#### 3.1.5 基础设施层

- **[P1] 重试策略不分可重试/致命**：空 `QueryClient`，瞬态 5xx/`BackendNotReady` 零重试，400 反而重试 → 全局 `retry:(n,err)=>isApiError(err)&&err.isServerError()&&n<2` + 指数退避，删除散落 `retry:false`。
- **[P1] 无 `MutationCache` 全局 onError** → 统一 `onError: toast.error(getLocalizedErrorMessage(err,t))`，页面仅覆盖乐观/特殊场景。
- **[P2] 无乐观更新**：插件 enable/stop、session 增删、model import 均 mutate→invalidate → 高频 toggle 加 `onMutate/onError` 回滚。

### 3.2 界面视觉系统重构（聚焦高级通透感、无边界设计、轻量化视线引导）

> 现状评估：Token 层（`globals.css` 的 `--shell-*` / `--surface-soft` / `--shell-card` / halo 渐变）**基础扎实且已有高级感雏形**（teal+gold 径向 halo），但缺三类关键 Token（elevation / glass / motion），且存在大量内联魔法值与硬分割线，削弱了通透感。重构以"补 Token + 收敛组件类 + 软化分割线"为主，而非推倒重来。

#### 3.2.1 Token 体系缺口（证据为 grep 计数）

| 缺口 | 证据 | 影响 |
|---|---|---|
| **阴影未 Token 化** | ~30 处内联 `shadow-[0_…_color-mix(in_oklab,var(--foreground)…)]`（[hub/index.tsx:142](packages/slab-desktop/src/pages/hub/index.tsx#L142)、[video-workbench.tsx:505](packages/slab-desktop/src/pages/video/components/video-workbench.tsx#L505)…） | 同深度卡片阴影不一 |
| **原生色泄漏** | `setup-workbench.tsx:221,278` raw rgba 阴影（暗色态不变）、`:88` `#00685f` 未用 `--brand-teal` | 暗色主题破相 |
| **玻璃透明度散乱** | `/45 /55 /72 /80 /85 /92 /95` 七档 opacity 各异 | 通透感不一致 |
| **圆角魔法值** | `rounded-[24/28/30/32/34px]` 30+ 处 | 半径无尺度 |
| **字号魔法值** | `text-[10/11/12/13/17px]`、`text-[1.65rem]` 共 **138 处/36 文件** | 排版节奏紊乱 |
| **字距魔法值** | `tracking-[-0.025/-0.04/-0.045/-0.05/-0.055em]`、`[0.12/0.16/0.22em]` | 标题光学对齐页间漂移 |
| **间距魔法值** | `gap-[18px]`、`p-[17px]`、`p-[5px]` 混用 scale | 密度页间不一 |

#### 3.2.2 硬分割线 → 软渐变（"通透感"最大单点收益）

当前 sidebar `shadow-[inset_-1px_0_0_var(--shell-line)]`（[sidebar.tsx:117](packages/slab-desktop/src/layouts/sidebar.tsx#L117)）、topbar/footer `border:1px solid var(--shell-line)`（globals.css:662/684）、header 内 `<span className="h-4 w-px bg-[var(--shell-divider)]">` 竖线（[header.tsx:95,130,137](packages/slab-desktop/src/layouts/header.tsx#L95)）——这些 1px 实线在 halo 渐变背景上显得沉重、割裂。

**方案**：替换为约 6px 的 `linear-gradient(var(--shell-line), transparent)` 软渐变 fade（`--hairline-soft`），让功能区"渗入"而非"撞上"彼此。

#### 3.2.3 动效与无障碍

- **[P0] 全域无 `prefers-reduced-motion`**（`slab-desktop/src` 与 `slab-components/src` 零命中）：`animate-pulse`/`animate-spin`/`hover:scale-105` 无条件运行 → 全局 `@media (prefers-reduced-motion: reduce)` 禁用非必要动画。
- **[P1] 动效无 Token 且过浅**：83 处 `transition`/`animate-*` 均裸 Tailwind 默认 `ease`，无 enter/exit 编排，brief 要求的"渐入"不存在 → 定义 `--ease-out-expo`/`--dur-*`，对 Dialog/Sheet/Card/EmptyState 统一施加 ~180ms `cubic-bezier(.16,1,.3,1)` 的 opacity+translateY "soft in"。

#### 3.2.4 状态原语统一

- **[P1] 三套空/错/载原语分裂**：`Empty`（[slab-components/empty.tsx](packages/slab-components/src/empty.tsx)）、`EmptyPanel`（plugins）、`StageEmptyState`（[slab-components/workspace.tsx:176](packages/slab-components/src/workspace.tsx#L176)）样式各异；加载态分别为散文块 / spinner / skeleton grid → 收敛为单一 `<StateSurface variant="empty|loading|error">`（icon/title/desc/action slots，halo 背景统一）。
- **[P1] 焦点环不一致**：Button `ring-[3px] ring-ring/50`（[button.tsx:8](packages/slab-components/src/button.tsx#L8)）、sidebar `ring-2 color-mix…28%`（[sidebar.tsx:94](packages/slab-desktop/src/layouts/sidebar.tsx#L94)），hub/plugin 卡片与历史行无可见焦点环 → 单一 `--focus-ring` Token + `.focus-ring` 工具类。
- **[P2] Skeleton 无 shimmer**：flat `bg-accent animate-pulse`（[skeleton.tsx:7](packages/slab-components/src/skeleton.tsx#L7)）→ 方向性 shimmer 渐变更契合通透美学。

#### 3.2.5 编辑器主题对齐

- **[P1] 双主题系统**：Monaco VS Code workbench 自带 theme service（Seti 默认）与 Slab `antd-style`/Tailwind 主题仅在 `vs`/`vs-dark` 基础切换同步（[use-workspace-page.ts:174](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L174)），token 配色/滚动条/面板 chrome 会与外围漂移 → 注册 VS Code theme contribution 映射 Slab 设计 Token，或显式收边。
- **[P2] 圆角套方角**：`SoftPanel rounded-[18px]` 内嵌方形 VS Code 编辑器（[workspace-workbench.tsx:402](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx#L402)）→ 编辑器面板内层用 `rounded-[6px]` 或外层改方角。

#### 3.2.6 新增 Token 提案（"无边界/通透"方向）

```css
/* Elevation（替换 ~30 处内联阴影） */
--elevation-1: 0 1px 2px oklch(0% 0 0 / 0.04);
--elevation-2: 0 18px 44px -30px color-mix(in oklab, var(--foreground) 28%, transparent);
--elevation-3: 0 32px 80px -48px color-mix(in oklab, var(--foreground) 40%, transparent);

/* Glass（替换 /45 /55 /72 /80 /85 /92 /95 散乱） */
--glass-bg:        color-mix(in oklab, var(--shell-card) 62%, transparent);
--glass-bg-strong: color-mix(in oklab, var(--shell-card) 80%, transparent);
--glass-border:    color-mix(in oklab, var(--shell-card) 32%, transparent);
--glass-blur:      14px;
--glass-highlight: inset 0 1px 0 color-mix(in oklab, var(--shell-card) 55%, transparent);

/* 软发丝线（替换 1px 实线） */
--hairline-soft: linear-gradient(var(--shell-line), transparent);

/* 圆角尺度扩展 */
--radius-2xl: 1.5rem; --radius-3xl: 2rem;

/* 字号尺度（消除 138 处 magic-px） */
--text-micro: 10px; --text-caption: 11px; --text-label: 12px; --text-body: 13px;
--tracking-display: -0.05em; --tracking-eyebrow: 0.16em;

/* 动效 */
--dur-120: 120ms; --dur-180: 180ms; --dur-240: 240ms;
--ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
--ease-soft: cubic-bezier(0.22, 0.61, 0.36, 1);
```

落地：新增 `.glass-surface`（`--glass-*` + `backdrop-filter:blur(var(--glass-blur))`）组件类，应用于 **header pills / hub 过滤栏 / image·video 浮动工具栏 / StageEmptyState / Dialog**；全部 `shadow-[0_…_color-mix…]` 改 `shadow-elevation-{1,2,3}`；sidebar/topbar/footer 1px 边改 `border-image: var(--hairline-soft)`。

### 3.3 新增功能交互规划（基于后端领先能力）

#### 3.3.1 Assistant（释放 agent 工具体系）

- **推理深度选择器**：`deepThink` 布尔 → `low/medium/high` 分段控件，透传 `reasoning_effort`。
- **系统提示 / 工具选择**：折叠式 "Custom instructions" 面板写 `system_prompt`；slash 命令（`/plan /skill /mcp /web_search`）映射到 `allowed_tools`/`tool_choice`。
- **编辑重发 / 重新生成**：最后一条 assistant bubble 加 regenerate；user bubble 加 edit→截断历史重提。
- **会话生命周期**：切换/卸载时 `agent.shutdown`（防线程泄漏）；附件/多模态输入（`ChatContentPart` 已存在）。
- **代码块复制 + 语言切换**（[assistant-markdown.tsx:46](packages/slab-desktop/src/pages/assistant/components/assistant-markdown.tsx#L50)）；消息级 copy 剥离 `<think>` 残留。

#### 3.3.2 Media（进度 + 转写 + 工作流闭环）

- **进度可视化**：消费 `progress`，渲染进度条 + 阶段（模型加载→采样→解码）+ ETA。
- **历史参数重跑**：image/video/audio 历史面板加"回填参数重跑"。
- **音频**：取消入口；SRT/VTT/TXT 导出；分段导航（点击时间戳跳播、逐段复制）。
- **视频**：结果"在工作区打开"；A/B 对比；批量。
- **任务列表**：利用 `?type=` 过滤 + progress 字段，替换全量客户端分页。

#### 3.3.3 Hub（模型管理闭环）

- **Load/Unload/Switch** 卡片动作（`runtime_state` 驱动）。
- **磁盘/VRAM 可见性**：展示 `size_bytes`，结合 `/v1/system/gpu` 给"显存适配"提示。
- **"Use" CTA**：按 `capabilities` 跳转 Assistant / Image。
- **分类基于 `capabilities`**，`inferModelCategory` 字符串匹配仅作 fallback。
- **失败卡片内重试**（持久化 last error）。

#### 3.3.4 Plugins（生命周期 + 事件闭环）

- **卸载**（`DELETE`，门控 `removable`）、**更新**（`updateAvailable` 触发 `/install`）、**商店/URL 安装**。
- **消费 JSON-RPC**：宿主 `usePluginRpcCall`（`/v1/plugins/rpc`）用于 WebView 外的插件命令调用。
- **消费事件总线**：`/v1/plugins/events` 订阅 → 安装/运行态实时 invalidate（替手动刷新）。
- **贡献的 settings/commands**：渲染进 Settings 页 / 命令面板。
- **导入进度**：`fetch`+`ReadableStream` 或 XHR upload-progress，含取消。
- **权限评审**：导入时解析 manifest 列权限；运行时首次拒绝时弹授权框。
- **WebView 崩溃恢复**：错误 Alert 加"Reload view"（unmount→remount）。

#### 3.3.5 Workspace（IDE 体验补齐）

- **"选中代码 → AI 解释"**：命令面板 + 编辑器右键，投递 `{relativePath, selection}` 到 assistant。
- **终端多标签/分屏/shell 选择**；`sendSignal` 扩展。
- **文件监听**：`/v1/workspace/watch`（SSE/WS）或 focus-regain 失效缓存。
- **大文件守卫**：HTTP read_file 服务端 `MAX_FILE_BYTES`，超限显示占位。
- **Monaco 懒启动**：`ensureWorkspaceLspServices` 延迟到编辑器/Explorer 真正渲染；52 个 service-override 包按需。
- **浏览器模式用 Monaco**（非 textarea），FS overlay 已支持 HTTP。
- **编辑器设置生效**：`editorSettings` 接入 VS Code configuration service（当前为 dead state）。
- **LSP 能力端点**：`/v1/workspace/lsp/providers` 驱动语言门控，删除 TS 硬编码表。

#### 3.3.6 设置与首次启动

- **设置搜索**：property title/description 关键字过滤。
- **autosave 未保存守卫**：全局 dirty set + 路由离开提示。
- **Setup 确认门**：非自动 provision；`runtime_payload_installed` 为真或显式 opt-in 才开始。
- **组件级 checklist**：渲染 `setupStatus` 的 ffmpeg/backends 安装状态与重试。

---

## 4. 落地执行规划与优先级（Action Items）

> 按"先止损、再释放、后打磨"分四阶段。`域` 列对应负责团队；`依赖`标注前置项。

### Phase 0 — 止损：正确性与安全（P0，立即）

| # | 行动 | 域 | 证据 | 预估 |
|---|---|---|---|---|
| 0.1 | 插件 `/api-request` HTTP 路由下线或强制 caller-token；`authorize_*_from_caller(None)` 改硬拒 | Plugins/安全 | [mod.rs:213](bin/slab-app/src-tauri/src/plugins/mod.rs#L213)、[assets.rs:61](crates/slab-app-core/src/domain/services/plugin/assets.rs#L61) | M |
| 0.2 | Workspace 未保存守卫统一到 Monaco working-copy service；`window.confirm`→AntD Modal | Workspace | [use-workspace-page.ts:280](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L280) | M |
| 0.3 | 统一 workspace 状态源（recent/config 入 `slab-app-core`），HTTP 不再返回空 | Workspace/后端 | [handler.rs:563](bin/slab-server/src/api/v1/workspace/handler.rs#L563) | M |
| 0.4 | Assistant 流式加 `AbortController`；`turn_failed` 保留部分内容 | Assistant | [use-assistant-agent.ts:250,709](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L709) | S |
| 0.5 | Restart 按钮门控 `model_download` | Media/Task | [task-detail-dialog.tsx:174](packages/slab-desktop/src/pages/task/components/task-detail-dialog.tsx#L174) | S |
| 0.6 | 全局 `prefers-reduced-motion` 守卫 | UI | grep 零命中 | S |

### Phase 1 — 释放后端能力（R1，高价值）

| # | 行动 | 域 | 依赖 | 预估 |
|---|---|---|---|---|
| 1.1 | 消费 `task.progress` → 媒体生成进度条/阶段 | Media | — | M |
| 1.2 | `reasoning_effort` 选择器 + `system_prompt` + slash→`allowed_tools` | Assistant | — | M |
| 1.3 | Hub Load/Unload/Switch + size/VRAM + capabilities 分类 + Use CTA | Hub | — | L |
| 1.4 | 音频取消 + SRT/VTT 导出 + 分段导航 | Media/Audio | — | M |
| 1.5 | 插件 uninstall/update/install-from-URL + 消费 `/rpc` + `/events` | Plugins | 0.1 | L |
| 1.6 | 统一错误包络：assistant 适配 `{code,message,data,i18n}`；补 4 类 code；`getLocalizedErrorMessage` 翻译 `i18n` | Infra | — | M |
| 1.7 | 全局 `MutationCache.onError` + retry 策略；删除散落 `retry:false` | Infra | 1.6 | S |
| 1.8 | 会话切换/unmount 调 `agent.shutdown` | Assistant | 0.4 | S |

### Phase 2 — 交互闭环与体验（R4）

| # | 行动 | 域 | 依赖 | 预估 |
|---|---|---|---|---|
| 2.1 | SSE resume（`Last-Event-ID`）+ 退避重连 | Assistant | 0.4 | M |
| 2.2 | 审批 `Map<callId>` + stop `pendingAbort` 竞态修复 | Assistant | — | S |
| 2.3 | Edit-and-resend / regenerate；代码块复制 | Assistant | — | M |
| 2.4 | 媒体"历史参数重跑"；视频"在工作区打开"/对比 | Media | — | M |
| 2.5 | BackendStatus 失败阈值 + 无抖动 + 重试 CTA；toast 去重；ui-state 失败可见 | Infra | 1.7 | S |
| 2.6 | Setup 确认门 + 组件 checklist；设置搜索 + autosave 守卫 | Setup/Settings | — | M |
| 2.7 | 选中代码→AI 解释；文件监听；大文件守卫；Monaco 懒启动 | Workspace | 0.2 | L |

### Phase 3 — 设计系统重构（R5）

| # | 行动 | 域 | 依赖 | 预估 |
|---|---|---|---|---|
| 3.1 | 落地 §3.2.6 新 Token（elevation/glass/motion/text/tracking） | UI | — | M |
| 3.2 | 硬分割线 → `--hairline-soft` 软渐变（sidebar/topbar/footer/header） | UI | 3.1 | S |
| 3.3 | 统一 `<StateSurface>`（empty/loading/error）+ `.focus-ring` | UI/Components | 3.1 | M |
| 3.4 | Skeleton shimmer；动效 Token + "soft in" 编排 | UI | 3.1 | S |
| 3.5 | Monaco ↔ app 主题 Token 映射；编辑器面板圆角协调 | Workspace/UI | 3.1 | M |

### 五大最高杠杆动作（若资源有限，优先这 5 项）

1. **Phase 1.1 消费 `task.progress`** —— 单点修复解锁全部媒体生成的"进度/阶段"体验，投入小、收益面最大。
2. **Phase 3.1 + 3.2 Token 化 + 软分割线** —— 一次收敛 ~60 处内联阴影/opacity、消除 1px 硬线，"通透感/无边界"即时可见。
3. **Phase 0.1 插件越权止损** —— 唯一安全 P0，必须先行。
4. **Phase 1.6 + 1.7 统一错误层** —— 让全域错误"说人话、能翻译、可重试"，消除 ~30 处不一致 toast。
5. **Phase 1.3 Hub 模型管理闭环** —— 把"下载完无处用/无法卸载"补齐，释放后端 load/unload 能力。

---

## 附录 A · 取证文件索引（关键前端文件）

- Assistant：[index.tsx](packages/slab-desktop/src/pages/assistant/index.tsx)、[hooks/use-assistant-agent.ts](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts)、[lib/assistant-agent-events.ts](packages/slab-desktop/src/pages/assistant/lib/assistant-agent-events.ts)、[lib/assistant-agent-state.ts](packages/slab-desktop/src/pages/assistant/lib/assistant-agent-state.ts)
- Media：[media-task-api.ts](packages/slab-desktop/src/lib/media-task-api.ts)、[use-image-generation.ts](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts)、[use-image-model-preparation.ts](packages/slab-desktop/src/pages/image/hooks/use-image-model-preparation.ts)、[use-video-generation.ts](packages/slab-desktop/src/pages/video/hooks/use-video-generation.ts)、[audio-workbench.tsx](packages/slab-desktop/src/pages/audio/components/audio-workbench.tsx)、[task-detail-dialog.tsx](packages/slab-desktop/src/pages/task/components/task-detail-dialog.tsx)
- Workspace：[workspace-bridge.ts](packages/slab-desktop/src/lib/workspace-bridge.ts)、[workspace-lsp.ts](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts)、[workspace-lsp-utils.ts](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp-utils.ts)、[use-workspace-page.ts](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts)、[workspace-workbench.tsx](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx)
- Plugins：[hooks/use-plugins-page.ts](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts)、[components/plugin-webview-page.tsx](packages/slab-desktop/src/pages/plugins/components/plugin-webview-page.tsx)、[lib/plugin-host-bridge.ts](packages/slab-desktop/src/lib/plugin-host-bridge.ts)
- Hub/Settings/Setup：[use-hub-model-catalog.ts](packages/slab-desktop/src/pages/hub/hooks/use-hub-model-catalog.ts)、[hub-catalog-table.tsx](packages/slab-desktop/src/pages/hub/components/hub-catalog-table.tsx)、[use-setup.ts](packages/slab-desktop/src/pages/setup/hooks/use-setup.ts)
- Infra：[packages/api/src/errors.ts](packages/api/src/errors.ts)、[lib/query-client.ts](packages/slab-desktop/src/lib/query-client.ts)、[store/ui-state-storage.ts](packages/slab-desktop/src/store/ui-state-storage.ts)、[components/backend-status.tsx](packages/slab-desktop/src/components/backend-status.tsx)
- UI/Design：[layouts/sidebar.tsx](packages/slab-desktop/src/layouts/sidebar.tsx)、[layouts/header.tsx](packages/slab-desktop/src/layouts/header.tsx)、[slab-components/src/styles/globals.css](packages/slab-components/src/styles/globals.css)、[slab-components/src/button.tsx](packages/slab-components/src/button.tsx)

## 附录 B · 后端能力面取证（关键后端文件）

- [bin/slab-server/src/error.rs](bin/slab-server/src/error.rs)（错误契约 `{code,message,data,i18n}` + 12 变体）
- [crates/slab-app-core/src/error.rs](crates/slab-app-core/src/error.rs)（`AppCoreErrorData` 结构化子码）
- [crates/slab-app-core/src/schemas/agent.rs](crates/slab-app-core/src/schemas/agent.rs)（`AgentConfigInput` 全能力字段）
- [bin/slab-server/src/api/v1/tasks/handler.rs](bin/slab-server/src/api/v1/tasks/handler.rs)（restart 仅 model_download）
- [bin/slab-server/src/api/v1/workspace/handler.rs](bin/slab-server/src/api/v1/workspace/handler.rs)（HTTP recent/config 恒空）
- [bin/slab-app/src-tauri/src/plugins/mod.rs](bin/slab-app/src-tauri/src/plugins/mod.rs)（caller-id 派生与越权点）
