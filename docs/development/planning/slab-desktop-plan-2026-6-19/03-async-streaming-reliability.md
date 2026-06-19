# 专项执行计划 · 异步与流式状态对齐 (Async & Streaming Reliability)

| 字段 | 值 |
|---|---|
| Plan ID | C |
| 关联根因 | R2（状态对齐脆弱） |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) |
| 负责域 | Assistant · Media · Hub · Infra(QueryClient/stores) |
| 状态 | Draft / Pending Review |
| 预估总工作量 | L |

## 1. 目标与边界 (Scope)

- **北极星**：异步/流式/持久化状态一律以后端为唯一真相源（single source of truth），UI 永远可取消、可恢复、可确认，任何中断都不丢已生成内容、不空烧 token、不泄漏服务端线程。
- **In scope**：
  - Assistant 流式协议层（WS/SSE）的取消、恢复、`turn_failed` 内容保留、会话生命周期 `shutdown`。
  - Media 任务轮询的终态收敛、取消确认、退避与上限。
  - Hub 模型状态双源合并（后端 `status` 为准，`downloadTracking` 退化为纯进度条数据）。
  - Infra：`QueryClient` 全局 retry 与 `MutationCache.onError`、`BackendStatus` 抗抖动、`ui-state-storage` 失败可见化、toast 去重。
- **Out of scope（移交他计划）**：
  - 媒体 `progress` 字段消费与进度条 UI（R1 能力释放）→ **Plan A / §3.1.4**。
  - 统一错误包络 `{code,message,data,i18n}` + `getLocalizedErrorMessage` + `isServerError` 实现（**Plan B / T-B-7**，本计划仅依赖其产出的 `isServerError` 判别器，并提供 mock 兜底以解耦）。
  - 审批 `Map<callId>` 与 `pendingAbort` 竞态修复（同域但属交互瑕疵 R4）→ **Plan D（交互）**。
  - Workspace HTTP↔Tauri 状态源割裂（R2，但归 Workspace 域）→ **独立 Workspace 计划**。
- **Definition of Done**：
  - [ ] T-C-1..T-C-8 全部 AC 通过，`bun run check:frontend && bun run test:frontend && bun run lint` 全绿。
  - [ ] E2E 流式中断/重连/网络抖动/进程切换 4 个场景通过（见 §5）。
  - [ ] 服务端 `agent.shutdown` 在会话切换/unmount 时被调用（可通过日志埋点或线程计数验证，无泄漏）。
  - [ ] 前端不再存在任何「先 clear 再 await」或「直接 `console.warn` 吞错」的状态对齐反模式（grep 守卫）。

## 2. 任务卡 (Task Cards)

### T-C-1 · Assistant 流式加 `AbortController` 取消 + 关闭 EventSource/WS
- **严重度** P0 · **类型** bugfix · **预估** M
- **证据** [use-assistant-agent.ts:709](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L709)、[use-assistant-agent.ts:441](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L441)、[use-assistant-agent.ts:492](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L492)
- **问题**：`abort()` 仅向服务端发送 `agent.interrupt`，但本地传输通道（`socketRef` 的 WebSocket、`eventSourceRef` 的 EventSource、`responsesMutation` 的 fetch）始终存活。一旦 interrupt 帧因网络/调度丢失，服务端虽停但客户端仍在收 `assistant_delta`，token 空烧；提交后 ack 返回 `thread_id` 前点 Stop 还会被 `if (!threadId) return` 直接吞掉（§3.1.2）。
- **方案**：
  1. 引入 `abortControllerRef = useRef<AbortController | null>(null)`；每次 `responsesMutation.mutateAsync({ body, signal })` 创建新 controller；`openSse` 与 WS `send` 前确认 controller 未 abort。
  2. `abort()` 改为「双轨」：(a) 若已有 `threadId` → 发 `agent.interrupt`；(b) 无论 threadId 是否就绪，立即 `abortControllerRef.current?.abort()` 并 `eventSourceRef.current?.close()` / `socketRef.current?.close()`，本地状态置 `interrupting`。
  3. 新增 `pendingAbortRef`：提交后 threadId 尚未返回时点 Stop，记录 `pendingAbort = true`；`postAgentCommand` 拿到 `threadId` 后若 `pendingAbort` 则立即补发 interrupt。
  4. unmount（`useEffect` cleanup）调用同一 `abort()` 路径（无 threadId 时只做本地 close + abort controller）。
  5. `responsesMutation` 的 `onError` 区分 `AbortError`（name === 'AbortError' 或 `signal.aborted`）→ 静默，不 toast；其它走 T-C-6 的全局 onError。
- **客户端状态机要点**（abort 维度）：
  - 状态：`idle → submitting → streaming → interrupting → cancelled` / `... → failed`
  - `submitting`（threadId 未就绪）：本地 AbortController 已生效；`pendingAbort` 在此态可记录。
  - `streaming`：`abort()` 同时发 interrupt + 关通道。
  - `interrupting`：等 `turn_cancelled`（T-C-2 保证内容保留）或 5s 超时强制转 `cancelled`。
- **验收标准 (AC)**：
  - [ ] 提交后 thread_id 到达前点 Stop，本地停止收流且 ack 后自动发 interrupt（E2E：mock 服务端延迟 ack 2s）。
  - [ ] 流式中点 Stop 后再无新 token 追加（grep 网络面板，EventSource 已 `readyState: CLOSED`）。
  - [ ] 切换会话 / unmount 时 WS+EventSource 全部 `close()`，无悬挂 listener。
  - [ ] `responsesMutation` 被 abort 时不上报错误 toast，`status` 收敛到 `interrupted`。
- **依赖**：与 T-C-2 同改一文件，建议合并实现；不阻塞其它。

### T-C-2 · `turn_failed` 保留已流式部分内容（仿 `turn_cancelled`）
- **严重度** P0 · **类型** bugfix · **预估** S
- **证据** [use-assistant-agent.ts:250](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L250)（`appendAssistantError` 整体覆盖 `content`）、对比 [use-assistant-agent.ts:316](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L316) `turn_cancelled` 分支不覆盖内容
- **问题**：`turn_failed` 当前调用 `appendAssistantError(content)`，把错误字符串直接写入 `message.content`，已流式输出的 90% 回答被覆盖，无法恢复、无法复制。
- **方案**：
  1. 新增 `markAssistantTurnFailed(errorInfo)`：仅 `updateLastAssistantMessage` 将 `status` 置 `error`，并写入独立字段 `errorFooter: { code, message, retryable }`（在 `AssistantMessage` 类型上扩展，不污染 `content`）。
  2. 若 `messages` 末尾非 assistant 气泡（无任何 delta 到达），则追加一个纯错误气泡（保留原 fallback 行为）。
  3. UI 层在 assistant 气泡底部渲染 `errorFooter`（红色细条 + 「重试」「复制已生成内容」CTA，重试复用最近一条 user message）。
  4. `turn_cancelled` 同样追加一个 `cancelledFooter` 标记（视觉弱化），保持两条终态路径一致。
- **验收标准 (AC)**：
  - [ ] 模拟服务端在第 N 个 token 后返回 `turn_failed`：已生成内容完整保留，错误信息以独立 footer 呈现，气泡 `status=error`。
  - [ ] 「复制」按钮得到的是已生成文本而非错误字符串。
  - [ ] 「重试」重发最后一条 user 消息并复用其 `AgentConfigInput`。
  - [ ] 无 delta 的纯错误场景仍正常追加错误气泡（回归）。
- **依赖**：建议与 T-C-1 同 PR；UI footer 可后置。

### T-C-3 · SSE resume（回传 Last-Event-ID）+ 退避重连
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [use-assistant-agent.ts:441](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L441)（`openSse` 无 `Last-Event-ID`、error 仅 `setEventsConnected(false)`）、[agent/handler.rs:339](bin/slab-server/src/api/v1/agent/handler.rs#L339) + [handler.rs:437](bin/slab-server/src/api/v1/agent/handler.rs#L437) `parse_last_event_id` 已实现服务端 replay
- **问题**：服务端 `agent_events_sse` 已支持读取 `Last-Event-ID` 头并 replay `id > last_event_id` 的事件（handler.rs:430 `should_replay_event`），前端却从不回传，断线后丢失中间事件、不自动重连。
- **方案**：
  1. `handleTransportPayload` 中维护 `lastSeenEventIdRef`（取自 `agentEventKey` 解析出的 numeric id；每个 SSE `Event` 已带 `.id`，需让 `EventSource` 的 `message` 监听读 `message.lastEventId`）。
  2. `openSse` 改造：因浏览器原生 `EventSource` 无法自定义请求头，采用「URL query 兜底」方案——与服务端确认是否接受 `?last_event_id=`（若否，则改用 `fetch` + `ReadableStream` 解析 SSE，可控 headers）。**首选 fetch-stream**：一次性解决 header + `AbortController`（与 T-C-1 共享）+ 关闭语义。
  3. error/网络断开时进入退避重连：`backoffMs = min(30000, 1000 * 2^attempt)` + jitter，最多 6 次；重连时携带 `lastSeenEventIdRef.current`。
  4. 重连成功后 `setEventsConnected(true)` 并复位 attempt；若 `turn_completed/finished` 已收则不再重连（终态短路）。
  5. 与 WS 通道并存：WS 优先，WS error 后 fallback 到 SSE（现有 `fallbackRestore` 逻辑保留并复用 last id）。
- **客户端状态机要点**（传输维度）：
  - `none → websocket(connecting→open) → sse(connecting→open) → connected`
  - 任一通道 error：`disconnected → backoff(attempt=1..6) → reconnect`；终态事件后 `terminal`，不再重连。
  - `lastSeenEventIdRef` 单调递增，跨通道复用（WS 帧也含 `id`）。
- **验收标准 (AC)**：
  - [ ] mock 服务端在传输第 5 个事件后强制断连：前端在退避窗口内自动重连，第 6+ 事件不丢（通过 `seenEventIdsRef` 去重 + last id replay 双保险）。
  - [ ] 网络抓包确认重连请求携带 `Last-Event-ID` 头（fetch-stream 方案）或 query。
  - [ ] `turn_completed` 后即使通道断开也不再触发重连。
  - [ ] 连续 6 次重连失败 → `eventsConnected(false)` 并 toast「连接中断，请手动重试」。
- **依赖**：T-C-1（共享 AbortController + fetch-stream 基建）；可能需后端确认 `?last_event_id=` query 支持（若坚持 fetch-stream 则无后端改动）。

### T-C-4 · 媒体轮询按 task 终态终止 + 取消等后端确认 + 退避与硬上限
- **严重度** P1 · **类型** bugfix · **预估** M
- **证据** [const.ts:29](packages/slab-desktop/src/pages/image/const.ts#L29)（`MAX_POLL_ATTEMPTS=150`×2s=5min 硬上限）、[use-image-generation.ts:367](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L367)（超时即 clearGenerationTask）、[use-image-generation.ts:440](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L440)（`handleCancel` 无论成败都 `clearGenerationTask`）
- **问题**：
  - 终态集不含 `cancelled`/`interrupted`，轮询不会因用户取消而停（继续空转到 5min 上限）。
  - `handleCancel` 不 await 后端 cancel 结果就清状态，失败时任务仍在跑但 UI 已清空。
  - 固定 150×2s 上限误杀长任务（高分辨率视频可达 10min+），瞬态轮询错误（5xx）也无退避。
- **方案**：
  1. 终态门控：新增 `isTerminalTaskStatus(status)` 含 `succeeded | failed | cancelled | interrupted`（与后端 `TaskStatus` 枚举对齐，需核对 `packages/api` schema）；轮询 effect 命中终态即停止并分支处理，不再依赖 `MAX_POLL_ATTEMPTS`。
  2. `handleCancel` 改为：`await cancelTaskMutation.mutateAsync(...)` 成功 → 等下一次轮询拿到 `cancelled` 终态再 `clearGenerationTask`（或乐观置 `cancelling`，终态到达后清）；失败 → toast 保留 running 视图，不清状态。
  3. 上限上调：`MAX_POLL_ATTEMPTS` 提升到覆盖 30min（如 360×5s，或改为 `MAX_POLL_DURATION_MS = 30 * 60 * 1000` + 软警告「任务耗时较长，可在后台继续」）；同时把固定 2s 间隔改为「成功轮询保持 2s，失败轮询指数退避到 10s」。
  4. 轮询错误 effect（[:389](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L389)）不立即 `clearGenerationTask`，仅记录连续失败计数，≥3 次才 toast 并保留任务（让用户手动取消）；与 T-C-7 的 toast 去重联动。
  5. video/audio hook 同步改造（`use-video-generation.ts`、`audio-workbench.tsx`），抽取公共 `useMediaTaskPolling(taskId, opts)` hook 收敛逻辑。
- **验收标准 (AC)**：
  - [ ] 用户取消 → 后端返回 `cancelled` → 前端轮询停止并清状态；后端 cancel 失败 → UI 保留 running，toast「取消失败」。
  - [ ] 任务运行 8min 不触发超时清状态（长任务回归）。
  - [ ] 连续 2 次轮询 5xx 不清状态、不重复 toast（去重生效）。
  - [ ] `isTerminalTaskStatus` 覆盖 4 类终态，grep 无裸 `=== 'succeeded'` 漏判。
- **依赖**：T-C-7（toast 去重基础设施）；T-B-7 提供错误分类（mock 兜底可先行）。

### T-C-5 · 模型状态单源：信任后端 `status`，downloadTracking 仅用于进度条
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [use-hub-model-catalog.ts:414](packages/slab-desktop/src/pages/hub-model-catalog.ts#L414)（`toModelItem` 用 `tracking` 覆盖 `status`）、[use-hub-model-catalog.ts:212](packages/slab-desktop/src/pages/hub/hooks/use-hub-model-catalog.ts#L212)（`waitForTaskToFinish` 写 `downloadTracking`）
- **问题**：`toModelItem` 在 `hasLocalPath`/`pending` 判定中用本地 `downloadTracking` 覆盖后端 `status`，导致重载后 `downloadTracking` 失效的瞬间模型从 `downloading` 闪回 `ready`/`not_downloaded`，状态二义。
- **方案**：
  1. `toModelItem` 不再改写 `status`：`status` 一律取 `model.status`（后端真相源）。
  2. `downloadTracking` 仅作为 `download_progress` / `download_task_id` 的来源（进度条数据），不参与状态判定。
  3. `pending` 改由 `status === 'downloading' || status === 'pending'` 派生，删除 `Boolean(tracking)` 分支。
  4. 进度条 UI 在 `status === 'downloading'` 且无 `downloadTracking` 时显示 indeterminate（而非隐藏），避免重载闪烁。
  5. `waitForTaskToFinish` 失败时 invalidate `/v1/models` 让后端 `status` 收敛到 `failed`/`not_downloaded`，前端不再本地推断。
- **验收标准 (AC)**：
  - [ ] 下载中刷新页面：模型卡片状态稳定显示 `downloading`（来自后端），进度条 indeterminate 直至 tracking 恢复。
  - [ ] 下载失败：卡片状态 = 后端 `status`（`failed`/`not_downloaded`），不再闪烁 `ready`。
  - [ ] grep 确认 `toModelItem` 内无 `status = 'downloading'` 等本地覆写。
- **依赖**：无；可独立合并。需后端确认 `status` 在下载态可靠（已验证 `runtime_state`/`status` 字段存在）。

### T-C-6 · 全局 `MutationCache.onError` + `QueryClient` retry 策略 + 删除散落 `retry:false`
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [query-client.ts:3](packages/slab-desktop/src/lib/query-client.ts#L3)（`new QueryClient({})` 空配置）、[backend-status.tsx:37](packages/slab-desktop/src/components/backend-status.tsx#L37)（散落 `retry: false`）、审计 §3.1.5（22 处 `retry:false` 各自为政）
- **问题**：空 `QueryClient` 无默认 retry/staleTime，瞬态 5xx/`BackendNotReady` 零重试，400 反而可能被默认重试；无 `MutationCache.onError` 导致 ~30 处手写 toast，一致性差（GPU 错误被吞进假快照）。
- **方案**：
  1. `query-client.ts` 配置：
     ```ts
     new QueryClient({
       defaultOptions: {
         queries: {
           retry: (failureCount, error) =>
             isServerError(error) && failureCount < 2, // 仅 5xx/network 重试
           retryDelay: (attempt) => Math.min(10000, 1000 * 2 ** attempt) + Math.random() * 300,
           staleTime: 10_000,
           refetchOnWindowFocus: false,
         },
         mutations: { retry: false }, // mutation 不自动重试（由用户触发）
       },
       mutationCache: new MutationCache({
         onError: (error, _vars, _ctx, mutation) => {
           if (mutation.meta?.skipGlobalErrorToast) return;
           if (isAbortError(error) || isServerError(error) /* server 已自愈 */) return;
           toast.error(getLocalizedErrorMessage(error, t));
         },
       }),
     })
     ```
  2. `isServerError` 由 **T-B-7** 提供；过渡期提供本地 mock：`error?.status >= 500 || error?.code === 'BackendNotReady'`。
  3. 删除散落 `retry: false`（grep 22 处），保留确有理由的（如 `/health` 因有 `refetchInterval` 自带轮询，可保留或改用 `meta.skipRetry`）。
  4. 为乐观更新场景加 `meta.skipGlobalErrorToast` 逃生口（T-D 交互计划会用）。
  5. 在 `mutation.meta` 上支持 `{ skipGlobalErrorToast?: boolean; successToast?: string }` 约定。
- **验收标准 (AC)**：
  - [ ] 模拟 `/v1/models` 返回 503：自动重试 2 次后展示错误；返回 400 不重试。
  - [ ] 任意 mutation 失败（未设 skip）默认弹 toast，文案走 `getLocalizedErrorMessage`。
  - [ ] grep 全仓 `retry: false` 数量 ≤ 3（仅保留 `/health` 等有正当理由的，加注释）。
  - [ ] GPU 快照错误不再被吞（footer-status-bar 回归）。
- **依赖**：**T-B-7**（统一错误层 `isServerError` / `getLocalizedErrorMessage`）；未就绪时用 mock 解耦先行。

### T-C-7 · BackendStatus 连续失败阈值翻红 + 仅 isLoading 视为 Checking + Offline 重试 CTA + toast 去重 + ui-state 持久化失败可见
- **严重度** P1 · **类型** bugfix · **预估** M
- **证据** [backend-status.tsx:40](packages/slab-desktop/src/components/backend-status.tsx#L40)（`isChecking = isLoading || isRefetching` 每次 refetch 抖动）、[ui-state-storage.ts:34](packages/slab-desktop/src/store/ui-state-storage.ts#L34)（catch 仅 `console.warn`）、审计 §3.1.3（视频轮询每轮 toast）
- **问题**：
  - `isChecking = isLoading || isRefetching` 导致每 30s refetch 闪「Checking...」。
  - Offline 无恢复入口，用户只能干等。
  - ui-state 加载/写入失败全静默，`hasHydrated` 仍置真 → 偏好丢失无感知。
  - 轮询错误每轮重复 toast（视频）。
- **方案**：
  1. `BackendStatus`：`isChecking` 仅取 `isLoading`（首次）；维护 `consecutiveFailuresRef`，`≥3` 次连续失败才置 Offline（红）；refetch 成功归零。Offline 态 Badge 包裹按钮，点击 `refetch()` 立即探测（CTA）。
  2. toast 去重：封装 `dedupeToast(key, opts)`（基于 `code|message` 的 Sonner `id`），或直接给 `toast.error` 传 `id` 参数；轮询错误用固定 `id` 覆盖前一条。
  3. `ui-state-storage`：
     - `getItem` 失败：区分 `response.status === 404`（新装，静默返回 null）vs 网络错误（toast「无法加载偏好」并返回 null，`hasHydrated` 仍真但标记 `loadFailed`）。
     - `flushWrite` 失败：首次失败 toast「偏好保存失败」；后续失败按 key 去重，不再刷屏。
     - 暴露 `uiStateLoadFailed` 选择子供 Settings 页展示告警条。
- **验收标准 (AC)**：
  - [ ] 30s refetch 期间 Badge 不闪 Checking；连续 3 次 `/health` 失败才转红 Offline。
  - [ ] Offline 态点击 Badge 立即触发 `refetch`，恢复后转绿。
  - [ ] ui-state GET 返回网络错误时弹一次 toast（404 不弹）。
  - [ ] 视频轮询连续 5 次错误只弹 1 条 toast（去重生效）。
- **依赖**：T-C-6（toast 基建与 `getLocalizedErrorMessage`）。

### T-C-8 · 会话切换/unmount 调用 `agent.shutdown`（防服务端线程泄漏）
- **严重度** P1 · **类型** bugfix · **预估** S
- **证据** [agent.rs:192](crates/slab-app-core/src/schemas/agent.rs#L192)（`agent.shutdown` 命令已定义）、[handler.rs:322](bin/slab-server/src/api/v1/agent/handler.rs#L322)（服务端 `service.shutdown(&thread_id)` 已实现）、[use-assistant-agent.ts:492](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L492)（session effect cleanup 从不发 shutdown）
- **问题**：后端 `agent.shutdown` 完整可用（终止 thread 推理、释放 agent 资源），前端切换会话/卸载组件时仅 `socketRef.close()` + `eventSourceRef.close()`，服务端线程/订阅持续存活，长时间使用造成线程泄漏与内存增长。
- **方案**：
  1. 在 session effect 的 cleanup 中（[:494](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L494) 附近），若 `threadId` 非空且传输通道仍可用（或降级用裸 fetch），发送 `agent.shutdown`（fire-and-forget，`keepalive: true` 确保 unmount 后请求仍发出）。
  2. 判定条件：仅对「用户主动切换/卸载」触发，避免「正常 turn 结束」误关；用 `sessionRef.current !== resolvedSessionId` 区分（session 真正变更才发）。
  3. 若 T-C-1 已实现 AbortController + fetch-stream，shutdown 复用同一 fetch 客户端（带 `keepalive`）。
  4. 失败静默（`catch(() => {})`），不影响 UI；服务端幂等（重复 shutdown 无害）。
  5. 可选：增加开发环境日志/计数（`import.meta.env.DEV`）便于回归观察。
- **验收标准 (AC)**：
  - [ ] 切换会话 A→B：A 的 `threadId` 收到一次 `agent.shutdown`（抓包/日志验证）。
  - [ ] 卸载 Assistant 页：当前 threadId 收到 shutdown（`keepalive` 保证导航后仍发出）。
  - [ ] 正常 turn 完成（`turn_completed`）不触发 shutdown（回归）。
  - [ ] 服务端线程/订阅计数在多次切换后稳定不增长（手动或 E2E 验证）。
- **依赖**：T-C-1（共享 fetch/AbortController 基建，便于带 `keepalive`）；可独立先行（裸 fetch 即可）。

## 3. 执行顺序 (Sequencing)

- **M1（止损 · P0 · 1 周）**：T-C-1 + T-C-2 同 PR（同文件、状态机耦合）。**关键路径**。
- **M2（基建 · P1 · 1 周，可与 M1 部分并行）**：
  - T-C-6（QueryClient/MutationCache 基建，mock `isServerError` 先行）—— 阻塞 T-C-4、T-C-7 的 toast 体验。
  - T-C-5（模型状态单源，独立可并行）。
  - T-C-8（shutdown，独立可并行，依赖 T-C-1 的 fetch 时合并）。
- **M3（协议与体验闭环 · P1 · 1.5 周）**：
  - T-C-3（SSE resume，依赖 T-C-1 的 AbortController/fetch-stream）—— **关键路径**。
  - T-C-4（媒体轮询终态，依赖 T-C-6 toast 基建）。
  - T-C-7（BackendStatus/ui-state/toast 去重，依赖 T-C-6）。
- **可并行簇**：M2 内三卡（T-C-5/T-C-6/T-C-8）互不依赖，可三人并行。
- **关键路径**：T-C-1 → T-C-3（fetch-stream + AbortController 是 SSE resume 的基建底座）；T-C-6 → T-C-4 / T-C-7（toast/retry 基建）。
- **跨计划卡点**：T-B-7（统一错误层）是 T-C-6 的理想依赖；若 Plan B 滞后，T-C-6 用本地 mock `isServerError` 解耦先行，待 T-B-7 落地后替换。

## 4. 风险与缓解

| 风险 | 概率×影响 | 缓解 |
|---|---|---|
| 浏览器原生 `EventSource` 不支持自定义 header（阻断 Last-Event-ID 回传） | 高×高 | T-C-3 首选 `fetch` + `ReadableStream` 解析 SSE（可控 header + AbortController + 关闭语义三合一）；与服务端确认是否额外接受 `?last_event_id=` query 作为兜底 |
| `agent.shutdown` 在 unmount 后请求被取消（导航中断 fetch） | 中×中 | 使用 `fetch(url, { keepalive: true })`；fire-and-forget + `catch(()=>{})`；服务端幂等兜底重复 |
| 状态机改造引入新竞态（abort 与 turn_completed 同时到达） | 中×高 | T-C-1 引入显式状态枚举 + 终态短路（终态后忽略 abort/interrupt）；补充单测覆盖 8 种转移 |
| T-B-7 滞后阻塞 T-C-6 / T-C-4 / T-C-7 | 中×中 | 本地 mock `isServerError(error)`（`status>=5xx \|\| code==='BackendNotReady'`）+ `getLocalizedErrorMessage` 兜底，接口对齐 T-B-7 便于无缝替换 |
| 媒体轮询退避拉长导致用户感知「卡死」 | 中×低 | 退避同时展示 `progress`（Plan A）+ 连续失败计数 toast；保留手动取消入口（T-C-4） |
| `toModelItem` 重构误伤依赖 `downloadTracking.status` 的下游组件 | 低×中 | grep 全仓 `download_progress` / `pending` 消费点；保留 `downloadTracking` 字段（仅改用途），不改类型签名 |
| QueryClient 全局 retry 对 `/health` 等 `refetchInterval` 查询产生重试风暴 | 低×中 | `/health` 保留 `retry: false` + 注释；或统一用 `meta.skipRetry`；retry 上限 `n<2` + 退避 |

## 5. 验证与回归 (Verification)

- **类型/契约**：`bun run check:frontend`（OpenAPI 类型与 `AgentResponsesClientMessage` / `TaskStatus` 终态枚举对齐）。
- **单测/组件**：`bun run test:frontend`
  - 新增 assistant 状态机转移单测（abort/interrupt/shutdown/turn_failed/turn_cancelled 全路径）。
  - `useMediaTaskPolling` 终态门控 + 退避单测。
  - `toModelItem` 状态来源单测（后端 status 优先）。
  - `ui-state-storage` 404 vs 网络错误分支单测。
- **E2E**：`bun run test:browser`（流式中断/重连场景）
  1. 流式中点 Stop → 无新 token、EventSource CLOSED、已生成内容保留（T-C-1/T-C-2）。
  2. 提交后 ack 延迟 2s 内点 Stop → threadId 到达后自动 interrupt（T-C-1）。
  3. SSE 第 5 帧后断网 → 退避重连、第 6+ 帧不丢（T-C-3）。
  4. 媒体任务运行中取消 → 后端确认前不清状态、失败保留 running（T-C-4）。
  5. 会话 A→B 切换 → A 收到 `agent.shutdown`（T-C-8）。
  6. `/health` 连续 3 次失败 → Offline 红 + CTA 可点（T-C-7）。
- **Lint**：`bun run lint`（含 `retry:false` 守卫规则若已配置）。
- **手动回归**：
  - 下载模型中刷新 Hub 页（T-C-5 状态稳定）。
  - 视频生成 8min+（T-C-4 不误杀）。
  - 断网恢复后偏好保存（T-C-7 ui-state）。
