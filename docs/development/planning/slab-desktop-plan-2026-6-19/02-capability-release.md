# 专项执行计划 · 后端能力释放 (Backend Capability Release)

| 字段 | 值 |
|---|---|
| Plan ID | B |
| 关联根因 | R1（能力释放失败） |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) |
| 负责域 | Assistant · Media · Hub · Plugins · Infra(errors) |
| 状态 | Implemented / Verified（Plan B scope） |
| 预估总工作量 | L |

## 1. 目标与边界 (Scope)
- **北极星**：把后端已实现、前端读取后丢弃（或无入口）的能力字段/端点/事件流全部释放为可见、可操作的产品体验，消除"功能强大但交互单薄"的核心症状。
- **In scope**：
  - 媒体生成进度/阶段/ETA（image/video/audio 三 workbench）
  - Assistant agent 工具体系（reasoning_effort 5 级 / system_prompt / slash → allowed_tools+tool_choice）
  - Hub 模型管理闭环（load/unload/switch + size/VRAM + capabilities 分类 + Use CTA）
  - 音频转写（取消 + SRT/VTT/TXT 导出 + 分段时间戳导航）
  - 插件生命周期（uninstall / update / install-from-URL）+ JSON-RPC + 事件总线消费
  - 统一错误契约（`{code,message,data,i18n}` 适配 + 补 4 类 code + `getLocalizedErrorMessage` + 结构化 data 渲染）
- **Out of scope（移交他计划）**：
  - Assistant 流式脆弱性（AbortController / `turn_failed` 保内容 / SSE resume）→ **Plan A（R2）**
  - Workspace `recent`/`config` 恒空、Monaco 懒启动 → **Plan A（R2）**
  - 设计系统 Token 化、软分割线、`<StateSurface>`、reduced-motion → **Plan D（R5）**
  - 插件 `/api-request` 越权止损（caller-token）→ **Plan A（R3，Phase 0.1 前置）**
  - 全局 `MutationCache.onError` + retry 策略的最终落地 → **Plan C（R2，与本计划的 T-B-7 协同）**
- **Definition of Done**：
  - [x] 7 张任务卡全部 AC 勾选
  - [x] `bun run check:frontend` / `bun run gen:api`（schema 变更后）/ `bun run test:frontend` 全绿
  - [x] `bun run lint` 零新增告警
  - [x] 浏览器验证覆盖 image/video/audio 进度、assistant 请求体、hub load/use、plugin URL install/uninstall/update；完整 `bun run test:browser` 仅剩 Settings/Setup 的 4 个视觉基线漂移，已确认不属于 Plan B
  - [x] 上游审计 §2.1 矩阵中本计划覆盖的 ❌ 项全部转 ✅ 或 ⚠️

## 2. 任务卡 (Task Cards)

### T-B-1 · 消费 `task.progress`（current/total/step/step_count）→ 媒体生成进度条/阶段/ETA
- **严重度** P0 · **类型** feat · **预估** M
- **证据** [media-task-api.ts](packages/slab-desktop/src/lib/media-task-api.ts)、[use-image-generation.ts:300-329](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L300)、[use-video-generation.ts:121-144](packages/slab-desktop/src/pages/video/hooks/use-video-generation.ts#L121)、[const.ts:29](packages/slab-desktop/src/pages/image/const.ts#L29)（`MAX_POLL_ATTEMPTS=150×2s`）
- **问题**：三个媒体 hook 通过 `/v1/tasks/{id}` 轮询，但只读 `status`/`result`，完全丢弃 `TaskProgressResponse { current, total, step, step_count }`。用户在数秒到数十秒的采样/解码过程中只能看到一个无语义的 spinner，无法判断阶段、剩余时间、是否卡死。
- **方案**：
  1. 在 `media-task-api.ts` 新增纯函数 `deriveProgress(progress: TaskProgressResponse | null)`：返回 `{ percent: number, stage: string, etaMs: number | null, stepLabel: string }`。
     - `percent = total>0 ? clamp(current/total*100,0,100) : null`；`step`/`step_count` → `stage` 文案（按 task_type 映射：image/video = "加载模型→采样→解码"，audio = "加载模型→VAD→转写→对齐"）。
     - `etaMs`：用两次轮询的 `current` 增量 × `POLL_INTERVAL_MS` 线性外推，做 EMA 平滑避免抖动；首次无基线返回 null。
  2. 在 image/video/audio hook 的 `useQuery` 选择器中解构 `taskStatus.progress`，调用 `deriveProgress`，写入新 state `progressInfo`。
  3. 抽出共享组件 `<GenerationProgress percent stage eta stepLabel />`（放 `packages/slab-desktop/src/pages/media/components/`），三 workbench 复用；`<Progress>` 用 AntD `Progress` + 自定义 `format`，阶段文案走 i18n。
  4. 修配套缺口：将 `MAX_POLL_ATTEMPTS` 上限上调并改为依据 task 终态终止（`status==='succeeded'|'failed'|'cancelled'` 才停），轮询间隔在 `percent<5` 时退避到 3s（模型加载阶段通常无 progress 更新），避免误杀长任务（审计 §3.1.4 P1）。
- **验收标准 (AC)**：
  - [x] image/video/audio 生成中渲染进度条，`percent` 平滑递增，卡顿不回退
  - [x] 阶段标签随 `step` 切换且可翻译（zh/en 各加 ≥4 个 key）
  - [x] ETA 在 ≥2 次轮询后出现，<2 次或 `total===0` 时不显示（不闪烁）
  - [x] 长任务（模拟 200s）不被 `MAX_POLL_ATTEMPTS` 误杀
  - [x] `deriveProgress` 纯函数有单测覆盖（`total=0`、`step>step_count`、EMA 平滑）
  - [x] 后端不返回 `progress`（旧字段）时降级为 spinner，不报错
- **依赖**：无（后端 `TaskProgressResponse` 已就绪）；与 T-B-7 协同（轮询错误 toast 经统一错误层去重）

### T-B-2 · Assistant 释放 agent 工具体系：reasoning_effort 选择器 + system_prompt + slash 命令映射 allowed_tools/tool_choice
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [assistant-agent-state.ts:24-49](packages/slab-desktop/src/pages/assistant/lib/assistant-agent-state.ts#L47)（`toAgentConfig` 仅设 `max_turns` + 硬编码 `reasoning_effort:'medium'`）、[agent.rs:20-39](crates/slab-app-core/src/schemas/agent.rs#L20)（`AgentConfigInput` 全字段：`system_prompt/allowed_tools/tool_choice/tool_concurrency/structured_output/verbosity`）、[chat.rs:160-166](crates/slab-app-core/src/schemas/chat.rs#L160)（`ChatReasoningEffort { None, Low, Medium, High, Minimal }`）、[assistant-composer.tsx:83](packages/slab-desktop/src/pages/assistant/components/assistant-composer.tsx#L83)
- **问题**：后端 `AgentConfigInput` 暴露了完整 agent 工具体系，但 `toAgentConfig` 只透传布尔 `deepThink`（硬编码 medium，`none/low/high/minimal` 不可达），`system_prompt/allowed_tools/tool_choice/tool_concurrency` 完全未透出；slash 命令（`/plan /skill /mcp /web_search`）只是文本前缀，未映射到 `allowed_tools`/`tool_choice`。
- **方案**：
  1. 重构 `toAgentConfig(model, presets, options)`，入参由 `deepThink:boolean` 改为 `options: { reasoningEffort: ChatReasoningEffort, systemPrompt?: string, allowedTools?: string[], toolChoice?: AgentToolChoiceInput, toolConcurrency?: number }`。所有字段按 `AgentConfigInput` schema 条件展开（沿用现有 `runtimePresets?.x` 模式）。
  2. UI：
     - 把 composer 顶栏的 `deepThink` 开关替换为 `reasoningEffort` 分段控件（`none/low/medium/high` 四档，`minimal` 进高级面板），i18n 标签。
     - 新增折叠面板 "Custom instructions"（默认收起，记住状态到 `ui-state`）→ 写 `system_prompt`（textarea，maxlength 与后端对齐）。
     - 高级面板内：`tool_concurrency` 数字输入（1..4，后端 `MAX_TOOL_CONCURRENCY=4`，见 agent.rs:265-271）、`tool_choice` 下拉（auto/none/required/tool{name}）。
  3. Slash 命令映射：在 composer 输入解析处（`assistant-composer.tsx`）维护 `SLASH_COMMAND_TOOLS` 表：`/plan→['plan']`、`/web_search→['web_search']`、`/mcp→['mcp_*']`、`/skill→['skill_*']`。命中 slash 时将并集写入 `allowed_tools` 并设 `tool_choice='required'`（首个命令）或 `'auto'`（多命令）；无 slash 时 `allowed_tools` 留空（后端 default 全开）。
  4. 状态持久化：`reasoningEffort` 与 `systemPrompt` 走 `ui-state`（key `assistant.config`），跨会话保留。
- **验收标准 (AC)**：
  - [x] 切换 `reasoningEffort` 后，提交请求体 `config.reasoning_effort` 精确匹配（抓包验证 none/low/medium/high 四档）
  - [x] "Custom instructions" 文本经 `config.system_prompt` 透传，后端 agent 行为可观察（如系统提示中带 "回答用中文"）
  - [x] 输入 `/web_search xxx` 后请求体 `allowed_tools` 含 `'web_search'`、`tool_choice.type==='required'`
  - [x] `tool_concurrency` 输入 0 或 5 被前端 clamp 到 1..4（与后端 validator 一致，提前拦截 400）
  - [x] 高级面板收起态持久化（刷新后保持）
  - [x] 无 slash 输入时 `allowed_tools` 不发送（保留后端默认全开语义）
- **依赖**：无；与 Plan A（R2）的 AbortController 协同但不阻塞（两者改不同文件段）

### T-B-3 · Hub 模型管理闭环：load/unload/switch 动作 + size/VRAM 可见 + capabilities 分类 + "Use" CTA
- **严重度** P1 · **类型** feat · **预估** L
- **证据** [use-hub-model-catalog.ts:357-404](packages/slab-desktop/src/pages/hub/hooks/use-hub-model-catalog.ts#L357)（`inferModelCategory` 字符串匹配）、[hub-catalog-table.tsx:261](packages/slab-desktop/src/pages/hub/components/hub-catalog-table.tsx#L261)、[models/handler.rs:79-82](bin/slab-server/src/api/v1/models/handler.rs#L79)（`/models/load|/unload|/switch` 已注册）、[footer-status-bar.tsx:68](packages/slab-desktop/src/layouts/footer-status-bar.tsx#L68)（GPU 信息已拉取但未与模型 size 关联）
- **问题**：后端已实现 `POST /v1/models/{load,unload,switch}` 与 `capabilities`/`size_bytes`/`runtime_state`，但 Hub 零管理动作；用户下载完模型后无法手动驻留/驱逐，也无法判断"这个模型能否塞进我的显存"。分类靠 `inferModelCategory` 文件名字符串匹配（haystack includes），`capabilities` 仅作辅助，逻辑脆弱（如 `vision` 命中 `image_embedding` 会误判）。
- **方案**：
  1. 新增 mutations（`api.useMutation`）：`post /v1/models/load`、`post /v1/models/unload`、`post /v1/models/switch`；onSuccess invalidate `['/v1/models']` 与 `/v1/backends/status`（驻留态影响后端）。
  2. 卡片/行操作区（`hub-catalog-table.tsx`）按 `runtime_state` 渲染：
     - `loaded` → "Unload" 按钮（confirm modal，提示将释放 VRAM）。
     - `ready`/`unloaded` → "Load" 按钮（异步 pending 态）。
     - 多模型同 backend 时提供 "Switch" 下拉（切换驻留目标）。
  3. 列展示 `size_bytes`（humanize，如 `4.2 GB`）；结合 `useGpuInfo`（footer-status-bar 已拉取 `/v1/system/gpu`）计算 `size / vram_total`，超 90% 显示 amber 警告 "可能超出显存，将走 CPU offload"。
  4. 重构分类：新增 `classifyByCapabilities(capabilities: string[]): ModelCategory`（capabilities 优先：`chat_generation|text_generation→language`、`image_generation|video_generation→vision`、`audio_transcription|audio_vad→audio`、`*embedding→embedding`），`inferModelCategory` 降级为 fallback（capabilities 为空时才用）。删掉 `vision` 同时命中 `image_embedding` 的 bug。
  5. "Use" CTA：按分类跳转——`language→/assistant`、`vision→/image`、`audio→/audio`、`embedding→禁用(disabled)`。CTA 在模型 `runtime_state!=='loaded'` 时先触发 load 再跳转（或 toast "已开始加载，完成后可用"）。
- **验收标准 (AC)**：
  - [x] 已下载模型卡片显示 Load/Unload 按钮，点击后 `runtime_state` 在 ≤2s 内刷新（invalidate 生效）
  - [x] size 列正确 humanize；VRAM 占用 >90% 显示警告，≤90% 不显示
  - [x] capabilities 含 `image_generation` 的模型分类为 `vision`（不再因文件名含 `image` 误判 embedding）
  - [x] "Use" 按钮按分类正确跳转；未加载模型点击 Use 触发 load（toast 提示）
  - [x] Load/Unload 失败时卡片内显示错误 + 重试（持久化 last error，复用 T-B-7 的结构化 data）
  - [x] 并发 Load 同 backend 第二个请求被后端 409 Conflict 时，前端显示 "请先卸载当前模型"（经 T-B-7 翻译）
- **依赖**：T-B-7（错误契约 + Conflict 翻译）；GPU info query 已存在无需新增

### T-B-4 · 音频转写：取消入口 + SRT/VTT/TXT 导出 + 分段（时间戳）导航
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [audio-workbench.tsx:407,536](packages/slab-desktop/src/pages/audio/components/audio-workbench.tsx#L407)、[audio.rs:296-300](crates/slab-app-core/src/schemas/audio.rs#L296)（`AudioTranscriptionResultData { text, segments: Vec<TimedTextSegmentResponse> }`）、[tasks.rs:42-44](crates/slab-app-core/src/schemas/tasks.rs#L42)（result.segments 也带时间戳）、[handler.rs:124](bin/slab-server/src/api/v1/tasks/handler.rs)（`/v1/tasks/{id}/cancel`）
- **问题**：音频转写全程 spinner 无取消入口；结果仅渲染 `result.text` 纯文本 `<pre>`，`segments[]`（带 `start/end` 时间戳）被丢弃；无 SRT/VTT 导出，用户只能手动复制。
- **方案**：
  1. 取消入口：转写 running 态显示 "Cancel" 按钮，调用已有 `cancelTaskMutation`（`POST /v1/tasks/{id}/cancel`）；await 成功再清状态（不复用旧 `handleCancel` 不等后端的反模式，审计 §3.1.1 P1）。
  2. 分段渲染：新增 `<TranscriptSegments segments={result.segments} audioUrl={...} />`，每段显示 `[mm:ss.ms → mm:ss.ms]` 时间戳 + 文本；点击时间戳 seek 音频播放器（若 workbench 内嵌 `<audio>` 则 `audio.currentTime = start`）；逐段 hover 显示复制按钮。
  3. 导出：新增 `packages/slab-desktop/src/pages/audio/lib/export-transcript.ts`：
     - `toSrt(segments)`：`1\n00:00:01,000 --> 00:00:03,500\n文本\n`（标准 SRT，时间戳 hh:mm:ss,mmm）
     - `toVtt(segments)`：`WEBVTT\n\n00:00:01.000 --> 00:00:03.500\n文本\n`
     - `toTxt(result.text)`：纯文本
     - 导出按钮组（SRT/VTT/TXT），用 `Blob` + `URL.createObjectURL` + `<a download>`，文件名 `{task_id}.srt`。
  4. 空段降级：`segments` 为空（某些 backend 不返回分段）时仅显示纯文本 + 只提供 TXT 导出，不显示 SRT/VTT。
- **验收标准 (AC)**：
  - [x] 转写中显示 Cancel 按钮，点击后 task 状态变 `cancelled`，UI 返回 idle（不等后端则不清状态）
  - [x] 结果区按段渲染，时间戳格式正确；点击时间戳音频 seek 到对应位置
  - [x] SRT/VTT 导出文件可用 VLC/PotPlayer 正确加载字幕（时间戳与音频对齐）
  - [x] TXT 导出为纯文本（无时间戳）
  - [x] `segments=[]` 时不显示 SRT/VTT 按钮，不报错
  - [x] 复制单段文本可用
- **依赖**：T-B-7（cancel 失败翻译）；与 T-B-1 共享 `GenerationProgress`（audio 也显示进度）

### T-B-5 · 插件生命周期 UI：uninstall（DELETE，门控 removable）+ update（updateAvailable）+ install-from-URL（/install）
- **严重度** P0 · **类型** feat · **预估** L
- **证据** [handler.rs:341-348](bin/slab-server/src/api/v1/plugins/handler.rs#L341)（`DELETE /v1/plugins/{id}` 已实现）、[handler.rs:111-116](bin/slab-server/src/api/v1/plugins/handler.rs#L111)（`POST /v1/plugins/install` 接 `packageUrl+sha256`）、[plugin.rs:30,70-72](crates/slab-app-core/src/schemas/plugin.rs#L30)（`InstallPluginRequest.package_url`、`PluginResponse.available_version/removable/updateAvailable`）、[use-plugins-page.ts](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts)、[utils.ts:44](packages/slab-desktop/src/pages/plugins/utils.ts#L44)（`pluginSummaryMessage` 已识别 `updateAvailable`）
- **问题**：插件无卸载入口（`removable` 字段已就绪却未消费）、无更新入口（`updateAvailable`/`available_version` 已透出但无 `/install` 触发）、安装仅支持本地 `.plugin.slab` 导入，无 URL/商店安装。
- **方案**：
  1. 新增 mutations：`delete /v1/plugins/{id}`、`post /v1/plugins/install`（body `{ package_url, sha256? }`）；onSuccess invalidate `['/v1/plugins']`。
  2. 卡片动作区按状态渲染：
     - 卸载：`removable===true` 时显示 "Uninstall"（AntD `Modal.confirm`，二次确认，提示将删除配置与文件）；`removable===false`（系统内置）禁用并 tooltip 说明。
     - 更新：`updateAvailable===true` 时显示 "Update to {available_version}"，点击触发 `POST /v1/plugins/install`（package_url 取 `source_ref` 或 manifest 中的更新源）。
     - URL 安装：页面顶部新增 "Install from URL" 入口（Modal：url + 可选 sha256），提交后 `/install`；进度走 T-B-6 的 `/v1/plugins/events`（若未订阅则降级为轮询 `/v1/plugins/{id}`）。
  3. 导入 `.slab` 包补进度（审计 §2.1 `/import-pack ⚠️`）：改用 XHR `upload.onprogress` 或 `fetch`+`ReadableStream`，1GB 上限带百分比 + 取消按钮。
  4. 权限预览（可选，M+）：install/URL 安装提交前解析 manifest 列出申请权限，用户确认后才安装（运行时首次拒绝的授权框属于 Plan A R3 范畴，此处仅 install-time 预览）。
- **验收标准 (AC)**：
  - [x] `removable===true` 的插件可卸载，二次确认后调用 DELETE，成功后从列表消失
  - [x] `removable===false` 的插件卸载按钮禁用，hover 显示原因
  - [x] `updateAvailable` 插件显示 Update 按钮，点击后版本号刷新为 `available_version`
  - [x] "Install from URL" 可安装远程 `.plugin.slab`（含可选 sha256 校验，失败 400 经 T-B-7 翻译）
  - [x] 本地导入 `.slab` 显示上传百分比与取消按钮
  - [x] 安装/更新/卸载失败时卡片内显示错误（不全局 toast 覆盖）
- **依赖**：**Plan A Phase 0.1**（`/api-request` 越权止损，安全前置）；T-B-6（事件订阅做实时刷新）；T-B-7（错误翻译）

### T-B-6 · 宿主消费 `/v1/plugins/rpc`（JSON-RPC）+ `/v1/plugins/events`（WS 实时刷新）
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [handler.rs:118-129](bin/slab-server/src/api/v1/plugins/handler.rs#L124)（`WS /v1/plugins/rpc` JSON-RPC 2.0 派发）、[handler.rs:131-162](bin/slab-server/src/api/v1/plugins/handler.rs#L144)（`WS /v1/plugins/events` broadcast）、[plugin-host-bridge.ts:289](packages/slab-desktop/src/lib/plugin-host-bridge.ts#L289)
- **问题**：JSON-RPC 派发与事件总线后端已就绪，但仅 Tauri 子 WebView 可调；宿主 UI 从不消费 `/rpc`，也不订阅 `/events`，导致安装/启停/运行态变更需手动刷新，宿主层无法调用插件命令。
- **方案**：
  1. 新增 `packages/slab-desktop/src/lib/plugin-events.ts`：封装 WS 客户端连接 `/v1/plugins/events`，提供 `usePluginEvents()` hook（React Context，单连接，自动重连+退避）。事件类型按后端 broadcast schema 映射（`plugin.installed|updated|removed|enabled|disabled|started|stopped|state_changed`）。
  2. 在 `use-plugins-page.ts` 订阅事件，按 `event.type + pluginId` invalidate `['/v1/plugins']` 或 `['/v1/plugins/{id}']`，删除手动 refetch 按钮。
  3. 新增 `usePluginRpcCall()`（`/v1/plugins/rpc`）：暴露 `rpcCall(pluginId, method, params)`，发送 JSON-RPC 2.0 `{ jsonrpc:'2.0', id, method, params }`，Promise 等待对应 id 的 response（result/error）。用于宿主层调用插件贡献的命令（如 settings 面板调用插件导出的配置校验）。
  4. 连接生命周期：路由进入 plugins 页建立，离开销毁；连接状态（connecting/connected/disconnected）在 footer 或页面角标显示（审计 R4 反馈缺失）。
  5. 错误处理：JSON-RPC `error` 帧 → 转为 `ApiError`（复用 T-B-7）；WS 断连 toast "插件事件连接已断开，将重试"（去重）。
- **验收标准 (AC)**：
  - [x] 在另一客户端（或后端测试）触发插件安装/启停时，宿主列表自动刷新（无需手动点刷新）
  - [x] `usePluginRpcCall(pluginId, 'ping', {})` 能拿到正确 result；后端返回 error 时 hook reject 为 `ApiError`
  - [x] WS 断连后指数退避重连（1s→2s→4s→max 30s），重连成功后补 invalidate 一次
  - [x] 离开 plugins 页 WS 连接关闭（无泄漏，devtools Network 验证）
  - [x] 连接状态在 UI 可见（角标）
- **依赖**：T-B-7（错误统一）；与 T-B-5 协同（安装进度可走 events 而非轮询）

### T-B-7 · 统一错误契约：assistant 适配服务端 `{code,message,data,i18n}` 包络 + 补 Conflict/NotImplemented/TooManyRequests/data.code 映射 + `getLocalizedErrorMessage` 翻译 i18n + 渲染结构化 data（suggestion/attempts）
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [errors.ts:91,124,175](packages/api/src/errors.ts#L91)（`getUserMessage` 缺 4009/4029/5010；`ErrorCodes` 缺 3 类）、[assistant-request-errors.ts:46](packages/slab-desktop/src/pages/assistant/lib/assistant-request-errors.ts#L46)（assistant 假定 OpenAI 形 `{error:{type,code}}`，失败不产生 `ApiError`）、[error.rs:35-45](bin/slab-server/src/error.rs#L35)（`CONFLICT=4009 / NOT_IMPLEMENTED=5010 / TOO_MANY_REQUESTS=4029`）、[error.rs (AppCoreErrorData)](crates/slab-app-core/src/error.rs#L13)（`UnsupportedChatParameter / ModelDownloadUnavailable{suggestion} / RuntimeFailure{runtime_code,detail}`）
- **问题**：服务端统一返回 `{code,message,data,i18n}`（`error.rs:24-32`），但前端 `ErrorCodes` 缺 `Conflict/NotImplemented/TooManyRequests`（429/501 落入 default 显示 "unexpected error"）；assistant 域假定 OpenAI 形 `{error:{...}}` 导致失败不产生 `ApiError`，丢失可重试性分类与共享 toast；`i18n` 字段被解析但从不翻译；结构化 `data`（如 `ModelDownloadUnavailable.suggestion`、`RuntimeFailure.runtime_code`）被丢弃。
- **方案**：
  1. `packages/api/src/errors.ts`：
     - `ErrorCodes` 补 `CONFLICT:4009`、`TOO_MANY_REQUESTS:4029`、`NOT_IMPLEMENTED:5010`。
     - `getUserMessage` 的 switch 补 3 类文案（可重试提示：4029 "请求过于频繁，请稍后重试"、409 "资源冲突，请刷新后重试"、501 "该功能暂未实现"）。
     - 新增 `getLocalizedErrorMessage(error, t, locale)`：优先用 `error.i18n[locale]`（`{key, params}`）经 i18next `t(key, params)` 翻译；无 i18n 时降级 `getUserMessage()`。
     - 新增 `getErrorData<T = AppCoreErrorData>(error): T | undefined` 类型化访问 `error.data`；定义 TS 镜像类型 `AppCoreErrorData`（与 [crates/slab-app-core/src/error.rs:15](crates/slab-app-core/src/error.rs#L15) 对齐，`unsupported_chat_parameter/model_download_unavailable/runtime_failure` 三变体，经 `gen:api` 或手写）。
     - 新增 `isRetryable(error)`：`code===4029 || isServerError() && code!==5010`，供 mutation retry 与 UI "重试" 按钮共用。
  2. `assistant-request-errors.ts`：把对 `{error:{type,code}}` 的解析改为先尝试服务端 `{code,message,data,i18n}`（构造 `ApiError`），失败再降级旧 OpenAI 形；保证 assistant 失败也产出 `ApiError`，接入统一 toast。
  3. 结构化 data 渲染：新增 `<ErrorDataDetail data={error.data} />` 组件，按 `data.code` 分支渲染：
     - `model_download_unavailable` → 显示 `reason` + `suggestion`（高亮可操作建议）。
     - `runtime_failure` → 显示 `runtime_code` 标签 + `detail` 折叠（调试用，默认收起）。
     - `unsupported_chat_parameter` → 显示 `param` 名（"不支持的参数：{param}"）。
  4. 集成点：MutationCache（Plan C）的 `onError` 默认用 `getLocalizedErrorMessage`；本卡片提供该函数，Plan C 负责挂载与全局去重。各域 catch 块逐步迁移到 `getLocalizedErrorMessage` + `<ErrorDataDetail>`。
- **验收标准 (AC)**：
  - [x] 触发 409（如重复 load 同 backend）显示 "资源冲突，请刷新后重试"；429 显示 "请求过于频繁"；501 显示 "该功能暂未实现"（不再显示 "unexpected error"）
  - [x] assistant 请求失败（如 unsupported param）产生 `ApiError`，toast 与其他域一致
  - [x] `i18n` 字段存在时按当前 locale 翻译（zh/en 切换验证）；不存在时降级英文文案
  - [x] `ModelDownloadUnavailable` 错误显示 `suggestion`（如 "请在 Hub 下载该模型后重试"），`RuntimeFailure` 显示 `runtime_code` 标签
  - [x] `isRetryable(4029)===true`、`isRetryable(5010)===false`、`isRetryable(5003)===true`
  - [x] TS 类型 `AppCoreErrorData` 与后端 `error.rs:15` 三变体对齐（`gen:api` 后或手写校验）
- **依赖**：**Plan C**（MutationCache 全局 onError 挂载点，本卡片仅提供 `getLocalizedErrorMessage` 函数）；被 T-B-1/3/4/5/6 共同依赖（错误翻译是基础）

## 3. 执行顺序 (Sequencing)
- **M1（基础层）**：T-B-7（错误契约）。被其余 6 卡共同依赖，必须先行；产出 `getLocalizedErrorMessage` / `ErrorDataDetail` / 补全 `ErrorCodes`。
- **M2（独立高价值，可并行）**：T-B-1（媒体进度，P0）、T-B-2（assistant 工具体系）。两者互不依赖，分两人并行；T-B-1 是审计"五大最高杠杆动作"第 1 项。
- **M3（管理闭环，部分并行）**：T-B-3（hub 闭环，依赖 T-B-7）、T-B-4（音频，依赖 T-B-7，可与 T-B-1 共享 `GenerationProgress`）。两者可并行。
- **M4（插件域，串行内部）**：T-B-6（rpc+events）→ T-B-5（生命周期 UI，依赖 T-B-6 的 events 做安装进度、依赖 Plan A Phase 0.1 越权止损前置）。
- **关键路径**：T-B-7 → T-B-1/T-B-3 → 集成测试。T-B-7 是单点阻塞，须保证 M1 不 slip。
- **可并行最大宽度**：M2、M3 各 2 人并行；T-B-5/T-B-6 与 M3 可重叠（不同域）。

## 4. 风险与缓解
| 风险 | 概率×影响 | 缓解 |
|---|---|---|
| T-B-7 slip 阻塞全域 | 中×高 | M1 仅做错误层，scope 严格收口（不重构 MutationCache，挂载交 Plan C）；先补 `ErrorCodes` + `getLocalizedErrorMessage` 即可解锁下游 |
| 后端 `progress.step` 语义未文档化（T-B-1 阶段文案猜错） | 中×中 | 实现前读 `bin/slab-runtime` 任务执行器确认 step 枚举；无法确认时阶段文案做 i18n 占位 + 后续微调，不阻塞 percent/ETA |
| Hub Load/Unload 触发后端 VRAM 竞态（T-B-3 多模型同 backend） | 中×中 | Load 前 confirm "将卸载当前驻留模型"；后端 409 经 T-B-7 翻译为可操作文案；UI 显示 pending 直到 `/v1/backends/status` 确认 |
| 插件 install-from-URL 无 sha256 时校验缺失（T-B-5 安全） | 低×高 | URL 安装默认要求 sha256（与后端 `InstallPluginRequest` 对齐）；缺省时 UI 警告 "未校验完整性"但仍允许（与本地导入一致风险面） |
| JSON-RPC WS 与 Tauri 子 WebView 双消费产生事件重复（T-B-6） | 低×低 | 宿主只订阅 `/events`（只读广播），不重复 `/rpc` 调用；事件去重按 `pluginId+event.type+ts` |
| assistant `toAgentConfig` 重构破坏现有请求（T-B-2 回归） | 中×高 | 入参改对象但保留旧 `deepThink` 调用点临时兼容（`deepThink?true → reasoningEffort:'medium'`）；加契约测试断言请求体 shape；M2 末统一清理兼容层 |
| `AppCoreErrorData` TS 类型与后端漂移（T-B-7） | 中×中 | 优先经 `bun run gen:api` 从 OpenAPI 生成；手写时加单测对比后端 `error.rs` 三变体字段名 |

## 5. 验证与回归 (Verification)
- 类型/契约：`bun run gen:api`、`bun run check:frontend`
- 单测/组件：`bun run test:frontend`、`bun run test:components`
- E2E：`bun run test:browser` 的 Plan B 触达文件通过；完整套件当前仅剩 Settings/Setup 4 个视觉基线漂移，不计入本专项
- Lint：`bun run lint`

## 6. 收口记录
- 状态：Plan B 已实现并按代码事实验收；文档从 Draft 收口为 `Implemented / Verified（Plan B scope）`
- 代码事实偏差：
  - `UnifiedModelResponse.size_bytes` 已由本地 `spec.local_path` 和已物化 artifacts 聚合投影，远程/未下载返回 `null`
  - 插件安装请求继续使用当前生成契约 `{ pluginId, packageUrl, packageSha256?, sourceId?, version? }`，没有改成文档里的 snake_case body
  - `/v1/plugins/events` 的客户端事件仍是 `{ plugin_id, topic, data, ts }`，`usePluginRpcCall` 只封装 JSON-RPC 2.0 的 `id/result/error`
  - `updateAvailable` / `availableVersion` 继续由后端现有投影字段驱动，未改安全模型
- 验证摘要：
  - `bun run check:frontend`
  - `bun run test:frontend`
  - `bun run test:components`
  - `bun run lint`
  - `cargo fmt`
  - `cargo test -p slab-app-core`
  - `cargo test -p slab-app`
  - `cargo test -p slab-server`
  - `bun run gen:api`
