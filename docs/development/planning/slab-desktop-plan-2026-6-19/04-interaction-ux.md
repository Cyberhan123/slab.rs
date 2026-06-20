# 专项执行计划 · 交互体验闭环 (Interaction & UX Closure)

| 字段 | 值 |
|---|---|
| Plan ID | D |
| 关联根因 | R4（交互硬伤） |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) |
| 负责域 | Assistant · Media · Workspace · Settings/Setup |
| 状态 | Draft / Pending Review |
| 预估总工作量 | L |

## 1. 目标与边界 (Scope)

- **北极星**：把"后端已支持但前端不闭环"的交互路径全部接通——审批按 call 分发、停止/编辑/重发永远可点、媒体产物可重跑与流转到工作区、代码可与 AI 对话、IDE 体验与设置不再有"dead state"，任何用户操作都有确定性反馈。
- **In scope**：
  - Assistant：审批 `Map<callId>`、Stop `pendingAbort` 竞态、Edit-and-resend / Regenerate、代码块复制 + 消息级 copy 剥离 `<think>` 残留。
  - Media：image/video/audio 历史参数重跑；视频"在工作区打开" + A-B 对比。
  - Workspace：选中代码 → AI 解释桥接；终端多标签/分屏/shell 选择；文件监听（新端点 `/v1/workspace/watch`，后端联动）+ HTTP 大文件守卫；Monaco 懒启动 + 浏览器模式 Monaco；`editorSettings` 接入 VS Code configuration service。
  - Setup/Settings：Setup 确认门 + 组件级 checklist（ffmpeg/backends）；设置关键字搜索 + autosave 未保存离开守卫。
- **Out of scope（移交他计划）**：
  - Assistant 流式通道的 `AbortController` 关闭与 `turn_failed` 内容保留 → **Plan C（T-C-1 / T-C-2）**。本计划 T-D-2 仅负责 `pendingAbort` 竞态的交互层补发，需与 T-C-1 合并实现。
  - 媒体 `task.progress` 字段消费与进度条 UI（R1 能力释放）→ **Plan B（T-B-5）**。本计划 T-D-5 不含进度条。
  - 统一错误包络 `{code,message,data,i18n}` + `getLocalizedErrorMessage` → **Plan B（T-B-7）**。本计划仅消费其产出。
  - 设计系统 Token / 玻璃质感 / 软分割线 / reduced-motion → **Plan E（设计系统）**。本计划复用其 `.focus-ring` / `<StateSurface>`（若有），否则先用临时本地实现并标注 TODO。
  - Workspace HTTP↔Tauri 状态源割裂（recent/config 恒空）→ **Plan A（安全与正确性，状态源收敛）**。
  - 未保存守卫统一到 Monaco working-copy service（`window.confirm` → AntD Modal）的 P0 部分 → **Plan A（T-A-2，安全/正确性）**。本计划 T-D-12 仅覆盖 Settings 的 autosave 未保存守卫（独立 dirty set）。
- **Definition of Done**：
  - [ ] T-D-1..T-D-12 全部 AC 通过，`bun run check:frontend && bun run test:frontend && bun run test:components && bun run lint` 全绿。
  - [ ] 涉及 schema/端点变更的卡片（T-D-8）完成 `bun run gen:api` 并提交生成产物。
  - [ ] E2E（`bun run test:browser`）：审批并发 / Stop 提交窗口 / Edit-resend / 视频转工作区 / AI 解释代码 5 个新场景通过。
  - [ ] 前端不再存在「单例审批覆盖」「Stop 静默返回」「`<think>` 残留 copy」「textarea 充当浏览器编辑器」反模式（grep 守卫，见各卡）。
  - [ ] 无新增原生 `window.confirm` / `window.alert`（仅保留 Plan A 已认领的 workspace 守卫，待其迁移后一并删除）。

## 2. 任务卡 (Task Cards)

### Track 1 — Chat & Media 交互

---

#### T-D-1 · 审批改为 `Map<callId, PendingApproval>` + 按 thought 渲染 per-call approve/reject
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [use-assistant-agent.ts:72](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L72)（`const [pendingApproval, setPendingApproval] = useState<PendingApproval | null>(null)` 单例）、[use-assistant-agent.ts:676](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L676)（`submitApproval(approved)` 无 `callId` 参数）、[index.tsx:636](packages/slab-desktop/src/pages/assistant/index.tsx#L636)（`approving: isRequesting` 全局布尔，bubble 内无法区分是哪个 call 在等审批）。
- **问题**：`tool_concurrency > 1` 时，第二个 `approval_required` 事件覆盖第一个 `pendingApproval`，用户只能批/拒一个，另一个被静默丢弃或错配。bubble 渲染用全局 `isRequesting` 表达"approving"，与具体 thought 无关。
- **方案**：
  1. 状态结构：`useState<Map<string, PendingApproval>>`（key = `callId`）；导出 `pendingApprovals`（数组）与 `submitApproval(callId, approved)`。
  2. 收到 `approval_required` 事件时 `setPendingApprovals(prev => new Map(prev).set(callId, payload))`；`approval_resolved` / thought 进入 `loading`/`abort` 时 `delete(callId)`。
  3. `submitApproval(callId, approved)` 改签名，内部从 Map 取该 call 的 payload 后 `delete`，再 `updateThoughtStatus(callId, approved ? 'loading' : 'abort')`，发 `agent.approval.resolve`（`call_id` 字段已就绪）。
  4. 渲染层：[index.tsx:636](packages/slab-desktop/src/pages/assistant/index.tsx#L636) 把 `approving: isRequesting` 改为 `approvingCallIds: Array<string>`（来自 `pendingApprovals.map(p => p.callId)`）；`AssistantBubbleContent` 内对每个 thought 判断 `approvingCallIds.includes(thought.callId)` 决定是否渲染该 thought 的 approve/reject 按钮（按钮 `onClick={() => onApprove(thought.callId, true/false)}`）。
  5. 顶部"等待审批"全局提示条（可选）：当 Map.size > 0 时 header 显示"等待 N 个审批"。
- **验收标准 (AC)**：
  - [ ] 单测：构造两个并发 `approval_required`（不同 `callId`），两个 thought 都渲染 approve/reject 按钮，互不覆盖。
  - [ ] 单测：approve call A 后，call A 按钮消失、状态转 `loading`，call B 仍可审批。
  - [ ] 组件测试（`test:components`）：bubble 内每个 thought 独立显示审批控件，`data-testid={`thought-approve-${callId}`}`。
  - [ ] grep 守卫：源码无 `useState<PendingApproval | null>`（单例形态）。
- **依赖**：无；与 T-D-2 同文件，建议同 PR。

---

#### T-D-2 · Stop 按钮 `pendingAbort` 竞态修复（thread_id 到达后补发 interrupt）
- **严重度** P1 · **类型** bugfix · **预估** S
- **证据** [use-assistant-agent.ts:709](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L709)（`if (!threadId || !isRequesting) { return }` —— 提交后 ack 返回 `thread_id` 的窗口内 Stop 直接吞掉）。
- **问题**：用户提交→`agent.response.create` 已发出→服务端尚未回 `thread_id`→用户点 Stop→`abort()` 因 `threadId === null` 直接 return，既不发 interrupt 也不关通道，服务端继续空烧 token，等 ack 回来后已无法取消（用户已离开）。
- **方案**：
  1. 新增 `pendingAbortRef = useRef(false)`；`abort()` 改为：
     - 若 `threadId` 已就绪且 `isRequesting` → 发 `agent.interrupt`（原路径）。
     - 若 `threadId` 为 null 但 `responsesMutation.isPending`（已提交、等 ack）→ 置 `pendingAbortRef.current = true`，并立即走 T-C-1 的本地 abort（`abortControllerRef.current?.abort()` + 关 EventSource/WS）。
  2. 在拿到 `thread_id` 的赋值处（`setThreadId(...)` 后的 effect 或 `postAgentCommand` 成功回调）检查 `pendingAbortRef.current`：若为真，立即补发 `agent.interrupt` 并清 flag、置 `status='interrupting'`。
  3. UI 反馈：Stop 按钮在"提交等 ack"态也置为可点 + loading 文案"正在停止…"，不再依赖 `threadId`。
- **验收标准 (AC)**：
  - [ ] E2E（mock 服务端延迟 ack 2s）：提交后立即点 Stop，本地停止收流；ack 到达后自动发 interrupt，服务端日志可见 `agent.interrupt`。
  - [ ] 单测：`threadId=null` + `isPending=true` 时调 `abort()`，`pendingAbortRef.current` 为 true 且本地 abort 被调用（mock `abortControllerRef`）。
  - [ ] 单测：随后 `setThreadId('t1')`，触发补发 interrupt 调用一次。
- **依赖**：**Plan C / T-C-1**（AbortController 基础设施）。必须先有 `abortControllerRef`；本卡在其上加 `pendingAbort` 交互补发逻辑。建议与 T-C-1 同 PR 或紧随其后。

---

#### T-D-3 · Edit-and-resend / Regenerate（截断历史重提）
- **严重度** P1 · **类型** feat · **预估** M
- **证据** assistant bubble 渲染区（[index.tsx:622-645](packages/slab-desktop/src/pages/assistant/index.tsx#L622) `bubbleItems`，当前 user/assistant bubble 无 edit/regenerate 入口，grep `regenerate|editAndResend` 在 assistant 组件目录零命中）。
- **问题**：用户无法修正一条已发的 prompt 重提，也无法让 assistant 重答最后一条；只能新开对话，历史无法复用。
- **方案**：
  1. **Regenerate**（最后一条 assistant bubble）：新增 `regenerateLast()`，删除最后一条 assistant message，以"上一条 user message + 既有 history"重发 `agent.input`（若有 threadId）或重建 thread（`agent.response.create` 用截断后的 messages）。
  2. **Edit-and-resend**（任意 user bubble）：user bubble 悬浮显示"编辑"按钮 → 点击进入行内编辑（textarea 替换气泡内容）→ 保存时截断该 user message 之后的所有消息（assistant + tool），以编辑后的 prompt 重发。
     - 截断实现：`setMessages(current => current.slice(0, targetUserIndex + 1))` 后改写 target user content，再 `handleSubmit(editedPrompt)`。
     - 因 thread 已被后续 turn 污染，edit-resend 必须开新 thread（重发 `agent.response.create`），并提示"已基于编辑内容开启新对话分支"。
  3. 入口：`bubbleItems` 的 user item 增加 `onEdit`/`onResend` 回调（仅非 streaming、非 ephemeral 会话）；最后一条 assistant item 增加 `onRegenerate`。
  4. 历史持久化：编辑/重发产生的截断通过既有 `PUT /v1/messages` / 新增 message 路径落库（会话切换后可恢复）。
- **验收标准 (AC)**：
  - [ ] 组件测试：最后一条 assistant bubble 有 `data-testid="assistant-regenerate"` 按钮，点击后该 assistant message 被移除并触发重发。
  - [ ] 组件测试：user bubble 编辑后保存，该 index 之后的消息全部消失，新 prompt 被提交。
  - [ ] 单测：`regenerateLast()` 在 `threadId=null` 时走 `agent.response.create`，在 `threadId` 就绪时走 `agent.input`。
  - [ ] 截断后的 messages 数组长度符合预期（无悬挂 tool_call）。
- **依赖**：T-D-1（共享 messages 截断工具）；建议在 T-D-2 之后（避免与 abort 竞态冲突）。

---

#### T-D-4 · 代码块复制按钮 + 消息级 copy 剥离 `<think>` 残留
- **严重度** P2 · **类型** feat · **预估** S
- **证据** [assistant-markdown.tsx:46](packages/slab-desktop/src/pages/assistant/components/assistant-markdown.tsx#L50)（`CodeBlockComponent` 仅渲染 `CodeHighlighter`，无复制按钮）、[assistant-markdown.tsx:69](packages/slab-desktop/src/pages/assistant/components/assistant-markdown.tsx#L69)（`ThinkComponent` 返回 `null`，`<think>` 标签内容被丢弃，但消息级 copy 若直接取 `message.content` 原始字符串可能含 `<think>...</think>` 残留）。
- **问题**：代码块无法一键复制；消息级复制可能把模型泄露的推理残骸（`<think>`）一起复制走，污染下游使用。
- **方案**：
  1. **代码块复制**：`CodeBlockComponent` 在 `CodeHighlighter` 的 `header` slot 旁加复制图标按钮，`onClick` 调 `navigator.clipboard.writeText(code)`（`code` 已由 `childrenToText` 提取），Sonner toast"已复制"。
  2. **消息级 copy 净化**：新增 `stripThinkTags(text: string)`（正则 `/<think>[\s\S]*?<\/think>/g` → ``，并 trim 前后空白）；所有消息级 copy 入口（[index.tsx:627](packages/slab-desktop/src/pages/assistant/index.tsx#L627) `labels.copy` 对应的 handler）调用 `stripThinkTags(getAssistantMessageTextContent(message))` 后再写剪贴板。
  3. 双保险：渲染层 `ThinkComponent` 返回 null 不变（视觉已隐藏），copy 层独立净化（防御 `content` 未被渲染管线清理的情况）。
- **验收标准 (AC)**：
  - [ ] 组件测试：代码块渲染 `data-testid="code-copy"` 按钮，点击后剪贴板内容 === 代码块原始文本。
  - [ ] 单测：`stripThinkTags('a<think>secret</think>\nb')` === `'a\nb'`；`stripThinkTags('<think>only</think>')` === `''`。
  - [ ] 组件测试：消息级 copy 含 `<think>` 的 content 时，剪贴板不含 `<think>` 及其内部文本。
- **依赖**：无。

---

#### T-D-5 · 媒体"历史参数重跑" + 视频"在工作区打开"/A-B 对比
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [use-image-generation.ts:53](packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts#L53)（`history` state 存在但无 rerun/reuse 函数，grep `rerun|reuse|lastParams` 仅命中本卡相关）、[video-workbench.tsx:604](packages/slab-desktop/src/pages/video/components/video-workbench.tsx#L604)（历史卡片 `onClick={openHistoryDetail}` 仅打开详情，无"重跑"/"在工作区打开"/"对比"动作）。
- **问题**：用户跑出一组满意参数后无法一键复用；视频产物无法流转到 Workspace 做进一步处理；无 A-B 对比导致调参只能凭记忆。
- **方案**：
  1. **历史参数重跑**（image/video/audio 通用）：
     - 在历史详情/卡片上加"重跑"按钮；点击调 `refillFromHistory(task)`：把 task 的 `params`（prompt/negative/size/steps/guidance/seed 等）回填到表单 state，不自动提交（让用户确认/微调）。
     - `seed` 默认回填但标注"锁定原 seed 复现 / 随机化"切换。
     - hook 新增 `loadParamsFromHistory(taskId)`，由 image/video/audio 各自实现回填映射。
  2. **视频"在工作区打开"**：
     - 视频产物（`artifact_url` 本地路径）加"在工作区打开"按钮 → 调 Tauri `open_in_workspace` 或路由跳转 `/workspace?reveal=<artifactPath>`，工作区打开后 `revealActiveFileInExplorer`（既有）定位文件。
     - 浏览器模式：下载产物到本地或打开新 tab（无工作区时 fallback）。
  3. **A-B 对比**：
     - 历史详情支持"加入对比"（最多 2 项）；选中 2 项后显示对比视图（左右并排 + 各自参数表 + 差异高亮：变化的 param 标黄）。
     - 对比视图复用 `<StateSurface>`（Plan E）或临时 grid；参数 diff 用 `diff(paramsA, paramsB)` 计算。
- **验收标准 (AC)**：
  - [ ] 组件测试：image 历史卡片"重跑"按钮点击后，表单各字段 === task.params 对应值。
  - [ ] 组件测试：视频历史详情"在工作区打开"在桌面模式触发 Tauri 命令（mock 验证调用），浏览器模式触发下载。
  - [ ] 组件测试：选中 2 个历史项后对比视图渲染，变化的参数被高亮（`data-testid="param-diff"`）。
  - [ ] 单测：`diffParams({a:1,b:2},{a:1,b:3})` 返回 `[{key:'b',from:2,to:3}]`。
- **依赖**：无（与 T-B-5 进度条正交）；视频"在工作区打开"依赖 workspace reveal 路径已存在。

---

### Track 2 — Workspace & Settings 交互

---

#### T-D-6 · "选中代码 → AI 解释"（命令面板 + 编辑器右键 → 投递 `{relativePath, selection}` 到 assistant）
- **严重度** P1 · **类型** feat · **预估** M
- **证据** workspace workbench 当前无 assistant 桥接（grep `regenerate|editAndResend` 与 assistant 桥接零命中；[workspace-lsp.ts:210-230](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L210) 已有 `registerEditorOpenHandler` 拿到 `relativePath` + `selection`，但仅用于打开文件，未投递到 assistant）；[workspace-command-palette.tsx:42](packages/slab-desktop/src/pages/workspace/components/workspace-command-palette.tsx#L42) 命令面板已存在可扩展。
- **问题**：用户在 IDE 里看到一段代码想问 AI，必须手动复制→切到 assistant→粘贴→补充"这是 xxx 文件的选中部分"，无原生桥接。
- **方案**：
  1. **取选区**：Monaco `editor.getSelection()` → `{startLineNumber, startColumn, endLineNumber, endColumn}`；`relativePath` 由 `workspaceLspRelativePathFromUri`（既有）从 model uri 推导。
  2. **命令面板入口**：`WorkspaceCommandPalette` 加命令"用 AI 解释选中代码"（快捷键 `Ctrl+Alt+I` / `Cmd+Alt+I`），仅当选区非空时启用。
  3. **编辑器右键**：注册 Monaco context menu item（`editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | KeyCode.KeyI, ...)` 或 `actions.addAction`）。
  4. **投递**：构造 prompt 模板：
     ```
     请解释以下代码（来自 {relativePath}:{startLine}-{endLine}）：
     ```{lang}
     {selectedText}
     ```
     ```
     通过全局事件/store 投递到 assistant 页（如 `useAssistantDropStore` 或 router state `/assistant?prefill=...`），assistant 接收后填入输入框并可选自动提交。
  5. **跨页投递机制**：新增 `assistantDraftStore`（Zustand，`{ prefillPrompt, autoSubmit }`），assistant 页 mount 时消费并清空；workspace 不直接依赖 assistant 内部 hook。
- **验收标准 (AC)**：
  - [ ] 组件测试：在 Monaco 选中代码 → `Ctrl+Alt+I` → 跳转 assistant 页且输入框含选中文本 + 文件路径标注。
  - [ ] 组件测试：命令面板搜索"AI 解释"出现该命令，选区为空时禁用（`aria-disabled`）。
  - [ ] 单测：prompt 模板包含 `relativePath`、行号区间、语言标识（从文件扩展名映射）。
  - [ ] E2E（`test:browser`）：浏览器模式 textarea 选中后命令不可用（仅 Monaco 模式启用）。
- **依赖**：T-D-9（Monaco 懒启动，否则浏览器模式无 Monaco 选区）；可与 T-D-7 并行。

---

#### T-D-7 · 终端多标签/分屏/shell 选择
- **严重度** P2 · **类型** feat · **预估** L
- **证据** [workspace-workbench.tsx:444](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx#L444)（`WorkspaceConsolePanel` 单实例，`consoleOpen` 布尔无多标签）、[terminal.rs:55](bin/slab-app/src-tauri/src/terminal.rs#L55)（`create_session` 每次创建新 session 但前端只持有一个 url；`TerminalClientMessage` 仅 `Input`/`Resize`，无 shell 选择参数）。
- **问题**：单终端窗口无法并行跑多个命令；shell 写死（后端默认），用户无法选 powershell/bash/cmd/zsh；无分屏。
- **方案**：
  1. **前端多标签**：`WorkspaceConsolePanel` 改为标签栏 + xterm 容器；`tabs: TerminalTab[]`（`{id, url, title, shell}`），新增/关闭/切换。
  2. **分屏**（次优先，可分阶段）：支持左右/上下双 pane，每 pane 绑定一个 tab。
  3. **shell 选择**：
     - 后端 [terminal.rs](bin/slab-app/src-tauri/src/terminal.rs) 扩展 `TerminalSessionRequest` 增加 `shell?: Option<String>`（枚举 `powershell|bash|cmd|zsh`，默认沿用当前）；`create_session` 透传给 pty 构建。
     - 前端新建 tab 时弹 shell 选择器（系统可用 shell 列表，Windows 默认 powershell，macOS/Linux 默认用户 login shell）。
  4. **持久化**：tab 列表（不含会话内容，仅 title+shell）写入 `ui-state`，工作区重开后恢复标签结构（会话需重建）。
- **验收标准 (AC)**：
  - [ ] 组件测试：新建终端标签 → xterm 实例独立 → 两个 tab 输入互不串扰。
  - [ ] 组件测试：关闭 tab 后对应 session 被回收（后端 `remove_session` 调用，mock 验证）。
  - [ ] 集成：shell 选择器列出系统可用 shell，选中后新 tab 以该 shell 启动（后端单测验证 `shell` 参数透传到 pty 命令）。
  - [ ] grep 守卫：`WorkspaceConsolePanel` 不再是单实例布尔。
- **依赖**：后端 terminal.rs 扩展（`shell` 字段）→ schema 变更 → `bun run gen:api`；与 T-D-6 并行。

---

#### T-D-8 · 文件监听（`/v1/workspace/watch` SSE/WS 或 focus-regain 失效）+ HTTP 大文件守卫（MAX_FILE_BYTES）
- **严重度** P1 · **类型** feat · **预估** L
- **证据** [workspace-lsp.ts:885](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L885)（`watch() { return noopDisposable }` —— 文件系统监听是 no-op，外部改动 Explorer/Search 不刷新）、[mod.rs:37,161](crates/slab-app-core/src/domain/services/workspace/mod.rs#L37)（`MAX_FILE_BYTES = 1MB`，超限返回 `AppCoreError::BadRequest("file is too large to preview...")`，前端无前置 size 守卫与优雅占位）。
- **问题**：用户用外部编辑器改了文件，Slab 的 Explorer/Search/编辑器视图全部失效；打开 >1MB 文件直接报错，无"文件过大"友好态。
- **方案**（双轨，因后端监听端点缺失 🚫）：
  1. **后端新端点 `/v1/workspace/watch`**（SSE 或 WS）：
     - 后端用 `notify`（crate）监听 workspace root，事件 `{relativePath, type: created|changed|deleted}`。
     - 前端 `workspaceBridge` 新增 `watchWorkspace()` 订阅，事件触发 `clearWorkspaceFileSystemCache()` + `changesEmitter.fire([...])`（既有 emitter），Monaco/Explorer 自动刷新。
     - schema 变更 → `bun run gen:api`。
  2. **Fallback（无监听端点时）**：
     - window `focus` 事件 + 每 30s 心跳：调 `GET /v1/workspace/path/stat` 比对 mtime，变化则 invalidate Explorer query。
     - 浏览器模式（无 Tauri）默认走 fallback。
  3. **大文件守卫**：
     - 前端打开文件前先 `stat`（`/v1/workspace/path/stat`，已有），若 `size_bytes > 1MB` 不发 `read_file`，直接渲染占位态"文件过大（{size}，上限 1MB），在工作区用外部编辑器打开"（含 Tauri `open_path` 按钮）。
     - 错误兜底：若已发 `read_file` 收到"too large"错误（T-B-7 错误包络后），同样渲染占位态（双保险）。
  4. `watch()` 不再返回 `noopDisposable`，返回真实 disposable 在 unmount/watch 失效时关闭订阅。
- **验收标准 (AC)**：
  - [ ] 集成（桌面）：外部修改文件 → Explorer 自动刷新（无需手动点刷新），grep 验证 `watch()` 非 noop。
  - [ ] 集成（浏览器 fallback）：window focus 后 30s 内 Explorer 刷新（mock stat mtime 变化）。
  - [ ] 组件测试：打开 2MB 文件时渲染 `data-testid="file-too-large"` 占位，不发起 `read_file`（网络 mock 验证未调用）。
  - [ ] 单测（后端）：`/v1/workspace/watch` 事件结构符合 schema；`notify` 接收到 created/changed/deleted。
  - [ ] `gen:api` 产物已提交，前端类型含 `WorkspaceWatchEvent`。
- **依赖**：后端新端点（notify crate）→ schema → `gen:api`；与 T-B-7（错误包络）协同识别"too large"。**关键路径**（影响 T-D-9 之后的编辑器体验）。

---

#### T-D-9 · Monaco 懒启动（延迟 `ensureWorkspaceLspServices` 到编辑器/Explorer 渲染）+ 浏览器模式用 Monaco 替换 textarea
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [workspace-lsp.ts:194](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L194)（`ensureWorkspaceLspServices` 加载 8+ 扩展 `setiThemeExtensionReady/emmet/...`，首次进入 workspace 即全量加载，阻塞渲染）、[workspace-workbench.tsx:656](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx#L656)（浏览器模式 `WorkspaceBrowserEditor` 用 `<textarea>`，与桌面 Monaco 体验割裂，FS overlay 已支持 HTTP）。
- **问题**：进入 workspace 即触发重型 Monaco 初始化（数十个 service-override + 扩展），首屏白屏；浏览器用户被迫用简陋 textarea，丧失高亮/LSP。
- **方案**：
  1. **懒启动**：把 `ensureWorkspaceLspServices()` 的调用从 workspace 页 mount 推迟到"编辑器面板首次渲染"或"Explorer 首次展开文件树"。
     - 用 `React.lazy` + `Suspense` 包裹 Monaco workbench 容器；`ensureWorkspaceLspServices` 移入 Monaco 容器的 `useEffect`（首次 mount 触发）。
     - 首屏（无文件打开时）只渲染 Explorer 骨架 + 空态，不加载 Monaco。
  2. **按需扩展**：`Promise.allSettled([...])` 的 8 个扩展拆为"核心（seti/emmet）即时"+"语言相关（cpp/go/sql/xml/docker/dotenv）按文件类型动态加载"。
  3. **浏览器模式 Monaco**：`WorkspaceBrowserEditor` 替换 textarea 为 Monaco（FS overlay [workspace-lsp.ts:185](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L185) 已注册 HTTP 文件系统）。
     - 保留 textarea 作为 Monaco 加载失败/极弱环境 fallback（`<noscript>` 式降级）。
  4. **加载态**：Monaco 首次加载时显示 skeleton（`<StateSurface variant="loading">`，Plan E）。
- **验收标准 (AC)**：
  - [ ] 性能：workspace 首屏（无文件打开）不发起 Monaco 相关 import（bundle analyzer 或 network 面板验证）；首屏 LCP 改善可量化（目标 -30%）。
  - [ ] 组件测试：首次打开文件触发 `ensureWorkspaceLspServices`（mock 验证调用时机）；后续打开不再重复调用。
  - [ ] 组件测试：浏览器模式渲染 Monaco（`data-testid="workspace-editor-monaco"`），非 textarea。
  - [ ] grep 守卫：`WorkspaceBrowserEditor` 不再含 `<textarea>` 作为主编辑器。
- **依赖**：T-D-8（文件监听，否则 Monaco 视图仍失效）；T-D-6（AI 解释依赖 Monaco 选区）。

---

#### T-D-10 · `editorSettings` 接入 VS Code configuration service（消除 dead state）
- **严重度** P2 · **类型** refactor · **预估** M
- **证据** [use-workspace-page.ts:789](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L789)（`handleUpdateEditorSettings` 被导出但未接入 Monaco/VS Code configuration service，状态仅存 React state，编辑器行为不变 —— dead state）。
- **问题**：用户改"字号/tab 大小/word wrap"等设置，UI 显示已保存但编辑器毫无反应，信任崩塌。
- **方案**：
  1. 梳理 `editorSettings` 字段（fontSize, tabSize, wordWrap, minimap, lineNumbers 等），映射到 VS Code configuration keys（`editor.fontSize` / `editor.tabSize` / `editor.wordWrap` ...）。
  2. 在 Monaco 初始化后（T-D-9 的 `ensureWorkspaceLspServices` 完成）调 `vscode.ConfigurationContext` / `updateConfigurationValues` 写入用户级配置；或在 `standaloneEditor` 实例上 `updateOptions(...)`。
  3. `handleUpdateEditorSettings` 落库后同步触发 configuration service 更新；从 `ui-state`/settings 恢复时同样写入。
  4. 单测：改 `fontSize=18` 后 Monaco editor options 的 `fontSize` === 18。
- **验收标准 (AC)**：
  - [ ] 组件测试：调用 `handleUpdateEditorSettings({ fontSize: 18 })` 后，Monaco editor 实例 `getOption(editor.Option.fontSize)` === 18。
  - [ ] 组件测试：tabSize / wordWrap / minimap 至少 4 项配置生效。
  - [ ] grep 守卫：`handleUpdateEditorSettings` 不再是仅 setState 不外发的 dead handler。
- **依赖**：T-D-9（Monaco 容器就绪后才能注入配置）。

---

#### T-D-11 · Setup 确认门（非自动 provision）+ 组件级 checklist（ffmpeg/backends）
- **严重度** P1 · **类型** feat · **预估** M
- **证据** [use-setup.ts:176-193](packages/slab-desktop/src/pages/setup/hooks/use-setup.ts#L176)（`useEffect` 检测 `!status.initialized` 即 `autoStartedRef.current = true; void startProvision()` —— 自动触发无确认门）、审计 §3.3.6（组件级 ffmpeg/backends 状态不可见）。
- **问题**：首次启动自动 provision，用户无机会选择"稍后"/查看将安装什么；某组件（ffmpeg/backends）失败时仅整体报错，无法定位/单独重试。
- **方案**：
  1. **确认门**：删除自动 `startProvision`；改为渲染"准备就绪"页面：展示将执行的动作（下载模型、安装 ffmpeg、配置 backends）+ "开始"/"稍后"按钮。
     - 仅当用户显式点击"开始"，或 `runtime_payload_installed === true`（已预装）时才 provision。
     - "稍后"进入主界面但置灰依赖功能（媒体生成提示"需先完成 Setup"）。
  2. **组件级 checklist**：
     - 渲染 `setupStatus.components[]`（ffmpeg/candle/ggml/各 backend）：每项显示 `name` + `status`（pending/installing/installed/failed）+ 失败时"重试"按钮（调 provision 传 `component` 过滤，若后端支持；否则整体重试但保留已成功项）。
     - provision 轮询（既有 `pollProvisionTask`）解析 task 的组件级进度，逐项更新 checklist。
  3. **`complete` 端点**：provision 成功后调 `POST /v1/setup/complete`（审计 §2.1 标注"未用"），标记 `initialized=true`。
- **验收标准 (AC)**：
  - [ ] 组件测试：首次加载 setup（`status.initialized=false`）不自动 provision，渲染"开始"按钮。
  - [ ] 组件测试：点击"开始"才触发 `startProvision`；"稍后"跳过并进入主界面。
  - [ ] 组件测试：checklist 渲染各组件状态，失败组件有 `data-testid="component-retry-{name}"` 按钮。
  - [ ] 集成：provision 成功后 `/setup/complete` 被调用（mock 验证），`status.initialized` 变 true。
- **依赖**：后端 `setupStatus.components` 字段（若不存在需补，schema 变更 → `gen:api`）；无前端阻塞。

---

#### T-D-12 · 设置关键字搜索 + autosave 未保存离开守卫
- **严重度** P2 · **类型** feat · **预估** M
- **证据** settings page（审计 §3.3.6：无搜索；autosave 模式无未保存守卫，`GET/PUT /v1/settings/{pmid}` 已就绪）。
- **问题**：设置项众多无搜索，用户难定位；autosave 失败或网络抖动时用户切页 unknowingly 丢失未保存编辑。
- **方案**：
  1. **关键字搜索**：
     - 顶部搜索框，输入即过滤 property（匹配 `title` / `description` / `category` / `pmid`）；命中高亮；无结果时 `<StateSurface variant="empty">`。
     - 支持按 category 折叠/展开（搜索时自动展开命中项所在 category）。
  2. **autosave 未保存守卫**（与 Plan A 的 workspace 守卫解耦，独立 dirty set）：
     - 维护 `dirtyPmids: Set<string>`：编辑入 dirty、`PUT` 成功出 dirty。
     - 路由离开（`useBlocker` from react-router-dom v6.4+ 或 `beforeunload`）：若 `dirtyPmids.size > 0` 弹 AntD Modal"有 N 项未保存，确定离开？"。
     - autosave 成功的编辑不入 dirty（即"已保存"），守卫只在 autosave 失败/进行中触发。
  3. **autosave 失败可见**：依赖 Plan B 的全局 `MutationCache.onError` toast；额外在 dirty 项旁显示"保存失败"红点 + 重试按钮。
- **验收标准 (AC)**：
  - [ ] 组件测试：搜索"ffmpeg"仅显示匹配 property，其余隐藏；高亮命中关键词。
  - [ ] 组件测试：编辑一个 setting（autosave mock 失败）→ 尝试路由离开 → 弹 Modal 拦截。
  - [ ] 组件测试：autosave 成功后离开不弹 Modal。
  - [ ] grep 守卫：settings 页无原生 `window.confirm`（用 AntD Modal）。
- **依赖**：Plan B 全局 onError（autosave 失败 toast）；可独立先行（本地 onError 兜底）。

---

## 3. 执行顺序 (Sequencing)

- **M1（交互止损）**：T-D-1（审批 Map）+ T-D-2（Stop pendingAbort）+ T-D-4（代码块复制/think 净化）。
  - T-D-1 与 T-D-2 同文件（`use-assistant-agent.ts`），合并一个 PR；T-D-2 紧随 Plan C / T-C-1（AbortController 基础设施）。
- **M2（媒体闭环 + 工作区 IDE 基座）**：T-D-5（媒体重跑/工作区/对比）+ T-D-9（Monaco 懒启动）+ T-D-6（AI 解释）。
  - T-D-9 → T-D-6（AI 解释依赖 Monaco 选区）；T-D-5 与工作区正交，可并行。
- **M3（工作区文件体验 + Setup 门）**：T-D-8（文件监听 + 大文件守卫，**关键路径**）+ T-D-11（Setup 确认门）。
  - T-D-8 后端端点为关键路径，需尽早开工（后端 + `gen:api`）。
- **M4（终端 + 设置 + 编辑器配置）**：T-D-7（终端多标签）+ T-D-10（editorSettings）+ T-D-12（设置搜索/守卫）+ T-D-3（Edit-and-resend）。
  - T-D-10 依赖 T-D-9；T-D-3 建议 M4（避免与 M1 的 abort/messages 改动冲突）。

- **可并行**：
  - T-D-5（Media）与 Track 2 全部并行（不同域）。
  - T-D-4（Assistant markdown）与 T-D-7（Terminal）并行。
  - T-D-11（Setup）与 T-D-12（Settings）并行。
- **关键路径**：`Plan C/T-C-1` → **T-D-2** →（T-D-1 同 PR）→ **T-D-9** → **T-D-6 / T-D-10**；以及独立的关键路径：后端 `/v1/workspace/watch` → **T-D-8** → T-D-9 编辑器视图可靠刷新。

## 4. 风险与缓解

| 风险 | 概率×影响 | 缓解 |
|---|---|---|
| T-D-2 与 Plan C / T-C-1 同改 `use-assistant-agent.ts`，合并冲突 | 高×中 | 与 Plan C owner 对齐，T-D-2 作为 T-C-1 的 follow-up commit 同 PR；约定 `pendingAbortRef` 命名与放置位置。 |
| T-D-8 后端 `/v1/workspace/watch` 跨平台 `notify` crate 行为差异（Windows ReadDirectoryChanges vs inotify/kqueue） | 中×高 | 后端先写跨平台单测；前端 fallback（focus+stat 心跳）作为一级降级路径，监听失败不阻断 workspace。 |
| T-D-9 Monaco 懒启动改变初始化时机，破坏既有 LSP/extension 依赖（如 T-D-10 配置注入时机） | 中×中 | 懒启动后所有依赖 Monaco 就绪的逻辑（editorSettings、AI 解释注册）改用 `ensureWorkspaceLspServices().then(...)` 事件驱动，而非 mount 时直接调；加单测覆盖"先 ensure 再配置"顺序。 |
| T-D-1 审批 Map 改动 `submitApproval` 签名，波及 bubble 渲染与既有调用方 | 中×中 | 一次性迁移所有 `submitApproval(approved)` → `submitApproval(callId, approved)`；TS 类型强制（函数签名变更）让编译器捕获遗漏。 |
| T-D-3 Edit-and-resend 截断历史可能与后端 thread 状态不一致（thread 已含被截断的 turn） | 高×中 | Edit-resend 强制开新 thread（`agent.response.create`），不试图"回滚"服务端 thread；UI 提示"新分支"。Regenerate（同 thread 重发最后一条）仅在 threadId 就绪时走 `agent.input`。 |
| T-D-7 shell 选择后端扩展引入安全面（任意 shell 执行） | 低×高 | `shell` 枚举白名单（`powershell|bash|cmd|zsh`），后端拒绝任意字符串；与 Plan A 安全 owner review。 |
| T-D-11 `setupStatus.components` 后端字段缺失 | 中×低 | 前端先做 optional 字段渲染（`components ?? []`），后端补字段后无需前端再改；schema 变更走 `gen:api`。 |
| 全卡共用 Plan E 的 `<StateSurface>`/`.focus-ring` 尚未交付 | 高×低 | 各卡先用本地临时实现 + TODO 注释引用 Plan E，待 Plan E 交付后统一替换；不阻塞功能 AC。 |

## 5. 验证与回归 (Verification)

- **类型/契约**：`bun run check:frontend`；schema 变更（T-D-7 shell、T-D-8 watch event、T-D-11 components）→ `bun run gen:api` 并提交生成产物。
- **单测/组件**：`bun run test:frontend` / `bun run test:components`；每卡至少 1 个单测 + 1 个组件测试（见各 AC）。
- **E2E**：`bun run test:browser`；新增场景：
  1. 审批并发（T-D-1）。
  2. Stop 提交窗口（T-D-2，mock 延迟 ack）。
  3. Edit-and-resend 截断历史（T-D-3）。
  4. 视频历史"在工作区打开"（T-D-5）。
  5. 选中代码 → AI 解释（T-D-6）。
  6. 外部文件改动 Explorer 刷新（T-D-8，桌面集成）。
- **Lint**：`bun run lint`；新增 grep 守卫（见各卡 DoD）：
  - 无 `useState<PendingApproval | null>`（单例审批）。
  - `WorkspaceConsolePanel` 非单实例布尔。
  - `WorkspaceBrowserEditor` 主编辑器非 textarea。
  - settings/setup 页无原生 `window.confirm`。
  - `watch()` 不返回 `noopDisposable`。
- **回归基线**：执行前跑一次全量 `check/test/lint` 记录基线，每 PR 不得引入新失败。
