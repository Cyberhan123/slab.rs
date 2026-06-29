# Slab Next 规划会议结论

> 日期：2026-06-26
> 性质：跨职能产品+工程规划会议综合结论（主席 / 综合架构师产出）
> 依据：以现有源码为准（已交叉验证关键事实），遵守 AGENTS.md 边界红线，参考 2 份外网研究（多 agent 编排 + agentic UI）
> 北极星守护者：产品负责人

---

## 0. 执行摘要

本次会议围绕用户提出的 4 个核心痛点（统一入口缺失 / 插件与 Agent 打通 / 编码办公能力增强+workspace / Agent 多轮任务支撑）展开，由 6 位角色（产品负责人、插件生态开发者、前端/UX 架构师、后端工程师、SRE、Agent/AI 架构师）+ 2 份外网研究共同论证，达成 14 条可执行决策（ADR-001..014）。

**核心结论一句话**：Slab 的"统一入口"不是把页面塞进对话，而是把 Assistant 升级为 AgentShell——一个以 Agent 为编排者、通过受控 a2u tool call 派发受信 surface 的 shell。统一入口的成立依赖四个隐藏地基同时修复：① 结构化终止纪律（task.complete default-deny + DAG 规划 + 确定性校验）；② 循环兜底走 interrupt 不硬杀；③ 假完成数据一致性（max_turns 耗尽被标 Completed 的代码级隐患）；④ 能力可达性发现。

**关键代码事实（已交叉验证）**：
- [turn.rs:198](crates/slab-agent/src/turn.rs#L198) 已是结构化判停（`response.tool_calls.is_empty()`），用户担心的"文本判停"代码里不存在——保持并强化。
- [turn.rs:199-211](crates/slab-agent/src/turn.rs#L199) 已有 `reject_missing_required_tool_call`，但仅在 `tool_choice` required 时生效——default-deny 不完全，需 task.complete 强化。
- [thread.rs:248-451](crates/slab-agent/src/thread.rs#L248) `'turns` for 循环**正常退出（非 break、非 interrupted/last_error）会落到 ResponseCompleted + ThreadStatus::Completed**——turn 耗尽被标完成是真实隐患（SRE 痛点已验证）。
- [plan.rs:91-106](crates/slab-agent-tools/src/plan.rs#L91) `normalize_plan` 只回显，无 DAG/无持久化/无 result_ref/无 mark_done。
- [subagent.rs:117](crates/slab-agent-tools/src/subagent.rs#L117) `wait_for_terminal_snapshot` 同步阻塞，无法并行 spawn。
- [control.rs:381](crates/slab-agent/src/control.rs#L381) `interrupt` 与 [:357](crates/slab-agent/src/control.rs#L357) `shutdown` 已解耦，但 interrupt 的 soft-stop 能力被 thread.rs break 路径绕过。
- [risk.rs:7-45](crates/slab-agent/src/risk.rs#L7) `ToolRiskAnalyzer` trait + `BasicToolRiskAnalyzer` 已存在，但**仅识别 shell**——审批分级基建已部分就绪，是 P1 而非从零开始。
- [plugin.rs:392-395](crates/slab-types/src/plugin.rs#L392) `PluginCapabilityKind` 仅 `{Tool, Workflow}`，无 `a2u_surface`；[plugin.rs:290](crates/slab-types/src/plugin.rs#L290) `effects: Vec<String>` 字段已存在但未消费；[plugin.rs:407-409](crates/slab-types/src/plugin.rs#L407) `PluginCapabilityTransportType` 仅 `PluginCall`。
- [registry.rs:498-507](crates/slab-plugin/src/registry.rs#L498) 校验 `exposeAsMcpTool` 需 `mcpTool:expose` 权限，但 pluginCall capability **没被 host 注册成 agent 可见工具**——闭合缺口确认。
- [use-assistant-agent.ts:526-540](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L526) 把 tool_call 折叠进 ThoughtChain，**无派发表**。
- [routes/index.tsx](packages/slab-desktop/src/routes/index.tsx) `'agent'→Navigate '/'`（注意是 `'agent'` 非 `/assistant`），但 [use-workspace-page.ts:683](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L683) `navigate("/assistant")` 已验证为失效路径。

---

## 1. 参会角色与职责边界

| 角色 | 核心守护 | 决策归属 |
|---|---|---|
| 产品负责人（主席守护者） | 北极星、统一入口三态语义、任务总结闭环、场景化 | ADR-001/010、非目标守门 |
| 插件生态开发者 | a2u 四段契约、capability→tool 注册、DevX | ADR-009 |
| 前端/UX 架构师 | AgentShell、surface 状态机、派发表、设计系统 | ADR-002/010/011 |
| 后端工程师 | ToolContext 扩容、plan DAG、task.complete、循环检测、契约对齐 | ADR-003/004/006/007/008 |
| SRE | 多窗口契约、sidecar 恢复、假完成修复、并发预算、诊断包、secret | ADR-005/012/013/014 |
| Agent/AI 架构师 | 编排纪律、subagent 隔离、终止协议、a2u 工具集设计 | ADR-003/004/006 |

---

## 2. 北极星（结合用户原意重述）

Slab 的北极星是：**以本地优先、隐私优先、离线可用为默认底座，把"AI 全能力"收口到一个简单入口**——以 AI Agent 为核心编排者，以受控的 a2u（agent-to-UI）为辅助。

用户在与 AI 对话时即可：① **发起意图并被理解**；② **在对话内闭环**（轻量问答/计划审阅/结果摘要）；③ 由 Agent 通过**结构化 tool call 决定何时"打开专门面"**（生成媒体/长代码/diff 审阅/插件能力——ChatGPT Canvas 范式的确定性触发）；④ 何时仍需用户进**专业页面深度操作**（多文件重构/视频时间线精修）。

"原页面退化为子窗口/新 tab"的真正语义是 **surface 化派发**（派发关系）而非多窗口堆砌（塞入关系）。结构化终止（task.complete + DAG 规划 + 确定性校验）是闭环的纪律保障，**绝不退回文本判停**。

**北极星指标**：单任务对话完成率（对话内闭环/无需手动切页/无需手动重试），辅以计划审阅通过率、a2u 打开成功率、平均轮次/中断率、计划持久化续跑成功率、确定性校验通过率。定量+定性双轨。

---

## 3. 痛点共识（用户 4 条 + 角色补充，按影响排序）

### 3.1 用户原提 4 痛点
1. **统一入口缺失**：Assistant 已是首页但与其它页面并列，用户仍需手动点侧边栏。
2. **插件系统与 Agent 打通**：插件应在对话中通过 a2u 打开，而非用户去插件页操作。
3. **编码/办公能力增强 + workspace**：项目形式组织，workspace 设置正确合并，Agent 能做 code/文案/视频/照片。
4. **Agent 增强（多轮任务支撑）**：主 agent 全局规划/DAG/结果校验/完成判定；subagent 独立 ReAct + 结构化终止（禁文本判停）+ 上下文隔离；兜底全局 max turns + 重复检测。

### 3.2 角色补充的高影响痛点（去重，按影响排序）
- **【假完成数据一致性隐患】**（SRE，已代码验证）：[thread.rs:424-451](crates/slab-agent/src/thread.rs#L424) max_turns 循环正常退出被标 `ThreadStatus::Completed`，恢复时无法区分真完成与 turn 耗尽停摆——比用户提的兜底更隐蔽，是 resume 无据可依的根因。
- **【任务黑盒断点】**（PO）：plan 回显的 todo 没在前端可视化为可跟踪进度面板，长任务用户失去耐心和信任。
- **【跨页上下文双向断点】**（PO）：workspace/image 页的产物/选区无法作为上下文带回对话；Agent 产出文件改完后对话里看不到 diff。
- **【能力可达性断点】**（PO）：用户不知道 Agent 能做什么、本地装了什么，Agent 也不知道，导致过度承诺或沉默断在工具报错——统一入口能否成立的隐藏地基。
- **【工具审批一刀切断点】**（PO/插件）：read_file 和 write_file、打开面和执行 shell 都走同样审批流，每次都被打断。
- **【中断后续跑断点】**（PO/前端）：interrupt 已解耦但前端无"从 checkpoint 续跑"入口。
- **【插件 capability→agent tool 注册路径未闭合】**（插件）：pluginCall capability 没被 host 注册成 agent 可见工具。
- **【subagent 同步阻塞】**（后端）：无法并行 spawn，多 agent 烧 15x token 但没拿到并行收益。
- **【secret 明文落盘】**（SRE）：admin_api_token/provider key 随配置落盘，.slab/settings.json 作 overlay 会被 git 带走，与隐私优先红线张力。
- **【失效跨页路径】**（前端，已验证）：[use-workspace-page.ts:683](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L683) `navigate("/assistant")` 无对应路由。
- **【工具爆炸风险】**（插件）：N 插件 × M capability 会让 agent 工具表膨胀超 LLM 合理上限。
- **【设计系统变体裂变】**（前端）：缺 StateSurface 统一渲染契约。
- **【场景差异化缺失】**（PO）：对开发者/创作者/办公用户是同一个空壳。

---

## 4. 能力支柱（6 个）

| 支柱 | 描述 | 映射痛点 |
|---|---|---|
| **P1 统一入口 AgentShell + a2u 受控派发** | Assistant 升级为 AgentShell 常驻主区，原页面降级为 surface；host 侧固定派发表（tool 名→受信 React 组件）；三态语义由确定性输出形态判据触发+用户可覆盖 | 痛点1/2、跨页双向断点、失效路径、场景差异化 |
| **P2 插件 a2u 四段闭环 + 能力注入** | PluginCapabilityKind 增 a2u_surface，强制声明 schema+effects；host 注册 plugin.open 高阶工具；pluginMountView 扩 initialPayload（host 注入，caller id 从 label 推导）；插件以工具集形式注入对话 | 痛点2、capability→tool 未闭合、工具爆炸、热更新漂移 |
| **P3 Agent 多轮编排（Plan Mode + DAG + default-deny + 循环兜底）** | plan 升级 DAG+mark_done/replan+持久化；task.complete default-deny+确定性 verify；循环检测走 interrupt 不硬杀；终止理由结构化；subagent 补四要素+artifact 落盘只回引用 | 痛点4 全部、假完成隐患、任务黑盒、中断续跑 |
| **P4 审批分级 + 本地优先信任模型** | ToolRiskAnalyzer 强化（现状仅识别 shell）为 allow/sandbox/ask 三态；分级由 app-core 静态配置，插件不可自报；隐私数据流向在计划审阅/总结处标注 | 审批一刀切、隐私心智缺失、a2u 副作用域无守卫 |
| **P5 workspace 项目化 + 能力可达性发现** | workspace 切换=sidecar 优雅重启+task 受控迁移；settings 合并语义钉死；project context 上提 shell header；capabilities.available 发现工具 | 痛点3、能力可达性、场景差异化 |
| **P6 兜底与可观测（终止纪律 + 诊断包 + secret + 预算）** | 诊断包字段白名单+log rotation+secret redaction；统一 secret store；subagent span 关联；预算三重看板；并发软上限+内存熔断；CI 场景化测试门 | secret 落盘、log 无 rotation、并发裸常量、GLM 限流 |

---

## 5. 决策记录（ADR 式）

> 每条决策含背景/决策/后果。完整 owner/rationale 见结构化 agreed_decisions。

### ADR-001 定义「统一入口」三态语义并落地 host 侧固定派发表
- **背景**：[use-assistant-agent.ts:526-540](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L526) 把所有 tool_call 折叠成 ThoughtChain 节点，无派发；[routes/index.tsx](packages/slab-desktop/src/routes/index.tsx) 平铺路由。"统一入口"曾被工程视角误解为"把页面塞进对话"。
- **决策**：三态产品契约——对话内完成 / a2u 打开新面 / 专业页面深度。host 侧固定派发表（a2u-dispatcher.ts，Vercel Generative UI / Claude Artifacts 范式），模型只决定调哪个工具+参数，渲染哪个 React 组件由 host 完全固定。触发判据基于输出形态（生成媒体/长代码/多文件 diff/需迭代编辑），非 LLM 主观，用户可显式覆盖（Canvas 范式）。
- **后果**：正面——统一入口语义锚定，不演化为 Computer Use 变体；负面——派发表是前端固定映射，插件 a2u 工具会落"未知工具"兜底，需约束插件只能声明"指向 host 已有 surface 类型"而非自带渲染器。

### ADR-002 主窗内 surface 状态机为最小改造路径（非 Tauri WebviewWindow）
- **背景**：全仓只有 [window-controls.tsx](packages/slab-desktop/src/layouts/window-controls.tsx) 的 getCurrentWindow()，无多窗口基建。
- **决策**：P0 不破单窗，把 Layout `<Outlet/>` 升级为 surface 状态机（主对话流常驻+副 surface 以分屏/浮窗/内联卡片叠加），Tauri WebviewWindow 离窗化作 P2 可选增强。
- **后果**：避免每窗独立 WebView 上下文丢失共享/CSP 红线/IPC 成本/低配机 OOM（每 WebView ~150-300MB，16GB 机超 12 个必 OOM）；保留 sidebar rail 52px 视觉契约不推翻。

### ADR-003 plan 升级为带依赖边 DAG + mark_done/replan + 持久化进 slab-agent-memories
- **背景**：[plan.rs:91-106](crates/slab-agent-tools/src/plan.rs#L91) 只 normalize 回显。Anthropic Multi-Agent Research System 教训："save plan to Memory because 200K context will be truncated"。
- **决策**：DAG 数据结构（task_id+depends_on+status+result_ref）+ mark_done(task_id)/replan(plan_patch) 接口落 **slab-agent-tools**（确定性）；plan 持久化进 slab-agent-memories 独立 namespace（`plan:<thread_id>`，不污染通用 memory 检索）；slab-agent 只在 thread.rs 加 replan 分支与 memory 读写，编排核心零行规划逻辑。
- **后果**：守住 AGENTS.md slab-agent 纯编排红线；plan DAG 持久化需防被当普通 memory 检索回灌造成上下文污染（独立 namespace 缓解）。

### ADR-004 task.complete default-deny 结构化完成判定工具
- **背景**：[turn.rs:199](crates/slab-agent/src/turn.rs#L199) reject_missing_required_tool_call 仅 tool_choice required 时生效，default-deny 不完全。Anthropic 反模式：同一 LLM 自评会自信确认自己的错误。
- **决策**：新增确定性工具 task.complete(summary, artifact_refs, followup_actions)，完成判定 = (task.complete 调用) AND (plan 全节点 completed) AND (确定性 verify 通过)。task.complete 内部校验未满足返回错误回灌 LLM 继续；与 [turn.rs:198](crates/slab-agent/src/turn.rs#L198) tool_calls.is_empty() 兜底双轨。归 slab-agent-tools，由 app-core/runtime.rs 注册。
- **后果**：判定权从模型手里夺回给确定性逻辑；绝不引入文本/正则判停（用户担心的反模式代码里不存在，保持住）。

### ADR-005 修复 max_turns 耗尽被标 Completed 的「假完成」+ 结构化终止理由
- **背景**：已代码验证 [thread.rs:424-451](crates/slab-agent/src/thread.rs#L424) 'turns 循环正常退出落到 ResponseCompleted + Completed——turn 耗尽本质未完成却被持久化成完成态。
- **决策**：扩 ThreadStatus（MaxTurnsReached/Stopped）+ state.rs 状态迁移表 + SQLx migration 只追加 + 前端展示；终止理由结构化（Completed/MaxTurns/RepetitionDetected/BudgetExhausted/Interrupted/Errored）写 tracing+session；MaxTurns 走 [control.rs:381](crates/slab-agent/src/control.rs#L381) interrupt 语义（保留线程可续跑）而非硬杀。
- **后果**：跨层变更（slab-types/state.rs/store/前端），回归面大，需 gen:api/gen:schemas 同步；resume 逻辑终于有据可依。

### ADR-006 循环/重复检测兜底（走 interrupt 不走 shutdown）
- **背景**：[thread.rs:248](crates/slab-agent/src/thread.rs#L248) 只有 max_turns 硬上限 + invalid_tool_call_retries + 并发 32/深度 4，无重复检测；interrupt 已解耦却被 break 路径绕过。
- **决策**：近 N=3 轮 (tool_name, 关键 args 规范化签名) SHA-256 哈希，连续命中阈值=2 判 stuck（read_file/grep 等只读工具豁免或提高阈值）。处置阶梯：interrupt → 回灌 stuck 原因给 LLM 换策略一次 → escalate 到人。建议内联 slab-agent thread.rs（读 TurnOutcome 属编排内置状态，需架构签字）。
- **后果**：误报风险（合法渐进探索被判 stuck）由只读豁免+阈值可配置+先 soft-stop 缓解；绝不 shutdown 硬杀（Anthropic：restarts are expensive）。

### ADR-007 扩容 ToolContext 注入 workspace/session/记忆/计划句柄
- **背景**：[tool.rs:17-24](crates/slab-agent/src/tool.rs#L17) ToolContext 仅 thread_id/turn_index/depth，是 workspace-scoped context/plan 持久化/循环签名计算共同卡点。
- **决策**：以 trait object 注入（trait 在 slab-agent 定义，实现在 app-core/slab-agent-tools），新增字段全 Option + 提供 ToolContext::for_thread(thread_id) builder。
- **后果**：横切改动触及 turn_tool_call.rs 唯一构造点 + 所有 ToolHandler 测试构造处（tests.rs/subagent.rs），Option+builder 缓解；slab-agent 不反向依赖业务 crate（破坏分层）。

### ADR-008 工具审批三态分级（allow/sandbox/ask）强化现有 ToolRiskAnalyzer
- **背景**：[risk.rs:7-45](crates/slab-agent/src/risk.rs#L7) ToolRiskAnalyzer + BasicToolRiskAnalyzer 已存在但仅识别 shell——分级基建已部分就绪。
- **决策**：强化为按工具名静态分级：a2u 打开/渲染/只读默认 allow，改动类（write_file/apply_patch/git）default ask，危险/外部网络类 sandbox/拒。分级策略由 app-core 按工具白名单静态配置，**插件不可自报风险等级**（本地优先隐私模型，插件不可信）。
- **后果**：守住审批门不退化为 Cursor YOLO 全自动；减少弹窗同时守信任模型。

### ADR-009 插件 a2u 四段闭环 + capability→tool 注册
- **背景**：[plugin.rs:392](crates/slab-types/src/plugin.rs#L392) PluginCapabilityKind 仅 Tool/Workflow；[registry.rs:498](crates/slab-plugin/src/registry.rs#L498) 校验权限但 pluginCall capability 未注册成 agent 可见工具；[plugin.rs:290](crates/slab-types/src/plugin.rs#L290) effects 字段未消费。
- **决策**：PluginCapabilityKind 增 a2u_surface（gen:plugin-packs+gen:schemas，向后兼容 serde default），强制声明 schema+effects；host/app-core 注册 plugin.open(plugin_id,surface,payload) 高阶工具（落 host 层不进 slab-agent-tools）；pluginMountView 扩可选 initialPayload（**业务 payload 可来自 agent，但 caller 身份只从 WebView label 推导**，AGENTS.md:42 红线）；surface output 经 host 转 ToolOutput 回灌。
- **后果**：plugin.json shape 变需 gen；caller-id-from-label 红线是 a2u 设计最高风险点，任何"为了方便从 payload 取 pluginId 做路由"的捷径违规；工具膨胀需配按需启用机制。

### ADR-010 任务总结三动作按钮（open/review/feedback）+ AgentTimeline
- **背景**：[use-assistant-agent.ts:540](packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts#L540) turn_completed 只写文本；thoughts（:113）是扁平 tool 流。
- **决策**：turn_completed 携带 artifact_refs 时渲染 agent-action-card.tsx 三按钮——open→shell.openSurface('workspace',{revealPath})；review→shell.openSurface('review',{diff})；feedback→composer 注入草稿续跑不重启线程。/v1/agents/responses 扩 artifact_refs+reason（gen:api）。AgentTimeline 订阅 plan_update 结构化输出渲染 DAG 节点图。
- **后果**：闭环北极星；artifact_refs 文件路径/workspace 引用必须做权限校验（防 agent 产出指向 workspace 之外的危险引用被直接打开）。

### ADR-011 修复失效跨页路径 + 统一 AgentSurfaceStore
- **背景**：已验证 [use-workspace-page.ts:683](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L683) navigate("/assistant") 无对应路由；location.state 散用（:615 workspaceRevealPath）。
- **决策**：新建 store/useAgentSurfaceStore.ts（{draftPrompt, pendingSurface, surfacePayload}），废弃 useAssistantDraftStore 的 navigate+draft 双步，所有页面入口只 set pendingSurface，AgentShell 监听自动 openSurface。
- **后果**：消灭失效路径与散契约；多 surface 并存焦点/键盘/aria 管理复杂度上升（需焦点陷阱+Escape 收敛，否则 a11y 退化）。

### ADR-012 workspace 切换 = sidecar 优雅重启 + running task 受控迁移
- **背景**：[sidecar.rs:39](bin/slab-app/src-tauri/src/setup/sidecar.rs#L39) shutdown_server_sidecar 是 stdin shutdown+8s 超时+kill；workspace.rs init() 只启动期解析，运行中切换必然杀进程重拉，留假 running 线程。
- **决策**：切换前枚举 active thread（control.rs 已有 active_thread_count）→ 逐个 control.interrupt（保留线程）→ 写 session 快照到 .slab/sessions → 才 shutdown_server_sidecar。重启后 /v1/sessions 恢复，UI 显示"N 个任务已挂起，切换后可恢复"。任一步失败中止切换，绝不静默 kill。
- **后果**：修改在 bin/slab-app/src-tauri host 层 + 复用 control.rs interrupt，不扩 /v1/* 新 API 树；切 project 前若有正在跑的 agent thread，session 归属需绑定到 project。

### ADR-013 并发预算可配置 + 背压降级
- **背景**：[bootstrap.rs:168](crates/slab-app-core/src/infra/agent/bootstrap.rs#L168) 硬编码 max_threads:32/max_depth:4 无降级；Anthropic 多 agent 烧 ~15x token；MEMORY 索引记录本账号 ~10-14 并发就 429。
- **决策**：提为可配置（settings agent.runtime.limits 域，gen:schemas），超软阈值（如 16）新 spawn 进 FIFO 队列，host 内存监控（process_supervisor 已存在）超阈值停止新 spawn 并通知主 agent replan/降并发。叠加按 query 复杂度缩放 subagent 数的 prompt 规则。
- **后果**：需引入冷却窗口防"降级-升级"振荡；排队语义需暴露给前端。

### ADR-014 诊断包字段白名单 + log rotation + secret redaction
- **背景**：[assistant-markdown.test.tsx:262](packages/slab-desktop/src/pages/assistant/components/__tests__/assistant-markdown.test.tsx#L262) slab-server.log 已 919MB 无 rotation；[app_config.rs:81](crates/slab-config/src/app_config.rs#L81) admin_api_token 明文落盘。
- **决策**：export_diagnostics host-only Tauri command（不扩 /v1/*）；白名单显式枚举（版本/git/OS、sidecar 启动参数路径、log 末尾 N KB 已 redact、agent thread 统计不含 messages 原文、失败工具调用摘要 tool_name+error 不含 args 原文、资源快照），排除 slab.db/sessions 全量/admin_api_token/provider key/trace args 原文；tracing-appender rolling（50MB×5 份）；日志侧 redaction filter。
- **后果**：白名单由 SRE+安全共同签字冻结；比"不做诊断包"更危险的打包方式一票否决。

---

## 6. 路线图（对齐 slab-goal-plain 阶段 0-6，因统一入口重构优先级）

### Phase 0：基线校准 + 契约收敛（对齐阶段 0，2-3 周）
- **goal**：不动业务逻辑，先把契约与失效路径收敛、把红线纪律工程化。
- **exit criteria**：
  - ADR-011 失效跨页路径修复，跨页契约统一 AgentSurfaceStore
  - ADR-007 ToolContext 扩容 Option+builder 不破坏测试矩阵
  - CI 门禁：API shape 变 gen:api、schema 变 gen:schemas/gen:plugin-packs 进 CI 强制
  - workspace settings 合并语义钉死成契约文档
  - 诊断包字段白名单 SRE+安全签字冻结（即使未实现先冻结）
  - slab-server.log rotation + secret redaction 上线

### Phase 1：统一入口 AgentShell + a2u 派发最小可用（对齐阶段 1，3-4 周）
- **goal**：Assistant 升级为 AgentShell（主窗内 surface 状态机），a2u 派发表落地，任务总结三动作闭环。
- **exit criteria**：
  - ADR-002 AgentShell 常驻主区，原页面降级 surface，路由零破坏
  - ADR-001+010 a2u-dispatcher + agent-action-card 三按钮，turn_completed 扩 artifact_refs+reason（gen:api 已跑）
  - 内置 a2u 工具（workspace.open/image.edit/plugin.launch/review.show/hub.browse）作为 host/app-core 注册 ToolHandler 注入
  - ADR-008 ToolRiskAnalyzer 强化 allow/sandbox/ask 三态
  - 北极星指标埋点
  - E2E：用户说"打开 slab.rs"→ Agent 调 workspace.open → workspace surface 主窗分屏打开

### Phase 2：Agent 编排增强（Plan Mode + default-deny + 循环兜底 + 假完成修复）（对齐阶段 2，4-5 周）
- **goal**：Agent 从纯 ReAct 升级为 plan-and-execute + 结构化终止纪律。
- **exit criteria**：
  - ADR-003 plan.rs DAG + mark_done/replan + 持久化 slab-agent-memories 独立 namespace
  - ADR-004 task.complete default-deny + 确定性 verify
  - ADR-005 假完成修复，ThreadStatus 扩 + state.rs 迁移表 + SQLx 只追加 + 前端展示，MaxTurns 走 interrupt 可续跑
  - ADR-006 循环检测（近 N=3 SHA-256，阈值=2，只读豁免），interrupt→回灌→escalate
  - AgentTimeline + 前端中断续跑入口
  - E2E：长任务跑到 max_turns → 显示"已达轮次上限，可续跑"+ 终止理由

### Phase 3：插件 a2u 四段闭环 + workspace 智能化（对齐阶段 3-4，4-5 周）
- **goal**：插件以工具集注入对话（a2u 四段闭合），workspace 项目化与 Agent 打通，能力可达性发现。
- **exit criteria**：
  - ADR-009 PluginCapabilityKind 增 a2u_surface（gen:plugin-packs+gen:schemas），pluginCall capability host 注册成 agent 可见工具
  - plugin.open 四段闭环可测（声明→调用→渲染→回灌）
  - ADR-012 workspace 切换=sidecar 优雅重启+task 受控迁移
  - capabilities.available(domain) 发现工具上线
  - subagent 补四要素 + artifact 落盘 .slab/ 只回引用
  - 确定性 verify 工具（verify.workspace_build/lint/diff）作 plan 节点 result_ref
  - 插件 a2u 模板 + 调试反馈在 slab-plugin-cli

### Phase 4：多 agent 成熟 + 场景化 + 分发安全（对齐阶段 5-6，5-6 周）
- **goal**：subagent 并行委派 + 按复杂度缩放，场景化 onboarding，Tauri 离窗化，统一 secret store，安装器健康检查。
- **exit criteria**：
  - delegate_subagent 并行 spawn（tasks: Vec）+ 缩放 prompt + 预算看板 + GLM 限流
  - ADR-013 并发预算可配置 + FIFO 排队 + 内存熔断
  - 场景化 onboarding 并入 setup 向导，不新增平行路由
  - ADR-014 export_diagnostics host-only 实现（按 Phase 0 白名单）
  - 统一 secret store（keyring，新 crates/slab-secrets 论证归属）
  - Tauri WebviewWindow 离窗化按需推进 + capabilities 审计
  - 安装器首次运行健康检查
  - CI 场景化集成测试门覆盖多窗口/并发/恢复

---

## 7. 关键风险与缓解

| 风险 | 缓解 |
|---|---|
| DAG/规划被塞进 slab-agent 编排核心（违反纯编排红线） | 代码评审强校验：DAG/task.complete/replan/verify 全落 slab-agent-tools，slab-agent 只加 replan 分支+memory 读写。循环 guard 是唯一可例外内联（读 TurnOutcome），需架构签字 |
| task.complete 被模型过早调用绕过校验 | task.complete 内部校验 plan 全节点 completed + 确定性 verify 通过才 Final，否则错误回灌；保留 turn.rs:198 兜底双轨 |
| 循环检测误报（合法渐进探索被判 stuck） | 签名只对副作用类工具计数，只读豁免或提高阈值；命中先 interrupt soft-stop 给人工介入；阈值可配置 |
| 多 agent 烧 15x token（Anthropic） | 默认单 agent ReAct，prompt 按复杂度缩放（事实 1/对比 2-4/研究 8-10）；并发上限 32+深度 4 已有；P2 预算看板兜底；本机 GLM workflow ~10-14 并发 429（MEMORY）需限流 |
| a2u 工具集膨胀失控（VS Code 128 上限警示） | 刻意高阶、命名空间化、合并链式多步；按需启用（tools picker）；每工具配 eval 反推描述；保持个位数高阶工具 |
| Tauri WebviewWindow capabilities 膨胀 | label 前缀通配而非逐个声明，与最小权限拉扯需安全评审；P2 才推进，P0 不引入 |
| 审批分级退化为 YOLO 全自动 | 分级由 app-core 静态配置，插件不可自报；分级可减少弹窗但不消失（反 Cursor YOLO） |
| ToolContext 扩容改炸测试矩阵 | 新增字段全 Option + ToolContext::for_thread builder |
| sidecar 切换留幽灵线程（store Running 实际进程死） | 切换前必 interrupt + session 快照；任一步失败中止切换 |
| 诊断包打包泄漏 .slab/slab.db/sessions/trace args | 字段白名单先行（SRE+安全签字），默认排除数据库原文/args 原文/明文 secret；日志 redaction |
| 渲染面忽略宿主环境敏感性（Claude Artifacts iOS 教训） | 插件/动态面渲染遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱，不用任意 origin inline；host 固定受信组件 |
| plan 持久化被当普通 memory 检索回灌污染上下文 | 独立 namespace `plan:<thread_id>`，不进通用 memory 检索 |

---

## 8. 非目标（本阶段不做）

- **不做完整 Computer Use**（截图-坐标路线，Anthropic CU/OpenAI CUA）——违反隐私优先、烧 token、坐标不可靠；a2u 副作用域限于"打开受信 host 面/读写 sandbox 文件/调 /v1 API"，绝不操控任意像素。
- **不照搬 ADK/A2A 声明式框架或跨主体协议**——Slab 是 Rust 命令式+本地优先隐私模型，ADK 类体系与 A2A 跨厂商互操作不符合纯编排边界与信任模型；只借鉴"显式终止条件""artifact 解耦"思想。
- **不照搬 Cursor YOLO mode 全自动**——审批可分级但不消失。
- **不引入文本/正则判停**——turn.rs:198 已结构化判停，保持绝不反向。
- **不为每个原页面暴露低阶原语工具**（open_tab/scroll/click_xy）——ACI 工具应高阶、命名空间化。
- **不新增平行 API 树/第二套 LSP bridge**——只扩 /v1/*；LSP 只走 /v1/workspace/lsp/{language}→app-core；诊断包 host-only；多窗口全落 host 层不让 slab-app-core 感知。
- **不把 DAG/规划/校验塞进 slab-agent 编排核心**。
- **不引入 Module Federation 作为默认插件模型**——Tauri child WebView/iframe sandbox。
- **不在 P0 引入多窗口基建风险**——主窗内 surface 状态机优先。
- **不默认就上多 agent**——subagent 是 researchers not coders，编码类主仓库修改留主线程。
- **不新增 /onboarding 平行路由**——场景化并入现有 setup 向导。
- **SQLx migration 只追加**——plan 持久化倾向先走 slab-agent-memories 文件存储。

---

## 9. 开放问题（需后续对齐，不阻塞 Phase 0-1）

1. **task.complete 触发 Final 的优先级**：是普通工具调用（调完下一轮 LLM 自然不调工具走 turn.rs:198）还是特殊控制工具（调即 Final 短路）？倾向后者但需确认不破坏 tool_calls 判停契约。
2. **plugin.open 是单一高阶工具（agent 传 plugin_id+surface+payload）还是每个 surface 注册独立命名工具**？前者工具表小但多一轮 list，后者膨胀但 ACI 友好——结合按需启用机制定。
3. **a2u surface outputSchema 回灌是同步（阻塞 turn）还是异步（background）**？影响 turn.rs 等待语义。
4. **循环 guard 放 slab-agent 内联还是独立 slab-agent-loopguard crate**？倾向内联（读 TurnOutcome 属编排内置状态），需架构签字。
5. **多窗口下插件 child WebView 与原页面退化子窗口是否共用同一 Tauri WebView 资源池**？决定 P0 多窗口契约实现位置。
6. **审批分级策略表放全局 settings 还是 workspace settings**？workspace 覆盖全局但安全策略应全局锁底，需安全/工程对齐。
7. **MaxTurnsReached 新状态是否需 SQLx migration**？还是复用现有 status+reason 字段？
8. **keyring crate 归属**：crates/slab-config（最近 secret() 占位符）还是新 crates/slab-secrets（独立职责）？建议后者。
9. **统一入口 shell 是否合并 image/video/audio 三媒体页为"媒体工作台"surface**？P0 保守保留三页只让 agent a2u 打开，P2 激进可合并——产品拍板。
10. **followup_actions 与现有 useAssistantDraftStore 跨页跳转如何对接**？复用还是独立 action schema。

---

## 10. 交付物与边界归属汇总（To-Be 新增文件）

| 新增/改动 | 归属 crate/包 | 边界论证 |
|---|---|---|
| task_complete.rs / plan_dag.rs / verify.rs | crates/slab-agent-tools/src/ | 确定性工具，符合 AGENTS.md 红线 |
| repetition_guard（循环检测） | crates/slab-agent/src/thread.rs 内联（建议） | 读 TurnOutcome 属编排内置状态，需架构签字；或独立 crate |
| TerminationReason 状态机骨架 | crates/slab-agent/src/thread.rs + turn.rs | 编排核心，仅状态/循环/分支 |
| ToolContext 扩容 | crates/slab-agent/src/tool.rs | 公共类型 shape 变，只加 Option 字段 |
| ThreadStatus 扩 MaxTurnsReached | crates/slab-types + state.rs + store migration | 跨层，SQLx 只追加 |
| PluginCapabilityKind::a2u_surface | crates/slab-types/src/plugin.rs | 确定性数据结构，gen:plugin-packs+gen:schemas |
| plugin.open ToolHandler / pluginCall capability 注册 | bin/slab-app/slab-app-core（host 层） | **不进 slab-agent-tools**，守纯编排红线（pluginCall 涉插件生命周期/sandbox 属 host 职责） |
| a2u-dispatcher.ts / agent-action-card.tsx / agent-shell.tsx / useAgentSurfaceStore.ts | packages/slab-desktop/src（纯前端） | 无需破例 |
| agent-surface-window.ts（P2 离窗化） | bin/slab-app/src-tauri | 涉 Tauri capabilities，需 CSP/capabilities 审计 |
| export_diagnostics | bin/slab-app/src-tauri（host-only） | 不扩 /v1/* 红线 |
| crates/slab-secrets（P1 统一 secret store） | 新 crate | 独立职责，不污染 slab-config/slab-app-core |
| /v1/agents/responses 扩 artifact_refs+reason | bin/slab-server（扩 fields） | 仅扩不新增 API 树，gen:api |
| plan 持久化 namespace | crates/slab-agent-memories | 独立 namespace plan:<thread_id>，不碰 DB migration |

---

**会议结论签署**：6 角色共识达成，14 条 ADR 可执行，路线图 Phase 0-4 对齐 slab-goal-plain 阶段 0-6。下一步进入 Phase 0 执行。

> 本结论以现有源码为准（关键事实已交叉验证），遵守 AGENTS.md 边界红线，不臆造 API/文件。所有 To-Be 新增文件已显式标注边界归属与论证。