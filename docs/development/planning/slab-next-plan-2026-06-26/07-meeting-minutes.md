# 附录 B · 会议发言纪要（Meeting Minutes · 6 角色视角）

> 本附录是会议 Phase 2 六位角色（产品负责人 / 插件生态开发者 / 前端·UX 架构师 / 后端工程师 / 基础设施·SRE 工程师 / Agent·AI 架构师）的结构化发言，保留原始视角与分歧，供追溯。

---

## 产品负责人（用户视角 / 北极星守护者）—— 用户价值优先、统一入口定义、北极星指标、场景化、用户旅程里程碑

### 核心关切
- 「统一入口」被工程视角误解为「把所有页面塞进对话」。从用户视角，统一入口的真正语义是：用户的意图在对话里发起并被 Agent 理解，Agent 通过结构化 tool call 决定三件事——哪些在对话内完成（轻量问答/计划审阅/结果摘要）、哪些 a2u 打开新面（生成媒体/长代码/diff 审阅/插件能力）、哪些仍需专业页面深度操作（多文件重构/视频时间线精修/复杂数据建模）。当前 routes/index.tsx 是平铺的页面表，Assistant 已是 / 首页但与其它页面是并列关系而非「派发关系」——用户仍需手动去点侧边栏。
- Agent 做了事但用户不知道「做完了没有、做得对不对」。plan.rs 只回显 todo（crates/slab-agent-tools/src/plan.rs:99 normalize_plan），没有完成判定、没有结果校验、没有「为什么停」的理由。用户在任务总结处看到的是一段自然语言文本，没有可操作的 open/review/feedback 行动按钮，无法形成闭环。这是用户最痛的「任务黑盒」断点。
- 审批弹窗与权限心智。use-assistant-agent.ts:472 的 approval_required 每个工具阻塞等审批，但当前是「一刀切」——读文件和写文件、打开面和执行 shell 都走同样的审批流。对统一入口体验是致命的：用户每次都被打断。需要按副作用域分级（打开/只读=allow、改动=ask、危险=sandbox），否则「对话即完成一切」会变成「对话即不停弹窗」。
- 本地优先与能力可达性的张力。Slab 的北极星是「本地+隐私优先的 AI 全能力入口」，但 diffusion/whisper/candle 等能力的发现与启用门槛高（要装模型、要配 workspace、要装插件）。用户在对话里说「帮我处理这张照片」时，Agent 若不知道本地有没有图像模型/相关插件，就会断在这。能力可达性（capability discovery）是统一入口能否成立的隐藏前提。
- 场景差异化缺失。开发者、创作者、办公用户对「统一入口」的期待完全不同：开发者要 Agent 进 workspace 改代码 + 跑测试；创作者要 Agent 生成/编辑媒体并打开预览迭代；办公用户要 Agent 写文档/做表格/总结。当前产品没有按场景剪裁的 onboarding 和能力配置，统一入口对所有人是同一个空壳。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| 定义「统一入口」三态语义并落地 a2u 受控工具派发表（host 侧固定派发，非任意 Computer Use） | P0 | L | 痛点1 统一入口缺失 + 痛点2 插件系统与 Agent 打通 | 回应 must-answer 1。把「在对话里完成 / a2u 打开新面 / 专业页面深度」三态固化为产品契约：(1) 对话内完成=轻量问答、计划审阅、结果摘要；(2) a2u 打开新面=当任务输出形态为「生成二进制媒体/长代码/多文件 diff/需迭代编辑」时（ChatGPT Canvas 的确定性触发判据），Agent 通过结构化 tool call（如 workspace.open / image.edit / plugin.launch / review.show）请求 host 打开对应面；(3) 专业页面深度=多文件重构/视频时间线精修等仍由用户主动进专业页面。实现路径：在 packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts:529 的 tool_call_started 分支基础上，新增一个 host 侧「UI 工具名 → React 组件/动作」派发表（Vercel Generative UI / Claude Artifacts 范式）——模型只决定调哪个工具+参数，渲染哪个组件由 host 完全固定。这是把 routes/index.tsx 的平铺页面表升级为「Assistant 可派发的面集合」，而不是把页面塞进对话。 |
| 任务总结三动作按钮（open / review / feedback）+ 结构化 task.complete 判停 | P0 | M | 痛点4 Agent 多轮任务支撑 + 任务总结无闭环 | 回应 must-answer 4 的产品语义。在 use-assistant-agent.ts:540 的 turn_completed 事件处，当 Agent 产出最终答复时，按产物类型渲染三个确定性动作按钮（不是任意跳转）：(a) open（产物存在时）→ 在对应面打开 Agent 产出物（workspace 定位文件/图像预览/diff 面）；(b) review（产物含 diff/改动时）→ 进入 review 面供人工审阅；(c) feedback（始终可选）→ 回写意见续跑 Agent（等价于在当前 thread 继续 input）。后端配套：新增确定性工具 task.complete(summary, artifact_refs)（归 slab-agent-tools，符合 AGENTS.md「确定性工具进 slab-agent-tools」红线），由 LLM 在所有 plan 节点 mark_done 后调用判停——强化并保持 turn.rs:198 已有的 tool_calls.is_empty() 结构化判停语义，绝不引入文本判停（用户担心的反模式代码里不存在，保持住）。 |
| 升级 plan_update 为 Plan Mode（可审阅计划 + DAG + 持久化 + mark_done/replan） | P0 | L | 痛点4 主 agent 全局规划/DAG/完成判定 | 直击「主 agent 全局规划、DAG、结果校验、判断任务完成」诉求。当前 plan.rs:99 normalize_plan 只回显无状态。升级为：(1) plan 数据结构加 task_id + depends_on + status + result_ref（DAG 作为 plan 工具内部确定性表示，归 slab-agent-tools，不污染 slab-agent 纯编排）；(2) 新增 mark_done(task_id, result_ref) 与 replan(plan_patch) 接口；(3) plan 持久化到 slab-agent-memories 防 200K context 截断丢计划（Anthropic 多 agent 研究系统教训）；(4) 主循环 thread.rs:248 for turn 结束后允许 LLM 调 replan 触发动态重规划；(5) Cursor Plan Mode 范式——产出可审阅计划（含文件路径/资源引用+完成判据）等 ApprovalPort 批准后才执行。这是「主 agent 先看全局再动手」的基础，也是 task.complete 判停的前置（所有节点 mark_done 才 complete）。 |
| 工具审批三态分级（allow/sandbox/ask）对齐 ApprovalPort | P1 | M | 痛点1 统一入口体验被审批打断 + 本地优先信任模型 | 解决「对话即不停弹窗」断点。在 turn_tool_call.rs:418 的审批门基础上，按副作用域静态分级 + 动态门（Cursor v3.6 / VS Code 范式）：a2u「打开/渲染/只读」类工具（workspace.open / image.preview / read_file / list_dir）默认 allow（无副作用，不阻塞对话流）；改动类（write_file / apply_patch / git commit）default ask（保留人工把关）；危险/外部网络类（shell 执行 / web 操作未授权资源）sandbox 或拒。这是把当前一刀切的 approval_required 升级为按工具分级的策略表，既减少弹窗又守住本地优先+隐私优先信任模型。绝不照搬 Cursor YOLO 全自动——审批可以分级但不能消失。 |
| 循环/重复检测兜底 + 终止理由结构化（保留线程可续跑，非硬杀） | P1 | M | 痛点4 兜底：全局 max turns + 重复检测 | 补齐「全局最大 turn 轮次、多轮重复任务检查」兜底。当前只有 thread.rs:248 的 0..max_turns 硬上限 + invalid_tool_call_retries，无重复检测。新增：(1) 近 N 轮工具调用签名去重计数（对 (tool_name, 关键 args) 哈希，连续重复命中阈值判 stuck）；(2) 命中后走 control.rs:381 已解耦的 interrupt（保留线程+检查点+可续跑+human-in-the-loop），而非 shutdown 硬杀（Anthropic：restarts are expensive and frustrating）；(3) 把 max_turns 命中从静默 break 升级为带 reason 的结构化终止（completed / max_turns / repetition_detected / budget_exhausted / interrupted），写入 tracing 与 session，前端在任务总结处向用户展示「为什么停」。 |
| 能力可达性发现（capability discovery）—— Agent 知道本地能做什么 | P1 | M | additional_pain_point 能力可达性 + 本地优先与能力发现张力 | 这是统一入口能否成立的隐藏前提（additional_pain_point 第3条）。用户在对话里说「处理这张照片」时，Agent 需要知道本地装了哪些模型（diffusion/whisper）、哪些插件提供相关 agentCapability、当前 workspace 是否启用。实现：暴露一个只读的高阶发现工具（如 capabilities.available(domain)），返回自然语言摘要（Anthropic ACI：工具返回高信号低 token 上下文，UUID 解析成自然语言名）+ 可选 ID，喂回 TurnOutcome。让 Agent 在能力不可达时主动告知「需要先装 X 模型/启用 Y 插件」并给出 a2u 引导，而不是默默断在工具报错。 |
| 场景化 onboarding 与能力配置剪裁（开发者/创作者/办公） | P2 | L | additional_pain_point 场景差异化缺失 + 痛点3 编码/办公能力增强 | 回应「面向不同用户场景」。统一入口对三类用户应是「同一个壳、不同的能力默认集」：开发者默认启用 workspace + LSP + git + 编码工具集；创作者默认启用 image/video/audio 生成编辑 + 媒体预览迭代；办公用户默认启用文档/表格/总结 + hub 插件。在 setup 向导（routes/index.tsx:53 的 SetupPage）增加场景选择，按场景预配工具白名单 + 推荐 workspace 模型组合。delegate_subagent 的 allowed_tools（subagent.rs:29）也可按场景给只读研究类子任务配 read-only 工具集。这避免「对所有人是同一个空壳」。 |
| 插件以「工具集」形式注入对话（agentCapabilities.exposeAsMcpTool 收敛为 a2u 工具） | P1 | L | 痛点2 插件系统与 Agent 打通 | 回应痛点2。插件不应让用户去 plugins 页操作，而应在对话里被 Agent 通过 a2u 调用。利用已有 manifest 的 contributes.agentCapabilities[].exposeAsMcpTool（共享上下文已确认存在），把插件能力收敛为命名空间化的高阶工具（plugin.<plugin_id>.<action>），按 Anthropic ACI 命名空间化原则减少 Agent 在工具间混淆。插件 WebView 仍走 Tauri child WebView（AGENTS.md 红线：caller id 从 label 推导），但触发方式从「用户点页面」变为「Agent tool call → host 打开 child WebView」。这是「插件系统与 Agent 打通」的产品语义落地。 |
| 结果校验用确定性工具（编译/lint/diff/媒体元数据），禁 LLM 自评 | P2 | M | 痛点4 结果校验 + additional_pain_point 任务黑盒 | 回应「结果校验 + 完成判定」+ 防 Anthropic 反模式（同一 LLM 自评会自信确认自己的错误）。对编码/媒体类任务，task.complete 前跑确定性校验工具（workspace 已有 LSP，crates/slab-app-core owns LSP provider resolution）：编码→编译/lint/diff；媒体→元数据/格式校验。校验结果作为 plan 节点的 result_ref，失败回灌给 LLM 重试（复用 invalid_tool_call_retries 预算）。校验工具归 slab-agent-tools（确定性），不引入第二个模型，不污染 slab-agent 编排。 |

### 补充痛点（用户未提）
- 「任务黑盒」断点：用户看到 Agent 在跑（thoughts 列表一堆 tool_call），但不知道整体进度、剩余步骤、当前卡在哪。plan.rs 回显的 todo 没有在前端可视化呈现为可跟踪的进度面板（use-assistant-agent.ts:547 setThoughts 只是线性列表），长任务用户失去耐心和信任。
- 「能力发现」断点：用户不知道 Agent 能做什么、本地装了什么。对话里问「能做 X 吗」时没有能力清单，Agent 也不知道本地是否有对应模型/插件，导致要么过度承诺要么沉默。这是统一入口的隐藏地基——入口统一了但能力不可达，体验更差。
- 「跨页上下文丢失」断点：Assistant 已是首页且有 useAssistantDraftStore 支持跨页跳转（如 workspace「用助手解释代码」），但反向不通——用户在 workspace/image 页的产物/选区无法作为上下文带回对话继续多轮。Agent 产出的文件改完后，对话里看不到 diff、无法基于产物继续「再改一下」。双向上下文流是统一入口的灵魂。
- 「中断后续跑」断点：interrupt（control.rs:381）已解耦保留线程可继续，但前端 use-assistant-agent.ts:1144 的 interruptThread 后没有「从检查点续跑」的入口，用户中断后只能重发，长任务前功尽弃。这与「多轮任务支撑」直接冲突。
- 「Agent 行为不可预测」断点：没有终止理由、没有 plan 可审阅，用户无法预判 Agent 会做什么、会不会跑飞。Cursor 的 Plan Mode（先出计划等批准再动手）正是治这个——Slab 当前 plan_update 不阻塞执行，用户全程被动。
- 「隐私心智缺失」断点：本地优先是 Slab 北极星，但用户在对话里不知道「这句话/这个文件会不会发到云端 provider」。统一入口把所有能力收口到对话后，用户对数据流向的不确定感会被放大。需要在任务总结/计划审阅处明确标注「本地处理 / 云端 provider / 外部 MCP」的数据去向。

### Must-have
- 「统一入口」三态语义在产品文档与代码中显式定义并落地：对话内完成 / a2u 打开新面 / 专业页面深度——三态有明确的触发判据（输出形态，非 LLM 主观）且用户可显式覆盖。
- 任务总结处必须有三动作按钮（open / review / feedback），每个按钮对应一个受控 a2u tool call 的逆操作，不是任意跳转。
- 保持并强化结构化判停（turn.rs:198 tool_calls.is_empty()），引入显式 task.complete 工具；绝不退回文本/正则判停（用户担心的反模式代码里不存在，保持住）。
- 北极星指标确立：北极星=「单任务对话完成率」（对话内闭环/无需手动切换页面/无需手动重试），辅以计划审阅通过率、a2u 打开成功率、平均轮次/中断率、计划持久化续跑成功率、确定性校验通过率。定量+定性双轨。
- 工具审批三态分级（allow/sandbox/ask），按副作用域而非一刀切；本地优先信任模型不可破坏。
- 所有 plan/DAG/task.complete/verify 工具落在 slab-agent-tools（确定性），slab-agent 保持纯编排；a2u 工具副作用域限定在「打开受信 host 面 / 读写 sandbox 文件 / 调 /v1 API」，绝不操控任意像素。
- 循环/重复检测 + 终止理由结构化（completed/max_turns/repetition_detected/budget_exhausted/interrupted），命中走 interrupt 保留线程可续跑，非硬杀。

### 风险
- 范围蔓延到「完整 a2u / Computer Use」：统一入口容易被错误地实现为截图-坐标驱动（Anthropic Computer Use / OpenAI CUA 路线），带来高 token 成本、坐标不可靠、违反隐私优先。必须守住「受控 tool call + host 固定组件派发」红线，仅当面对无 API 的外部 GUI 才考虑受限 browser tool 且默认 ask。
- 「把 DAG/规划/校验塞进 slab-agent 编排核心」违反 AGENTS.md 红线（slab-agent 保持纯编排）。DAG 数据结构、task.complete、verify 工具必须落在 slab-agent-tools，由 host/app-core 注册。工程落地时易越界，需 PO 守门。
- 审批分级退化为 YOLO 全自动：为减少弹窗可能被推向 Cursor YOLO mode（全自动跑 terminal/MCP 无确认），违反本地信任模型和 turn_tool_call.rs:418 审批门设计。分级可以但不能消失。
- 多 agent 过度投资：Anthropic 明确多 agent 比 chat 烧约 15× token。统一入口不等于「每个任务都 spawn subagent」。默认单 agent ReAct，仅在研究/广度探索/产物隔离场景才 delegate_subagent，且按查询复杂度缩放子 agent 数（事实类 1 个，对比类 2-4 个，复杂研究 10+）。
- a2u 工具集膨胀到 100+：VS Code 128 工具上限是警示。Anthropic 指出工具贵精不贵多，重叠/低信号工具会分散 Agent 注意力。必须刻意保持 a2u 工具集小而高阶、命名空间化、任务式（合并链式多步）。
- 渲染面忽略宿主环境敏感性：Claude Artifacts iOS 渲染失败教训——插件/动态面渲染必须遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱边界，不能用任意 origin 的 inline 内容，否则绕过 WebView caller id 从 label 推导的红线。
- LLM 自评完成（自信确认自己的错误）：task.complete 不能由主 Agent 自我宣布完成代替校验，必须叠加确定性校验工具（编译/测试/lint/diff）。

### 边界红线注意
- 「统一入口」不得演化为新增平行 API 树——所有 a2u 工具仍走 /v1/* 扩展（AGENTS.md 红线），shape 变化要 bun run gen:api。
- a2u 受控工具派发表不得让模型自由生成任意 HTML/JS/React 渲染（= Computer Use 变体）——渲染面必须是 host 固定的受信组件，遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱边界。
- 插件 a2u 调用时，WebView caller id 必须从 WebView label 推导，不从插件 payload 字段（AGENTS.md 红线）；插件 WebView 仍走 Tauri child WebView，不引入 Module Federation 作为默认。
- Plan Mode 的 plan 持久化若写入 .slab/ 工作目录，需对齐 workspace-mode-design.md 的 .slab/{settings.json,workspace.json,slab.db,sessions} 结构，不另起目录体系；plan 持久化到 slab-agent-memories 则需确认该 crate 的存储边界。
- task.complete / verify / DAG 数据结构必须落 slab-agent-tools，不得污染 slab-agent 纯编排核心（AGENTS.md 红线：内置确定性工具进 slab-agent-tools，插件/API 能力由 host/app-core 注册）。
- 结果校验若复用 workspace LSP，必须走 /v1/workspace/lsp/{language} -> app-core 既定路径，桌面 host 不新增第二套 LSP bridge（AGENTS.md 红线）。
- 审批分级策略表的配置若涉及 settings schema 变化，必须 bun run gen:schemas；若涉及 plugin manifest 的 agentCapabilities，需 gen:plugin-packs。
- 循环/重复检测的 interrupt 不得退化为 shutdown 硬杀——保留线程+检查点+可续跑是产品底线（control.rs:381 已解耦，勿反向耦合）。
- 场景化 onboarding 不得引入平行页面树——场景配置应并入现有 settings 与 setup 向导（routes/index.tsx:53 SetupPage），不新增 /onboarding 等平行路由。

### 开放问题
- 统一入口的「a2u 打开新面」是单窗口内切面（替换当前主区域）还是真多窗口（WebviewWindow/tab）？当前无多窗口基础设施（只有 getCurrentWindow 的 min/max/close）。PO 立场倾向单窗口内「主区域 = 对话，副区域 = 派出面」的分栏，而非多窗口——但这与「原页面退化为子窗口/新 tab」的用户原话有张力，需与工程对齐。
- task.complete 的 artifact_refs 如何与 workspace 的 .slab/ 工作目录对齐？subagent 产物落盘 + 回传引用是 Anthropic 推荐模式，但 artifact 的生命周期（临时 vs 持久）和跨 workspace 的引用解析需设计。
- 场景化 onboarding 的场景分类粒度：三类（开发/创作/办公）够不够？是否需要更细（如「学生」「研究者」）？过度细分会让 setup 向导变重，过粗则失去剪裁价值。需要用户研究支撑。
- 能力可达性发现工具的返回粒度：返回全部能力清单 token 成本高，返回太少 Agent 决策不准。Anthropic ACI 建议 response_format=concise|detailed 让 Agent 自控粒度——是否采纳？
- 审批分级的配置归属：分级策略表放全局 settings 还是 workspace settings？workspace 设置覆盖全局（workspace-mode-design.md），但安全策略理论上应全局锁底。需与安全/工程对齐。
- feedback 动作回写的语义：是「在当前 thread 继续 input 续跑」还是「fork 一个新分支任务」？前者保留上下文连续性，后者避免污染主线。Cursor 的 branched response（startBranchedResponse use-assistant-agent.ts:967）已有先例可借鉴，但产品语义需明确。

---

## 插件生态开发者（Plugin Ecosystem Developer / 插件开发视角）

### 核心关切
- a2u 最小契约缺失：现有 tool_call 事件流 (packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts:526-539) 只把 tool_call 渲染成一行 thought，没有 tool 名 -> host 受控 UI 面/动作 的派发表，agent 无法通过结构化工具调用打开插件面。这正是 Vercel Generative UI / Canvas 的核心范式缺口。
- 插件 capability -> agent tool 的注册路径不闭合：contributes.agentCapabilities[] 已定义 (crates/slab-types/src/plugin.rs:278-294) 且 registry 校验 exposeAsMcpTool 需 mcpTool:expose 权限 (crates/slab-plugin/src/registry.rs:498-507)，但没有看到把 pluginCall transport 的 capability 注册成 agent 可见工具（区别于 mcp_call/mcp_proxy 在 crates/slab-agent-tools/src/mcp.rs）。插件工具集无法被 host 注入到 agent。
- 插件与 agent 打通的对称缺口：插件能声明 agentHooks（on_tool_start/end 等生命周期，plugin.rs:308-330）和 agentCapabilities，但没有反向的 agent->插件 a2u 动作通道——即 agent 在对话中打开插件面、把结果回灌的回环不存在。
- 插件 WebView 渲染面是静态路由（packages/slab-desktop/src/routes/index.tsx:40-47 把 contributes.routes 直接映射成 PluginWebviewPage），无法被 agent 带参数动态打开/聚焦；plugin_mount_view 只接受 pluginId+bounds，不接受 payload/初始状态。
- 插件开发者 DevX：没有 a2u 插件的模板/示例，permissions 校验（registry.rs:445-509）很严格但 capability 注册的端到端链路（manifest -> 校验 -> host 注册 -> agent 可见）对开发者是黑盒，缺乏调试反馈。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| P0: 定义并实现 a2u 最小契约（plugin 声明 / agent 调 / 前端渲染 / 结果回灌的四段闭环） | P0 | L | 痛点2（插件与 Agent 打通 a2u）；痛点1（统一入口）；插件北极星 | 这是会议北极星「agent 在对话中打开插件面」的根契约，当前完全缺失。契约必须四段闭合：(1) 插件侧——扩展 contributes 复用现有 PluginAgentCapabilityContribution (plugin.rs:278)，新增 kind="a2u_surface"（PluginCapabilityKind 当前只有 Tool/Workflow，plugin.rs:392-395），声明 surface（host 已知的受信面 id）、inputSchema（agent 传入的 payload schema）、outputSchema（回灌 agent 的结果 schema）；(2) agent 侧——host 注册一个高阶工具 plugin.open(plugin_id, surface, payload)，在 turn_tool_call.rs 审批门内执行（无副作用类 surface 默认 allow），执行后把 surface 的 outputSchema 回灌为 ToolOutput.content（对齐 mcp.rs:61-65 的 ToolOutput 形状）；(3) 前端侧——在 use-assistant-agent.ts:526-539 的 tool_call 分支加一个 tool 名 -> host 受控 React 组件/动作 的派发表，plugin.open 命中时调用 host 已知的导航/PluginWebviewPage 挂载并注入 payload（pluginMountView 当前只接 pluginId+bounds，需扩一个可选 initialPayload，从 host 侧注入而非 plugin 侧 payload，守住 caller-id 红线）；(4) 结果回灌——surface 内插件通过 slab-plugin-sdk 的 host bridge（postMessage）把 output 回传，host 转成 ToolOutput 喂回 agent turn。 |
| P0: 打通 pluginCall capability -> agent tool 的 host 注册路径（不破坏 exposeAsMcpTool / agentCapabilities / agentHooks） | P0 | L | 插件 capability -> agent tool 注册路径；不破坏现有能力体系 | registry 已校验 capability:declare 权限（registry.rs:472-477），但 transport.type=pluginCall 的 capability（plugin.rs:407-409 只有 PluginCall 一种）没有被 host 注册成 agent 可见工具。路径应为：host/app-core 启动时遍历 loaded_plugins().manifest.contributes.agent_capabilities，对每个 pluginCall capability 在 host 侧构造一个 ToolHandler（类似 McpProxyTool 包 McpClient，这里包 bin/slab-js-runtime 的 JSON-RPC pluginCall，走 AGENTS.md:36-37 的 /v1/plugins/rpc）。关键不破坏点：(a) exposeAsMcpTool=true 的 capability 仍走 mcp_call/mcp_proxy 路径（mcp.rs），不重复注册；(b) agentHooks 仍走生命周期分发，不被当作工具；(c) 注册发生在 host/app-core 层（AGENTS.md:29 明确插件/API 能力由 host/app-core 注册），绝不进 slab-agent 编排核心。ToolHandler 命名空间化为 plugin__{plugin_id}__{capability_id}（对齐 mcp.rs:159-161 的 sanitize 规则）。 |
| P0: 越权与审批分级对齐现有 permissions + ApprovalPort（a2u 工具静态分级 + 动态门） | P0 | M | 越权与审批对齐现有 permissions | a2u/插件工具必须复用现有两层防线而非新建：(1) 静态权限层——registry 已有的 permission 校验矩阵（registry.rs:445-509）：a2u surface 声明需 permissions.ui 含 surface:open（类比 route:create/sidebar:item:create），capability:declare/mcpTool:expose 已就绪，新增 a2u 不破坏既有 7 条校验规则；(2) 动态审批门——turn_tool_call.rs:418 每个 tool 阻塞等 ApprovalPort，a2u 工具按 Cursor/VSCode 的 allow/sandbox/ask 三态分级：只读/打开渲染类 surface 默认 allow（无副作用），改动/执行类（plugin 写文件、调网络 allowlist 外）default ask，危险类拒。caller-id 推导红线（AGENTS.md:42）必须贯穿：plugin.open 调插件时，插件 WebView 的 caller id 由 WebView label 推导，a2u payload 从 host 注入，绝不信任 plugin-supplied payload 字段做身份判定。 |
| P1: 提供插件开发者写 a2u 插件的最小步骤 + manifest 片段 + SDK 模板（DevX） | P1 | M | 插件开发者写 a2u 插件的最小步骤；DevX | 插件生态成败取决于开发者能否 10 分钟跑通一个 a2u 插件。最小步骤：(1) npx create-slab-plugin（packages/slab-plugin-cli 已存在，AGENTS.md:110）选 a2u 模板；(2) 写 plugin.json：runtime.ui.entry + contributes.agentCapabilities[]{kind:"a2u_surface", transport:{type:pluginCall, function}, inputSchema, outputSchema} + permissions.ui:[surface:open] + permissions.agent:[capability:declare]；(3) 在 JS/Python runtime export 该 function（走 JSON-RPC，AGENTS.md:40）；(4) UI 入口用 slab-plugin-sdk 的 createSlabPluginSdk + mountPluginUI（src/index.ts:262/337）监听 host 注入的 a2u payload。需补：a2u 模板、端到端调试反馈（capability 是否被 host 注册、agent 是否可见该工具）、一个最小示例（如「在对话中打开图片标注插件面、回传标注坐标给 agent」）。 |
| P1: a2u 工具集刻意小而高阶、命名空间化（Anthropic ACI 原则），拒绝低阶原语 | P1 | S | 痛点2；边界红线（不做完整 a2u） | Anthropic《writing-tools-for-agents》明确：工具应高阶、命名空间化、合并链式多步，否则 agent 在工具间迷失、token 浪费。对 a2u：不要为每个插件页暴露 open_tab/select_panel/scroll/click_xy，而应做 plugin.open(plugin_id, surface, payload) 一个高阶工具（surface 内部封装导航+渲染+聚焦）。工具名按域前缀（plugin./workspace./image.）与 mcp.rs:159 的 mcp__ 前缀一致。VS Code 128 工具上限是警示，Slab 应刻意保持 a2u 工具集小（个位数高阶工具）。这也守住「不做完整 a2u / 不做 Computer Use」红线——副作用域严格限定在「打开受信 host UI 面 / 读写 sandbox 文件 / 调 /v1 API」，绝不操控任意像素。 |
| P1: a2u surface 声明必须带 effects（副作用声明）+ 只读默认，对接结果校验/兜底 | P1 | M | 越权审批；痛点4（结果校验/兜底） | PluginAgentCapabilityContribution 已有 effects: Vec<String> 字段（plugin.rs:290）但当前未被消费。a2u surface 必须强制声明 effects（如 readonly / mutates_workspace / network / spawns_process），host 据此决定 allow/ask/sandbox 分级，并对接会议另一条「结果校验」诉求：对 mutates_workspace 类 surface，完成后跑确定性校验（diff/lint，复用 apply_patch 后 guardrail 思路），校验结果作为 surface outputSchema 回灌 agent。同时把 agent.open 失败/超时纳入现有 invalid_tool_call_retries 预算（共享上下文已确认有此预算），不新建重试体系。 |
| P2: 插件 LSP 与 agent 代码智能的关系明确化（避免双 LSP bridge 红线） | P2 | S | 插件 LSP 与 agent 代码智能关系；边界红线 | 插件 contributes.languageServers（plugin.rs:346-352，NodePackage transport 最适合包 VS Code 扩展 LSP）与 agent 的代码智能（shell/read_file/grep/apply_patch 工具）必须明确分工，不得重叠：插件 LSP 走唯一路径 packages/slab-desktop -> /v1/workspace/lsp/* -> app-core（AGENTS.md:43-44，桌面 host 不新增第二套 LSP bridge）；agent 不直接调 LSP，而是通过其确定性工具（grep/apply_patch）+ 未来 task.complete 校验消费 LSP 诊断。插件 LSP 是 workspace 代码智能底座，agent 是编排者，两者通过 workspace 文件系统 + LSP 诊断结果解耦，不互相侵入。这是给插件开发者的边界澄清，避免有人写「插件 LSP 直接喂 agent」绕过红线。 |
| P2: 插件 a2u 的 eval 套件（用真实多轮任务反推工具描述/capability 设计） | P2 | M | DevX；插件生态质量 | Anthropic ACI 强调 eval 驱动 tool 描述的 prompt-engineering，小改描述显著降错。为每个 a2u surface 配真实多轮任务 eval（如「让 agent 打开图片标注插件、回传坐标、继续处理」），测 inputSchema/description 命中率，用结果反推 capability 描述。这是生态成熟度的可演进项，前期可只对内置示例插件做。 |

### 补充痛点（用户未提）
- 对称性缺口被忽略：会议主要谈 agent->插件 a2u，但插件->agent 的反向通道（插件主动请求 agent 帮忙，如插件里点「用助手解释这段」）同样缺失。现有 useAssistantDraftStore 支持跨页跳转（共享上下文 4.3），但插件 WebView 内无法结构化地把上下文喂给 agent。应统一为双向 a2u 契约。
- 工具爆炸风险被低估：每个插件可声明多个 capability + a2u surface，N 个插件会让 agent 工具表膨胀。VS Code 用 128 上限 + virtual tools 阈值收窄。Slab 需要 capability 的「按需启用」机制（VS Code tools picker 范式）而非全量常驻，否则违背 ACI「贵精不贵多」。
- 插件 WebView origin/宿主敏感性（Claude Artifacts iOS 渲染失败教训）：a2u surface 渲染必须遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱，不能用任意 origin inline 内容；插件 uiUrl 的 iframe sandbox（plugin-webview-page.tsx:113 已用 allow-scripts allow-forms）需与 a2u payload 注入的信任级别匹配。
- a2u payload 回灌的 token 成本：surface outputSchema 回灌 agent 时若返回原始大对象会吃 token（Anthropic ACI 强调高信号低 token）。需要 host 侧标准化「capability output 摘要化」契约，让插件返回结构化引用（路径/id）而非原始 blob。
- 插件开发者调试反馈黑盒：registry 把 invalid 插件只记 error 字符串（registry.rs:98-114），capability 是否成功注册成 agent tool、agent 是否真的看到该工具，对开发者不可观测。需要 dev 模式下暴露 capability 注册状态 + agent 工具表快照。
- 插件版本/capability 变更与已运行 agent 的兼容：agent turn 进行中插件 capability 热更新（refresh，registry.rs:42）会导致工具表漂移，需明确「turn 内工具表快照」语义，避免 mid-turn 工具消失。

### Must-have
- a2u 四段闭环必须闭合且可测：plugin 声明（manifest kind=a2u_surface + input/outputSchema + effects）-> agent 调用（host 注册的高阶 plugin.open 工具，走 turn_tool_call.rs 审批门）-> 前端渲染（use-assistant-agent.ts tool_call 分支的 host 受控派发表，payload 从 host 注入）-> 结果回灌（surface output 经 host 转 ToolOutput 喂回 agent turn）
- caller-id 推导红线不可破（AGENTS.md:42）：a2u 全链路中插件 WebView 的 caller 身份只从 WebView label 推导，payload 仅作业务数据，任何身份判定不得读 plugin-supplied 字段
- 不破坏现有 exposeAsMcpTool / agentCapabilities / agentHooks / mcp_call/mcp_proxy：pluginCall capability 的 host 注册是新增路径，与 mcpTool:expose 走的 mcp.rs 路径互斥不重复；agentHooks 仍走生命周期分发
- a2u 工具副作用域严格限定：打开受信 host UI 面 / 读写 sandbox 文件 / 调 /v1 API，绝不操控任意像素（不做 Computer Use）、不访问未授权资源、不让插件自由生成任意 HTML/JS 渲染
- 审批分级复用现有两层（registry 静态 permission 校验 + turn_tool_call.rs ApprovalPort 动态门），a2u 只读/渲染类默认 allow、改动类 ask、危险类拒/拒，不新建第三套权限体系
- To-Be 新增文件边界归属：PluginCapabilityKind 新增 a2u_surface 变体落 crates/slab-types/src/plugin.rs（确定性数据结构，符合归属）；plugin.open ToolHandler 落 host/app-core 层（非 slab-agent-tools，守住纯编排红线）；前端派发表落 packages/slab-desktop/src/pages/assistant（host 固定受信组件）；a2u 模板落 packages/slab-plugin-cli

### 风险
- kind="a2u_surface" 新增枚举值会改 PluginCapabilityKind（plugin.rs:392-395）-> 触发 bun run gen:plugin-packs + gen:schemas（AGENTS.md 红线），所有现存 plugin.json 的 capability 字段需向后兼容（serde default 已覆盖大部分，但 kind 是必填需验证）
- pluginMountView 扩 initialPayload 若从 plugin-supplied payload 取身份会直接违反 AGENTS.md:42 caller-id 红线——payload 内容可来自 agent，但 caller 身份必须从 WebView label 推导，绝不可混淆
- host 注册 pluginCall capability 成 ToolHandler 若误放进 crates/slab-agent 或 crates/slab-agent-tools，违反「slab-agent 纯编排，插件能力由 host/app-core 注册」红线（AGENTS.md:29）——必须落在 bin/slab-app/slab-app-core 层
- a2u surface 渲染插件 WebView 时，若让插件自由返回任意 HTML/JS 让 host 渲染，等于 Computer Use 变体，绕过 Tauri CSP——必须 host 固定受信组件（PluginWebviewPage 已是固定组件，但 payload 注入点要审计）
- 工具膨胀：若每个插件 capability 都常驻 agent 工具表，N 插件 × M capability 会超 LLM 工具数合理上限（Anthropic/VS Code 经验是几十量级），需配按需启用机制否则 agent 决策质量下降
- 审批疲劳：a2u 若每个 surface 都 ask，会像 Cursor YOLO 前的体验一样弹窗不断；但全 allow 又违反本地优先信任模型——分级必须精准，否则两头不讨好

### 边界红线注意
- AGENTS.md:42 红线——Plugin WebView commands must derive the caller plugin id from the WebView label, not plugin-supplied payload fields。a2u 的 plugin.open payload 注入点必须审计：业务 payload 可来自 agent/插件，但 caller 身份判定只读 label。这是 a2u 设计的最高风险点，任何「为了方便从 payload 取 pluginId 做路由」的捷径都违规
- AGENTS.md:29 红线——slab-agent 保持纯编排。pluginCall capability -> ToolHandler 的注册必须在 host/app-core（bin/slab-app/slab-app-core），绝不进 crates/slab-agent 或 crates/slab-agent-tools。McpProxyTool 在 slab-agent-tools 是因为 MCP 是确定性协议适配，插件 capability 注册涉及插件生命周期/sandbox 属 host 职责，归属不同
- AGENTS.md:38 红线——Keep Tauri child WebViews as the default third-party plugin UI runtime. Do not make Module Federation the default plugin model。a2u surface 渲染必须继续用 PluginWebviewPage（Tauri child WebView / iframe sandbox），不可借 a2u 之名引入 Module Federation 动态加载
- AGENTS.md:43-44 红线——桌面 host 不新增第二套 LSP bridge。插件 LSP（languageServers contribution）只走 /v1/workspace/lsp/* -> app-core，agent 不直连插件 LSP，避免「插件 LSP 喂 agent」绕红线
- AGENTS.md:39 红线——plugin.json 是静态 source of truth。a2u surface 声明必须在 plugin.json contributes 里静态声明（含 input/outputSchema/effects），不允许运行时动态注册 surface（防越权面逃逸）
- AGENTS.md:90 / schema 红线——PluginCapabilityKind 加 a2u_surface、PluginCapabilityTransportType 若扩、permissions.ui 加 surface:open 都会改 plugin.json shape，必须 bun run gen:plugin-packs + gen:schemas，且 manifestVersion 是否需 bump 需评估（当前 manifestVersion:1，加可选字段可不 bump，但加必填 kind 变体需谨慎向后兼容）
- AGENTS.md:36-37 红线——插件 dispatch 走 bin/slab-app -> bin/slab-server /v1/plugins/rpc (WebSocket JSON-RPC 2.0) -> app-core，JS 走 slab-js-runtime、Python 走 slab-python-runtime。a2u 的 pluginCall capability 执行必须沿用此路径，不可为 a2u 开新 dispatch 通道

### 开放问题
- plugin.open 是单一高阶工具（agent 传 plugin_id+surface+payload）还是每个 a2u surface 注册成独立命名工具（plugin__id__surface）？前者工具表小但 agent 需先 list 可用 surface（多一轮），后者工具表膨胀但 ACI 更友好——需结合「按需启用」机制定。
- a2u surface 的 outputSchema 回灌是同步（surface 执行完阻塞 turn 回传，像普通 tool）还是异步（surface 长任务 background，像 VS Code Continue in Background）？影响 turn.rs 的等待语义，需与会议 agent 增强议题对齐。
- 插件 capability 注册成 agent tool 后，subagent（subagent.rs 的 allowed_tools 白名单）是否继承？a2u surface 在 subagent 隔离上下文里打开 UI 面是否合理（subagent 是 researchers not coders，可能不该有 UI 副作用）——需明确 a2u 工具默认不进 subagent 白名单。
- 插件热更新（registry.refresh）与正在进行的 agent turn 的工具表快照语义如何定义？是 turn 开始时快照、turn 内不变，还是实时生效？影响插件开发者调试预期。
- a2u payload 从 host 注入 vs 从 agent tool 参数注入的边界：payload 业务内容可来自 agent（如 agent 决定打开哪个文件的标注面），但 host 是否需要对 payload 做二次校验/裁剪（防 agent 被诱导传越权 payload 给恶意插件 surface）？
- 反向 a2u（插件主动请求 agent）是否纳入本期范围，还是只做 agent->插件单向？若双向，插件 WebView 内触发 agent 的契约（复用 useAssistantDraftStore 还是新走 /v1/agents/responses）需定。

---

## 前端 / UX 架构师（统一入口 shell、多窗口/分屏/tab、a2u 渲染管线、设计系统）

### 核心关切
- 单窗口无多窗口基建：routes/index.tsx、layouts/index.tsx、layouts/sidebar.tsx 是单 <Outlet/> 路由 + 52px rail 导航，window-controls.tsx 只用 getCurrentWindow() 做 minimize/maximize/close，没有任何 WebviewWindow/createWindow 体系。用户要的『原页面退化为子窗口/新 tab』需要决定 Tauri WebviewWindow vs 主窗内 tab/分屏 vs 侧抽屉的取舍，这是整套入口体验的基石。
- Assistant 已是 index route 但能力分散：Assistant 已是 / 首页（routes/index.tsx:72）且经 useAssistantDraftStore 支持跨页跳转，但 Image/Video/Audio/Hub/Workspace/Plugins 仍是同级 rail 项，用户心智是『点 sidebar 切页面』而非『对 AI 说话完成任务』。把 Assistant 升级为真正的统一入口 shell 而不推翻现有页面，是核心工程命题。
- a2u 渲染管线缺失派发表：use-assistant-agent.ts 的 handleAgentEvent（L458-569）把 tool_call_started/tool_call_output 全部折叠进 ThoughtChain（assistant-bubble-content.tsx 用 @ant-design/x 的 ThoughtChain）。没有『tool 名 → 受控 React 面/动作』的派发表，agent 无法通过结构化 tool call 打开 workspace/image/plugin/review 面。这是 a2u 的最小可用形态。
- 失效跨页路径：use-workspace-page.ts:683 的 handleExplainWithAssistant 跳 navigate('/assistant')，但 routes/index.tsx:73 已把 /assistant 改为 Navigate to='/' replace。这是一条真实失效的入口跳转，说明『入口升级』必须先收敛所有跨页契约。
- 任务总结 action 卡片不存在：北极星要『任务总结处给打开项目/人工审阅/feedback 调整三个按钮』，但现有 AssistantSessionSummaryCard 只是 session 切换器，turn_completed 事件（use-assistant-agent.ts:540）只写文本，没有 artifact 引用与可执行 action 的卡片层。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| P0：确认『主窗内 surface 状态机』为最小改造路径，而非 Tauri WebviewWindow | P0 | M | 痛点1（统一入口缺失）+ 痛点2（插件 a2u 打开新页面） | 现状 routes/index.tsx 是单 <Outlet/>，window-controls.tsx 仅 minimize/maximize/close，零多窗口基建。直接上 Tauri WebviewWindow 会触发：每窗独立 WebView 上下文丢失 Zustand/TanStack Query 共享、CSP/capabilities 红线、与现有 plugin-host-bridge 的 pluginMountView bounds 契约冲突（plugin-webview-page.tsx 依赖主窗 DOM rect）、IPC 序列化成本。最小改造是『不破单窗』——把 Layout 的 <Outlet/> 升级为 surface 状态机：主对话流常驻，副 surface（workspace/editor/diff/image-preview/plugin）以『分屏/浮窗/内联卡片』形态叠加，复用现有组件。Tauri WebviewWindow 仅作为 P1『离窗化』的可选增强（把当前主窗 surface 弹出为独立 OS 窗），届时再处理 capabilities。 |
| P0：把 Assistant 升级为 AgentShell——Assistant 常驻主区，原页面退化为 surface（不推翻路由） | P0 | L | 痛点1（统一入口）+ 痛点2（插件 a2u） | Assistant 已是 index route（routes/index.tsx:72）+ useAssistantDraftStore 跨页（use-workspace-page.ts:683）。升级而非推翻：(1) 新增 packages/slab-desktop/src/shell/agent-shell.tsx 作为 / 的 layout，左侧常驻 Assistant 对话流（只读快照态），右侧动态装载 surface；(2) 原 routes 的 image/video/workspace 等保留，但从『sidebar 顶层导航项』降级为『可被 a2u 打开的 surface 类型』；(3) sidebar rail 不删，但语义从『切页面』改为『打开 surface』。这样 useAssistantDraftStore 既有契约保留，只把 navigate(target) 统一改成 shell.openSurface(surface, payload)。复用 WorkspaceStage（layouts/index.tsx:23）作为 surface 容器。 |
| P0：建 a2u 渲染派发表与四态状态机（tool 名 → host 固定 surface/动作） | P0 | L | 痛点2（插件 a2u 打开）+ 痛点1（统一入口） | 现状 use-assistant-agent.ts 的 handleAgentEvent 把所有 tool_call 统一折叠成 ThoughtChain 节点，无派发。对齐 Vercel Generative UI / Claude Artifacts 范式（模型只决定调哪个工具+参数，渲染哪个组件 host 固定）。新增 packages/slab-desktop/src/lib/a2u-dispatcher.ts：定义 tool 名 → { surface, mountMode: 'split'/'float'/'inline-card', payloadAdapter } 的派发表，命名空间化（workspace.open / image.edit / plugin.launch / review.show / hub.open）。在 tool_call_started/tool_call_output 事件分支里调用 dispatcher，对『受控 UI 工具』触发 shell.openSurface 而非仅入 ThoughtChain。副作用域限定『打开受信 host 面/读写 sandbox 文件/调 /v1 API』，绝不暴露像素操控。 |
| P0：实现任务总结 action 卡片（turn_completed artifact_refs → 三按钮） | P0 | M | 北极星『任务总结三按钮』+ 痛点1 | 北极星明确要『任务总结处给打开项目/人工审阅/feedback 调整』。现状 turn_completed（use-assistant-agent.ts:540）只写文本。新增 packages/slab-desktop/src/pages/assistant/components/agent-action-card.tsx：当 turn_completed 携带 artifact_refs（文件路径/资源引用）时，在末条 assistant 气泡后渲染卡片，三按钮分别映射 a2u 动作——『打开项目』→ shell.openSurface('workspace', {revealPath})；『人工审阅』→ shell.openSurface('review', {diff})；『feedback 调整』→ composer 注入草稿（复用 useAssistantDraftStore.setDraft）不重启线程。需 /v1/agents/responses 的 turn_completed 事件扩 artifact_refs 字段（后端 turn.rs 已结构化判停，加 task.complete 工具产出引用即可，详见编排 agent 议题）。前端 shape 变动跑 gen:api。 |
| P0：修复 /assistant 跨页跳转失效路径，统一所有跨页契约为 AgentSurfaceStore | P0 | M | 入口契约漂移（隐性 bug）+ 痛点1 | use-workspace-page.ts:683 navigate('/assistant') 已失效（routes/index.tsx:73 是 Navigate to='/' replace），说明跨页契约已漂移。统一收敛：废弃 useAssistantDraftStore 的『navigate + draft』双步模式，改为单一 packages/slab-desktop/src/store/useAgentSurfaceStore.ts，承载 { draftPrompt, pendingSurface, surfacePayload }，所有页面的『用助手解释/打开并审阅』入口只 set pendingSurface，AgentShell 监听并自动 openSurface。彻底消灭失效路径与 location.state 散用（use-workspace-page.ts:615 用 location.state.workspaceRevealPath 是另一处散契约，一并收敛）。 |
| P1：workspace 项目切换器上提到 shell header，AgentShell 感知当前 project context | P1 | M | 痛点3（workspace 项目组织）+ 痛点1 | use-workspace-page.ts:83 的 recentWorkspaces 持久在 useWorkspaceUiStore 但 Assistant 不感知。AgentShell header 新增 project switcher（下拉：recent workspaces + open folder），切换时调 workspace-bridge.workspaceOpen 并把 rootPath 写入 AgentSurfaceStore.currentProjectContext。a2u 工具 workspace.open 默认绑定当前 context，agent 跑编码/文案/媒体任务时『针对哪个 workspace』对用户可见。复用现有 workspaceState()（use-workspace-page.ts:75），不新增 API。 |
| P1：Agent 执行时间线组件（plan 节点 + subagent 层级 + checkpoint） | P1 | L | 痛点4（多轮任务支撑）+ additional：缺失时间线状态层 | 现状 thoughts 是扁平 tool 流（use-assistant-agent.ts:113）。编排 agent 议题要把 plan.rs 升级为带依赖边的 DAG + task.complete。前端配套新增 packages/slab-desktop/src/pages/assistant/components/agent-timeline.tsx：订阅 plan_update 工具的结构化输出（task_id/status/depends_on）渲染节点图，subagent 的 delegate_subagent 调用渲染为可折叠子时间线，checkpoint/replan 事件标红。与 plan.rs（crates/slab-agent-tools）契约对齐：plan_update 的 output shape 变化需 gen:api。这是『多轮任务支撑』在 UX 侧的可见化，避免用户面对扁平 tool 流失去全局感。 |
| P1：审批/中断/恢复 UI——审批门排队 + 终止理由结构化 + checkpoint 续跑 | P1 | M | 痛点4（兜底）+ additional：审批/中断/恢复 UI 不完整 | 现状 pendingApprovals（use-assistant-agent.ts:114）是平面 Map，abort（L1166）只设 interrupted。补：(1) 审批门列表组件，多工具并发审批时排队展示（对齐 turn_tool_call.rs:418 阻塞模型）；(2) 终止理由结构化——订阅后端 turn_failed/turn_cancelled 的 reason（completed/max_turns/repetition_detected/budget_exhausted/interrupted，见编排 agent 议题的循环检测），AgentShell 渲染『为什么停』banner；(3) checkpoint 续跑按钮——命中 max_turns/repetition 时走 interrupt（control.rs:381 已解耦，保留线程可续），前端提供『调整后继续』入口而非硬终止。 |
| P1：tool call 默认折叠 + 关键节点置顶（VS Code collapsedTools 范式） | P1 | S | additional：tool call 默认全展开污染对话流 | 现状 ThoughtChain 全展（assistant-bubble-content.tsx），多轮任务气泡极长。对齐 VS Code chat.agent.thinking.collapsedTools：tool call 默认折叠为一行摘要（tool 名 + 关键参数），点击展开。仅『plan/approval/artifact』类关键节点置顶常显。在 a2u-dispatcher 里标记 tool 的 importance，决定折叠策略。纯前端改造，无 API 变动。 |
| P2：Tauri WebviewWindow 离窗化（surface 弹出为独立 OS 窗），含 capabilities 审计 | P2 | XL | 痛点1（原页面退化为子窗口）的长尾形态 | P0 的主窗内 surface 状态机满足『统一入口』，但用户场景可能需『把 workspace 拖到第二屏』『媒体预览独立窗』。P2 在 AgentSurfaceStore 增 mountMode: 'popout'，调用 @tauri-apps/api/window 的 WebviewWindow 创建独立窗，capability 走『surface-window-<surfaceId>』label 推导（延续 plugin caller-id-from-label 红线）。代价高（CSP/capabilities/IPC/状态同步），仅在 P0 验证后按需推进，不作为默认。 |
| P2：StateSurface 设计系统统一『空/加载/错误/成功/中断』渲染契约 | P2 | M | additional：设计系统对齐缺口 + 无障碍一等功能化 | layouts/index.tsx 的 isChatShell ? 'chat' : 'default' 二态已散落 Header/Sidebar/FooterStatusBar，a2u 新增内联卡片/浮窗/分屏会继续裂变变体。新增 packages/slab-components 的 StateSurface 组件，统一 agent 状态→视觉映射（empty/loading/error/success/interrupted/budget-warning），a2u 卡片、surface 容器、时间线节点统一消费。无障碍：StateSurface 内置 aria-live 播报 agent 状态变化。纯设计系统收敛，渐进迁移。 |

### 补充痛点（用户未提）
- 缺失统一的『agent 执行时间线』状态层：thoughts 数组（use-assistant-agent.ts:113）是扁平的 tool call 流，没有 plan/DAG 节点视图、没有 subagent 层级、没有 checkpoint 标记。多轮长任务时用户无法回答『现在到哪了/卡在哪/还要多久』。
- 审批/中断/恢复 UI 不完整：pendingApprovals 是平面 Map（L114），abort 仅设 interrupted 状态（L1166-1184）。没有『审批门排队』、『中断后从 checkpoint 续跑』、『为什么停（completed/max_turns/repetition/interrupted）』的可视化。control.rs:381 的 interrupt/shutdown 已解耦但前端没暴露终止理由结构。
- 设计系统对齐缺口：layouts/index.tsx 靠 isChatShell ? 'chat' : 'default' 二态变体散落各处（Header/Sidebar/FooterStatusBar），a2u 新增『内联卡片/浮窗/分屏』会继续裂变变体。缺一个 StateSurface 统一『空态/加载/错误/成功/被中断』的渲染契约，长期会变成 if-else 堆叠。
- 无障碍(a11y)未被一等功能化：sidebar 用 focus-ring（sidebar.tsx:94）但 ThoughtChain/卡片/审批按钮的键盘可达性、焦点陷阱、屏幕阅读器播报 agent 状态无统一规范。多窗口/浮窗引入后焦点管理风险更高。
- workspace 切换器与项目上下文割裂：use-workspace-page.ts 的 recentWorkspaces 持久在 useWorkspaceUiStore，但 Assistant 不知道当前在哪个 project；agent 跑任务时『针对哪个 workspace』不可见。统一入口必须让 project context 成为 shell 一等状态。
- 插件 a2u 入口与现有 WebView mount 契约未对齐：plugin-webview-page.tsx 是整页 pluginMountView（占满 Outlet），插件要『a2u 打开』需要『浮窗/分屏/内联卡片』三种挂载模式，但现契约只有占满 Outlet 一种 bounds，caller id 从 label 推导的红线在浮窗模式下需要新 label 方案。
- tool call 默认全展开污染对话流：现有 ThoughtChain 在气泡内全列 tool call（assistant-bubble-content.tsx），多轮任务下气泡会极长。需『默认折叠细节、点击展开』+ 关键节点（计划/审批/产物）才置顶展示。

### Must-have
- Assistant(/) 保持为统一入口 shell，且不推翻 image/audio/video/hub/workspace/plugins/task/settings 任一现有页面——新增组件层与派发表，路由层零破坏。
- a2u 渲染派发表与状态机：tool 名 → 受控 surface（浮窗/分屏/内联卡片）四态明确，agent 通过结构化 tool call 驱动，host 固定渲染、模型只决定『调哪个工具+参数』。
- 任务总结 action 卡片：turn_completed（带 artifact_refs）→『打开项目/人工审阅/feedback 调整』三按钮，每个按钮映射一个受控动作/逆 tool call。
- 修复失效跨页路径（/assistant → /）与统一跨页契约为 AgentSurfaceStore，消灭分散的 location.state/draft store。
- 多窗口/tab 方案明确写为『主窗内 surface 状态机优先（P0）+ Tauri WebviewWindow 离窗可选（P1，带 capabilities 审计）』，不在 P0 引入多窗口基建风险。
- 设计系统对齐：StateSurface（空/加载/错误/成功/中断/中断理由）+ tool call 折叠规范 + 焦点/无障碍契约，作为 a2u 一等公民而非事后补丁。
- 边界守护：所有新增落在 packages/slab-desktop / bin/slab-app/src-tauri host 层；/v1/agents/responses 仅扩 fields 跑 gen:api；插件浮窗 caller id 从 label 推导不破例；不引入 Computer Use 变体（模型不生成任意 UI）。

### 风险
- 路由层零破坏的承诺可能与 a2u 派发表产生张力：若 dispatcher 要在 tool_call 阶段打开 surface，但 surface 组件依赖各自页面的 lazy-load（routes/index.tsx:22 WorkspacePage lazy），首次打开会有可感延迟；需 preload 策略且不能把所有页面打进主 bundle（已有 check:bundle-budget 红线）。
- AgentSurfaceStore 统一契约后，多 surface 并存（分屏 + 浮窗 + 卡片）的焦点/键盘/aria 管理复杂度上升；若不引入焦点陷阱与 Escape 收敛规则，无障碍会退化，违背『一等功能化』。
- a2u 派发表是前端固定映射，但工具集会随插件 contributes.agentCapabilities 动态增长（registry.rs:498 校验 exposeAsMcpTool）。若派发表只认内置工具名，插件 a2u 工具会落到『未知工具』兜底；需让插件通过 contributes 声明其 surface 渲染器，但这与『host 固定渲染面』红线有张力——必须约束插件只能声明『指向 host 已有 surface 类型』而非自带渲染器。
- task.complete / artifact_refs 跨前后端：turn_completed 扩字段需 gen:api，且 artifact_refs 的文件路径/workspace 引用必须做权限校验（不能让 agent 产出指向 workspace 之外的危险引用被 action 卡片直接打开），否则绕过 sandbox。
- project switcher 与 workspace sidecar 的生命周期耦合：切换 project 触发 sidecar 以新 --settings-path/--database-url 重启（workspace-mode-design.md），若 AgentShell 在切换瞬间有正在跑的 agent thread，thread 的 session 归属会错乱；需明确『切 project 前先 interrupt 或绑定 session 到 project』。
- Tauri WebviewWindow P2 方案的 capabilities 膨胀：每个离窗 surface 若都要独立 capability，capabilities 文件会爆炸；需用 label 前缀通配而非逐个声明，但这又与最小权限原则拉扯，需安全评审。

### 边界红线注意
- 统一入口/多窗口/tab 全部落在 packages/slab-desktop 与 bin/slab-app/src-tauri（host 层），不得为它新建 API 树、不得让 slab-app-core 感知多窗口（HTTP-free 红线）。多窗口编排属 Tauri command（host-only，AGENTS.md L24）。
- Tauri WebviewWindow 多窗口方案必须配套声明 CSP/capabilities/permissions（AGENTS.md L31『Preserve Tauri CSP... unless the task explicitly changes them』）——本会建议的多窗口是『explicitly changes』，需在提案里显式说明并走 capabilities 审计，不能默认开放。
- 插件浮窗 label 必须由 host 推导（AGENTS.md L42『Plugin WebView commands must derive the caller plugin id from the WebView label』）。浮窗挂载若引入新 WebView，其 label 命名规则必须延续『从 label 推导 plugin id』，不得让 plugin payload 字段决定 caller——浮窗 a2u 插件挂载要新增 label 规范（如 surface-window-pluginId-surfaceId）。
- a2u 受控 UI 工具集与动作卡片的『动作』必须走 host 已有导航/命令（navigate / workspace-bridge / plugin-host-bridge），不得让模型自由生成任意 HTML/JS 渲染面（= Computer Use 变体，违反『不做完整 a2u』红线），渲染面必须是 host 固定的受信组件。
- 新建文件显式标注与边界归属：lib/a2u-dispatcher.ts（纯前端派发表，packages/slab-desktop，无需破例）、components/agent-action-card.tsx（纯前端卡片，同上）、shell/agent-shell.tsx（纯前端 shell 重排，同上）；要破例/需注意的：windows/agent-surface-window.ts（涉及 Tauri capabilities，属 host 层 bin/slab-app/src-tauri，需走 CSP/capabilities 审计流程，不破例但需显式声明）。后端 /v1/agents/responses 仅扩 fields 不新增 API 树（gen:api 必跑）。
- workspace project context 读取走现有 workspace-bridge（/v1/workspace），不得让 shell 直连 sidecar 或新增第二套 workspace 状态 API。
- 不建议把 sidebar 52px rail 魔改成复杂树/面板——rail 形态是设计系统稳定锚，a2u 应在主区呈现 surface，rail 仅作快速切换，避免推翻 layouts/sidebar.tsx 既定视觉契约。

### 开放问题
- 多窗口/tab 的 P1 选型：Tauri WebviewWindow 的 capabilities 审计成本与主窗内 surface 状态机的可接受上限之间，产品想推到哪一档？是否接受『同一时间最多 1 个主窗 surface + N 个浮窗』的硬约束来换零多窗口风险？
- 插件 a2u 浮窗的 caller-id-from-label 红线下，浮窗 WebView 的 label 命名规则如何定（surface-window-pluginId-surfaceId？）？需与 crates/slab-plugin/registry.rs 的权限校验协同，是否要扩 contributes.agentCapabilities 增加 surface 挂载模式声明（route/sidePanel/inlineCard/popoutWindow）并走 gen:schemas？
- 任务总结 action 卡片的 artifact_refs：是只回传文件路径（确定性），还是允许回传结构化产物（图像/diff/报告）的引用？前者简单但表达力弱，后者需 /v1/agents/responses 扩一个 artifact 引用 schema 并定义 host 渲染派发——范围要锁在哪一档？
- Agent 执行时间线与 plan.rs DAG 的契约：plan.rs 升级为 DAG（已在编排 agent 议题内）后，前端 plan 视图是订阅 tool_call_output 实时渲染，还是新增 /v1/agents/responses 的 plan 快照事件？后者更稳但要多一个事件 shape（gen:api）。
- 统一入口 shell 是否需要把 image/video/audio 三个媒体页合并成一个『媒体工作台』surface（agent 按 output_kind 派发），还是保留三页只让 agent 能 a2u 打开它们？前者更符合北极星『一个简单入口』，但推翻面更大——P0 保守 vs P2 激进的取舍需产品拍板。
- 审批/恢复 UI 的『终止理由结构』：control.rs:381 已解耦 interrupt/shutdown，但要暴露 completed/max_turns/repetition/interrupted 给前端，/v1/agents/responses 的 turn_finished/turn_failed 事件是否扩 reason enum？这跨前端+后端，需与编排 agent 议题对齐。

---

## 后端工程师 (Backend Engineer) — Agent 编排增强 / 结构化终止 / subagent 隔离 / 循环检测 / workspace-scoped context / 契约对齐

### 核心关切
- 边界红线 #1：DAG/规划/校验/task.complete 这类确定性逻辑必须落 slab-agent-tools，绝不能塞进 slab-agent 编排核心。slab-agent 的 thread.rs:248 for-turn 循环与 turn.rs:198 tool_calls.is_empty() 判停要作为纯编排契约保持不动，新能力靠工具 + hook + app-core 注册三件套挂载，避免污染红线。
- 结构化终止纪律不可倒退：当前 turn.rs:198 已经是结构化判停（response.tool_calls.is_empty() => TurnOutcome::Final），用户担心的文本判停在代码里根本不存在。强化方向是加显式 task.complete 工具表达意图，绝不引入关键词/正则匹配判停。
- ToolContext 注入点过窄：crates/slab-agent/src/tool.rs:17-24 的 ToolContext 只有 thread_id/turn_index/depth 三字段，没有 workspace/session/memory 句柄——这是 workspace-scoped agent context、plan 持久化、循环检测签名计算共同卡住的瓶颈，需一次结构性扩容。
- 循环/重复检测完全缺失：thread.rs:248 只有 max_turns 硬上限 + invalid_tool_call_retries 预算，control.rs:381 的 interrupt 与 shutdown 已解耦但没人用 interrupt 做 soft-stop。命中上限直接 break 会让长任务前功尽弃，需要带 reason 的终止 + soft-stop + 续跑。
- plan 状态不持久化、无 DAG、不回填结果：plan.rs 当前只把计划 normalize 回显给 LLM（plan.rs:91-106 的 execute 只 to_string 返回），200K 截断就会丢计划，也无 mark_done/replan/校验接口——这是 plan-and-execute 的硬缺口。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| P0 — 扩容 ToolContext，注入 workspace/session/记忆/计划句柄（结构性前置） | P0 | M | 痛点4（Agent 增强：workspace-scoped agent context，项目级 settings 合并、session/memory 隔离） | crates/slab-agent/src/tool.rs:17-24 的 ToolContext 仅 thread_id/turn_index/depth，是 workspace-scoped context、plan 持久化、循环签名计算共同卡点。这是纯数据结构扩容（不增加编排逻辑），由 turn_tool_call.rs:39 构造点（唯一构造处）统一注入，符合 slab-agent 只持有 port 句柄、不持有业务逻辑的纯编排边界。注入内容是 trait object（如 WorkspaceScope:提供 workspace_root/已合并 settings 的只读视图），确定性能力本体仍在 slab-agent-tools。 |
| P0 — task.complete 结构化完成判定工具（落 slab-agent-tools，app-core 注册） | P0 | M | 痛点4（结构化 tool 判停，禁止文本判停）+ 痛点1（任务总结处给 open_project/request_review/feedback 三个动作） | 保持并强化 turn.rs:198 的结构化判停语义：新增确定性工具 task.complete(summary, artifact_refs, verification_summary)，由 LLM 在所有 plan 节点 mark_done 后显式调用，作为另一种合法的 TurnOutcome::Final 触发路径（仍走 tool_calls 通道，不改 turn.rs:198 判停条件本体）。坚决不引入文本/正则判停——这恰是用户最担心的点，代码里不存在，保持住。task.complete 属确定性工具，归 slab-agent-tools（与 plan.rs 同 crate），由 crates/slab-app-core/src/infra/agent/runtime.rs 注册，符合边界。 |
| P0 — 把 plan 升级为带依赖边的 DAG + mark_done/replan 接口 + 持久化进 slab-agent-memories | P0 | L | 痛点4（主 agent 全局规划、DAG、恢复） | plan.rs 当前只 normalize 回显（plan.rs:91-106），无依赖、无状态机、无持久化。借鉴 Anthropic 多 agent 系统 save plan to Memory 防 200K 截断教训：plan 节点加 task_id/depends_on/status/result_ref 字段，新增 mark_done(task_id, result_ref) 与 replan(plan_patch) 工具；plan 数据通过 slab-agent-memories 持久化（Slab 已有该 crate 且 runtime.rs:26 已加载 memory 配置），每轮开局从 memory 重载计划。DAG/规划是确定性数据结构，归 slab-agent-tools，不污染 slab-agent 纯编排；replan 由 LLM 在 turn 内调用触发，主循环 thread.rs:248 无需新增 replan 分支（保持纯 for-turn 编排）。 |
| P0 — 循环/重复检测：近 N 轮工具调用签名去重 + soft-stop 走 interrupt | P0 | L | 痛点4（多轮重复任务检查、全局 max-turn 兜底） | 当前 thread.rs:248 只有 max_turns 硬上限。新增轻量检测：对 (tool_name, 关键 args 哈希) 做滚动窗口签名去重（如近 6 轮内同一签名连续重复 ≥3 次判 stuck），命中后复用已解耦的 control.rs:381 interrupt（soft-stop，保留线程可续跑 + 人工介入）而非 shutdown 硬杀。信号三类：(a)重复 tool+args 签名；(b)工具输出连续返回相同/空结果且 plan 状态不动；(c)同一文件路径在 write_file/apply_patch 中被反复修改（args 内 path 字段去重）。这是确定性逻辑，归 slab-agent-tools 或独立 slab-agent-loopguard（建议先内联在 thread.rs 的 turn 之间，作为编排内置的 guard，因为它读 TurnOutcome——属编排职责，不引入新业务概念）。 |
| P0 — 终止理由结构化（completed/max_turns/repetition/budget/interrupted）写 tracing + session | P0 | S | 痛点4（max-turn 兜底，需可观测） | 把 thread.rs:248 的静默 break 升级为带 reason 的结构化终止，写入 slab-agent-tracing 与 session 状态，便于前端展示为什么停。ADK LoopAgent 显式终止条件思想借鉴到此：终止是一等公民。属编排内置状态（不改业务概念），保留在 slab-agent。 |
| P1 — delegate_subagent 补四要素：objective/output_format/边界/只读工具集预设 | P1 | M | 痛点4（subagent 隔离与上下文边界） | subagent.rs:21 的 DelegateSubagentArgs 已有 task/model/system_prompt/allowed_tools/max_turns，但缺 Anthropic 反复强调的四要素（objective、output_format、工具与来源指引、明确任务边界）。在 prompt 工程层强制 lead agent 给出这些字段（schema 加 output_format 必填），并为只读校验类子任务提供 read-only 工具集预设常量。subagent 已隔离正确（独立 thread_id、全新对话、parent_id 回链、depth 上限 4/并发 32），方向与 Claude Code 一致，无需大改结构。 |
| P1 — 端态 + checkpoint 结果校验：确定性 verify 工具（编译/lint/diff），不用 LLM 自评 | P1 | L | 痛点4（结果校验）+ 痛点3（workspace 编码能力增强） | 借鉴 Anthropic 端态评估优于逐步评估 + 反模式（同模型自评会自信确认错误）。新增确定性校验工具（如 verify.workspace_build / verify.lint / verify.diff_summary），校验结果作为 plan 节点 result_ref。校验工具归 slab-agent-tools，由 app-core 按是否处于 workspace 决定是否注册（runtime.rs 模式）。绝不用主 agent 自我宣布完成代替校验。 |
| P1 — subagent 产物落盘 .slab/ + 只回传路径引用 | P1 | M | 痛点3（workspace 能力）+ 痛点4（subagent 隔离） | 借鉴 Anthropic subagent output to filesystem + A2A artifact 解耦思想（不引入 A2A 协议本身，信任模型不符）。workspace 任务让 subagent 把文件/报告写入 .slab/ 工作目录，只回传路径引用给主 agent，减少 token 与 telephone game。契合 docs/development/workspace-mode-design.md 的 .slab/ 结构。需扩 ToolContext 暴露 workspace_root（见 P0 第一条）。 |
| P1 — 工具审批分级 allow/sandbox/ask（a2u 打开/只读类默认 allow，改动类 ask，危险类 sandbox/拒） | P1 | M | 痛点1（统一入口，AI 在对话中完成任务）+ 痛点2（插件 a2u 打开） | 对齐 turn_tool_call.rs:418 已有 ApprovalPort，借鉴 VS Code/Cursor 三态分类。a2u 类打开/渲染工具（open_project/request_review 等）无副作用默认 allow，减少权限弹窗；write_file/apply_patch/shell default ask；外部网络/危险类 sandbox 或拒。分级策略属确定性，可由 app-core 按工具名静态配置注入 ToolRiskAnalyzer（slab-agent 已有 risk: &dyn ToolRiskAnalyzer port，turn.rs:46）。 |
| P1 — a2u 任务式高阶工具 + 命名空间（workspace.open/image.edit/plugin.launch/review.show），任务总结三动作按钮 | P1 | L | 痛点1（统一入口）+ 痛点2（插件 a2u 打开新页面）+ 痛点3（项目形式组织） | 借鉴 Anthropic ACI（高阶、命名空间化工具）+ ChatGPT Canvas 自动打开专门面。不做低阶 open_tab/select_panel/scroll 原语（会让 agent 迷失、token 浪费）。任务总结处给 open_project/request_review/feedback 三个受控 a2u 工具调用（host 固定派发表，模型只决定调哪个工具+参数）。a2u 工具归 slab-agent-tools，由 app-core/host 注册，绝不触碰任意像素操控（不做完整 Computer Use 红线）。 |
| P2 — 契约对齐自动化门禁：/v1 shape 变 gen:api、schema 变 gen:schemas/gen:plugin-packs 进 CI | P2 | S | 痛点2（插件 contributes.agentCapabilities[].exposeAsMcpTool 契约）+ 跨条目共享 | AGENTS.md 红线要求 API shape 变 -> bun run gen:api；schema 变 -> gen:schemas。把这套契约对齐做成 CI 强制门禁，防止 task.complete/plan DAG/a2u 工具新增时漏跑代码生成导致 packages/api/src/v1.d.ts 与 settings-document.schema.json 漂移。属工程治理，不引入新业务概念。 |
| P2 — 按 query 复杂度缩放 subagent 数的 prompt 规则 + 预算（turn/token/工具调用次数）看板 | P2 | S | 痛点4（兜底） | 借鉴 Anthropic 早期教训（为简单查询 spawn 50 个子 agent 反模式）。在 delegate_subagent 的 system prompt 加按复杂度缩放规则（事实类 1 个、对比类 2-4 个、复杂研究 10+），叠加 token/turn/工具调用次数预算看板（复用 slab-agent-tracing），防过度投资。已有并发上限 32/深度 4 兜底，此为 prompt 层精细化。 |

### 补充痛点（用户未提）
- interrupt 通道建好了却没被循环检测复用：control.rs:381 的 interrupt（soft-stop，保留线程）已解耦，但 thread.rs:248 命中 max_turns 直接 break 走硬终止路径（last_error / interrupted 分支），soft-stop 能力被浪费。循环检测应优先走 interrupt 而非 break，否则长任务前功尽弃（Anthropic：restarts are expensive）。
- subagent.rs:117 是同步阻塞 wait_for_terminal_snapshot：主 agent 在等子 agent 期间完全阻塞，无法并行 spawn 多个 subagent 做广度探索（Anthropic orchestrator-worker 的并行优势发挥不出来）。这是多 agent 烧 15x token 但没拿到并行收益的隐患。
- plan.rs 没有结果回填闭环：plan 节点无 result_ref 字段，工具执行结果无法回填到对应 plan 节点供 replan 决策——这是 plan-and-execute 的 replan 分支缺的数据基础，导致 agent 只能 turn-by-turn 看眼前。
- 工具调用签名去重需要规范化 args：apply_patch/write_file 的 args 里 path 可能用 ./ 或绝对路径或带 .. ，不规范化会导致 (tool, args) 哈希对同一操作产生不同签名，循环检测漏报。需要一个 args normalizer（确定性，归 slab-agent-tools 或 loop-guard）。
- ToolContext 扩容会破坏现有 ToolHandler 实现的构造处：tests.rs:2035/2046/2110 与 subagent.rs 测试都硬编码 ToolContext { thread_id, turn_index, depth }，扩容需提供默认值或 builder，否则一次性改炸所有工具的测试。
- workspace settings 合并的正确性缺测试锚点：workspace-mode-design.md:74 说 sidecar 以 --settings-path <workspace>/.slab/settings.json 启动，但合并语义（workspace 覆盖全局的深度、agent.tools.allowed 的并集/交集）没有显式契约，新增 workspace-scoped agent context 前必须把合并语义钉死，否则主/子 agent 拿到不一致的工具集。
- a2u 工具的副作用域边界没有静态守卫：AGENTS.md 红线说不做完整 Computer Use，但目前没有机制阻止某个插件 contributes 的工具偷偷做像素操控或访问未授权资源。需要给 a2u 工具一个副作用域声明 + 审批分级（P1 提案）做静态守卫，否则红线靠口头纪律守不住。

### Must-have
- slab-agent 保持纯编排：DAG/plan/task.complete/verify/循环签名计算等确定性逻辑一律落 slab-agent-tools 或编排内置 guard，thread.rs:248 for-turn 循环与 turn.rs:198 tool_calls.is_empty() 判停作为契约不破坏（新能力挂载不改判停条件本体）。
- 结构化终止不倒退：禁止任何关键词/正则文本判停。完成判定走 task.complete 工具（结构化 tool call）或现有 tool_calls.is_empty()，二选一双轨。
- ToolContext 扩容以 Option + builder 形式做，不破坏现有 ToolHandler 构造与测试；workspace/session/记忆/计划句柄以 trait object 注入，确定性能力本体留 slab-agent-tools。
- 循环检测优先走 control.rs:381 interrupt soft-stop（保留线程可续跑 + 人工介入），不直接 shutdown 硬杀；命中上限必须有结构化 reason 写 tracing/session。
- workspace-scoped context 注入路径钉死：经 sidecar 启动参数（workspace-mode-design.md:74）+ app-core 合并 settings + 扩容后 ToolContext 三层，主/子 agent 拿到一致的 workspace_root 与已合并 settings。
- 所有后端 API shape 变更走 gen:api、schema 变更走 gen:schemas/gen:plugin-packs，进 CI 门禁；只扩 /v1/*，不新增平行 API 树。
- a2u 工具副作用域静态受限：只允许打开受信 host UI 面 / 读写本地 sandbox 文件 / 调 /v1 API，绝不触碰任意像素操控；插件 WebView caller id 从 label 推导（红线）适用 a2u 调插件场景。

### 风险
- ToolContext 扩容（P0 第一条）是横切改动，会触及 turn_tool_call.rs:39 唯一构造点 + 所有 ToolHandler 实现的测试构造处（tests.rs 多处、subagent.rs 测试）。若新增字段非 Option/无默认值，会一次性改炸整个 slab-agent-tools 测试矩阵。缓解：新增字段全部 Option + 提供 ToolContext::for_thread(thread_id) builder。
- 循环检测的误报风险：近 N 轮签名去重可能把合法的渐进式修改（如多次 read_file 探查不同文件后回到同一文件）误判为 stuck。缓解：签名去重只对副作用类工具（write_file/apply_patch/shell）计数，只读工具不计；阈值给可配置 + 命中后走 interrupt soft-stop 而非直接终止，留人工介入余地。
- plan DAG 持久化进 slab-agent-memories 可能与现有 memory pipeline（runtime.rs:78 AgentMemoryPipeline）的注入/检索逻辑耦合不当，导致 plan 被当作普通 memory 被检索回灌造成上下文污染。缓解：plan 走独立 namespace（如 plan:<thread_id>），不进通用 memory 检索。
- task.complete 工具若被 LLM 过早调用（plan 节点未全 mark_done 就宣布完成），会绕过校验。缓解：task.complete 内部校验所有 plan 节点 status==completed 才允许 Final，否则返回错误要求继续；并保留 turn.rs:198 的 tool_calls.is_empty() 作为兜底 Final 路径（双轨，不互斥）。
- 审批分级（P1）若 a2u 打开类工具默认 allow，可能被恶意插件 contributes 的伪装成打开类工具的工具钻空子执行副作用。缓解：分级策略由 app-core 按工具白名单静态配置，不允许插件自报风险等级（插件不可信，本地优先隐私模型）。
- delegate_subagent 同步阻塞（additional_pain_point）若改成并行 spawn，会放大 token 消耗（Anthropic：多 agent 烧 15x），需配合按复杂度缩放规则（P2）+ 预算看板，否则成本失控。
- 本机 GLM workflow 账户在高并发 agent 时 429（见 MEMORY 索引 glm-workflow-rate-limit-throttling），plan-and-execute 触发的并行 subagent fan-out 需限流，避免触发限流。

### 边界红线注意
- DAG/规划/校验/task.complete 必须落 slab-agent-tools，由 app-core/runtime.rs 注册（runtime.rs:48 refresh_memory_tools 是活样本）。若任何确定性逻辑漏进 slab-agent 的 thread.rs/turn.rs 编排核心，违反 AGENTS.md slab-agent 纯编排红线，一票否决。循环 guard 是唯一可例外内联在 slab-agent 的（因读 TurnOutcome 属编排内置状态），需架构显式签字。
- ToolContext 扩容是 slab-agent 公共类型的 shape 变更，属契约改动。扩容只能加字段（Option），不能改/删现有 thread_id/turn_index/depth，且不能让 slab-agent 依赖任何业务 crate（workspace/config）——注入的是 trait object（在 slab-agent 定义 trait，在 app-core/slab-agent-tools 实现），否则 slab-agent 反向依赖业务，破坏分层。
- 新增 task.complete/plan DAG/verify/a2u 工具若涉及 /v1 shape 变化（如任务总结三动作的端点），必须 bun run gen:api；涉及 settings schema 或 plugin manifest（如 contributes 加 a2u 工具声明）必须 gen:schemas/gen:plugin-packs。漏跑即违反红线。
- 不引入 ADK 声明式 SequentialAgent/LoopAgent/ParallelAgent 类体系，不引入 A2A 跨厂商协议本身（信任模型不符）。只借鉴显式终止条件、artifact 解耦思想。引入框架协议即违反 slab-agent 纯编排 + 本地优先隐私模型红线。
- 不做截图-坐标 Computer Use（Anthropic CU/OpenAI CUA 路线）。a2u 工具副作用域只限打开受信 host UI 面/读写 sandbox 文件/调 /v1 API。引入像素操控即违反不做完整 a2u 红线。
- subagent 委派只做 orchestrator-worker（委派并等待结果，控制权不永久转移），不做隐式 handoff。OpenAI Swarm 反复强调 handoff 必须显式可测——delegate_subagent 已是显式工具调用，勿引入 agent 间暗中互相触发的隐式控制流。
- subagent 不做需要全局历史的编码修改任务（Claude Code 共识：subagent 是 researchers not coders，隔离上下文会让它误改）。主仓库编码修改留主线程，只把只读研究/广度探索/独立产物生成委派给 subagent。否则隔离反害任务。
- SQLx migration 只追加（红线）：plan 持久化若走 DB 而非 memory 文件，migration 只能 ADD，且需确认是否真有必要（倾向先走 slab-agent-memories 文件存储，不碰 DB migration）。
- 不新增第二套 LSP bridge：workspace 编码能力增强（verify.workspace_build 等）若涉及 LSP，只走 /v1/workspace/lsp/{language} -> app-core 现有通道，不在桌面 host 另起 bridge。

### 开放问题
- task.complete 触发 Final 与 turn.rs:198 的 tool_calls.is_empty() 的优先级关系：task.complete 是普通工具调用（调用后仍有下一轮 LLM 响应），还是特殊控制工具（调用即 Final）？若前者，需靠 LLM 调完 task.complete 后下一轮自然不调工具走到 turn.rs:198；若后者，需在 turn_tool_call.rs 识别 task.complete 后短路返回 TurnOutcome::Final。倾向后者但需确认不破坏 tool_calls 判停契约。
- 循环检测的 guard 放 slab-agent 内联（读 TurnOutcome）还是独立 slab-agent-loopguard crate？内联更贴近编排职责但增加 slab-agent 代码量；独立 crate 更清晰但跨 crate 读 TurnOutcome 需暴露内部类型。倾向内联（loop-guard 属编排内置状态，不引入新业务概念），需架构确认。
- plan DAG 持久化进 slab-agent-memories 是否需要新建独立 namespace/port，还是复用现有 AgentMemoryPipeline 的存储？复用更快但耦合污染风险（见 risks）；独立更干净但工作量大。
- delegate_subagent 改并行 spawn（解决同步阻塞）是否在本期范围内？若做，需配合按复杂度缩放规则 + 预算看板 + GLM 限流（见 risks），工作量从 M 升 XL，需产品确认是否本期必须。
- workspace settings 合并语义（全局 vs workspace 的 agent.tools.allowed 是并集还是 workspace 覆盖、agent.hooks 是否 workspace 可禁用）需产品+配置 owner 钉死契约，否则 workspace-scoped agent context 注入拿不到一致工具集。当前 docs 只说覆盖，没说深度。
- a2u 任务总结三动作（open_project/request_review/feedback）对应的 /v1 端点 shape 是复用现有 /v1/sessions 还是新增 /v1/agent/actions？需与前端 + API owner 对齐，影响 gen:api 工作量。

---

## 基础设施 / SRE 工程师

### 核心关切
- 多窗口与子窗口生命周期是全仓空白：packages/slab-desktop/src 全仓只有 layouts/window-controls.tsx:1 的 getCurrentWindow()（单窗口最小化/最大化/关闭），无 WebviewWindowBuilder / getAllWindows / WindowEvent 多窗口基础设施。痛点1要求'原页面退化为 Tauri 子窗口/新 tab'，但 SRE 视角这会同时打开多个 WebView 进程、各自连 WS、各自加载 Monaco/插件，内存/CSP/事件总线耦合是新系统的核心稳定性风险——必须先定数量上限与生命周期契约再开工，否则极易 OOM 与事件泄漏。
- sidecar 重启语义是'全量 kill + 冷启动'，与 running task 恢复直接冲突：bin/slab-app/src-tauri/src/setup/sidecar.rs:39-58 的 shutdown_server_sidecar 走 stdin 'shutdown\n' + 8 秒 SERVER_SHUTDOWN_TIMEOUT + child.kill()；workspace 切换在 bin/slab-app/src-tauri/src/workspace.rs 的 init() 仅在启动期生效，运行中切换 = 杀进程重拉。期间所有 Agent thread 会随进程消失，若无显式恢复就会留下'假 running'线程。
- max_turns 耗尽被静默标成'完成'，是中断一致性最大隐患：crates/slab-agent/src/thread.rs 的 'turns: for ... 循环正常结束后，代码在 430-450 行统一发 ResponseCompleted + set_status(Completed) + store.update_thread_status(..., Completed)。turn 用尽 ≠ 任务完成，但持久化状态写的是 Completed，前端据此显示'任务完成'，恢复时无从区分真完成与超限停摆——这正是用户痛点4'兜底'与 AGENTS.md '长任务保持 task-oriented'的真实落差点。
- 多 agent 并发预算是裸常量且无降级路径：crates/slab-app-core/src/infra/agent/bootstrap.rs:168 硬编码 max_threads:32 / max_depth:4（与 crates/slab-agent/src/config.rs:86 默认 max_turns:10、subagent.rs:7 DEFAULT_SUBAGENT_TURNS:8 共同构成预算），但没有任何'按负载降级 / 排队 / 内存阈值熔断'。Anthropic 经验是多 agent 比 chat 烧约 15× token，32 并发在低配机器上会直接 OOM/限流 429，没有背压就没有生产可用性。
- 可观测性缺口与诊断包泄漏 secret 双风险并存：仓内无诊断包导出（export_diagnostics/diag bundle 不存在），而 slab-server.log 在 assistant-markdown.test.tsx:262 测试断言里已出现 919MB 单文件——无 rotation、无 size cap。同时 crates/slab-config/src/app_config.rs:81 的 admin_api_token 与 descriptor.rs:424 server.admin.token 是明文 secret 字段，pmid_service.rs:105 虽有 secret() 占位符机制，但日志/config dump 若纳入诊断包会泄漏 key。需要一份确定的字段白名单。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| P0｜定义多窗口/子窗口生命周期契约与资源上限 | P0 | L | 痛点1（统一入口缺失，原页面退化为子窗口）+ 痛点2（插件子窗口 a2u 打开） | 痛点1要求'原页面退化为 Tauri 子窗口'，但当前 packages/slab-desktop/src 仅有 layouts/window-controls.tsx:1 的单窗口控制，全仓无 WebviewWindowBuilder/getAllWindows/WindowEvent。SRE 视角必须先把契约钉死：(1) 主窗口 + 受控子窗口（workspace/diff/media 渲染面）+ 插件 child WebView 三类，明确'同源 label 命名空间'（与 AGENTS.md 'plugin WebView caller id 从 label 推导'红线对齐）；(2) 子窗口数量上限（建议主窗口外硬上限 8 个同时存活，理由：每个 WebView 独立进程 + Monaco/插件内存基线 ~150-300MB，低配 16GB 机超 12 个必 OOM）；(3) 子窗口关闭走 WindowEvent::CloseRequested -> 先 control.interrupt(该窗口绑定的 thread) 再销毁，避免孤儿 thread；(4) 事件总线不复制——子窗口一律通过单一 WS 连到 slab-server /v1（已在 packages/slab-desktop/src/pages/workspace/lib/workspace-language-client.ts:26 复用的同一 server），host 侧按 thread_id 路由，绝不每个窗口开独立 WS 池。 |
| P0｜workspace 切换 = sidecar 优雅重启 + running task 受控迁移/挂起 | P0 | L | 痛点3（workspace 能力，项目设置合并）+ 痛点4（任务恢复/中断一致性） | 当前 bin/slab-app/src-tauri/src/setup/sidecar.rs:39 的 shutdown_server_sidecar 是 stdin 'shutdown\n' + 8s 超时 + kill；workspace.rs init() 只在启动期解析 sidecar_config_for_workspace，运行中切换必然杀进程重拉。必须显式定义切换协议：(1) 切换前枚举当前 active_thread_count（control.rs 已有该方法），对 Running 线程统一 control.interrupt（control.rs:381，已解耦、保留线程可续），写'切换快照'到 session_state_dir（.slab/sessions）；(2) 重启后 host 调 /v1/sessions 恢复时，按持久化状态重建——这要求 thread 状态不能是假 Completed（见下条）；(3) 切换 UI 显示'N 个任务已挂起，切换后可恢复'而非静默丢失。这是把'不留假 running'的硬要求落地到 sidecar 生命周期的唯一位置。 |
| P0｜修掉 max_turns 耗尽被标 Completed 的'假完成'，引入带 reason 的结构化终止 | P0 | M | 痛点4（兜底：全局最大 turn 轮次 + 任务恢复） | crates/slab-agent/src/thread.rs 的 'turns: for 循环正常结束（非 break）会落到 430-450 行 ResponseCompleted + ThreadStatus::Completed + store.update_thread_status(Completed)。turn 耗尽本质是'未完成'，却被持久化成完成态——这是恢复一致性的根因，也是用户痛点4'兜底'的代码落点。修复方案（符合 slab-agent 纯编排边界，只动 thread.rs 状态机，不引入新工具）：(1) 区分 break 'turns 的来源——正常耗尽 vs Final；(2) 耗尽时引入新终止态（如 MaxTurnsReached / Stopped，需扩 ThreadStatus 枚举与 state.rs 状态迁移表），写 reason=max_turns 到 store + tracing；(3) 前端按 MaxTurnsReached 显示'已达轮次上限，可续跑'并提供 resume 按钮。复用 control.rs:381 interrupt 语义（保留线程可继续），不引入硬杀。 |
| P0｜多 agent 并发预算与降级（对齐 bootstrap.rs:168 的 32/4，加背压与内存熔断） | P0 | L | 痛点4（多 agent 并发，对齐 32/4）+ 主动补充：资源治理/OOM | bootstrap.rs:168 硬编码 max_threads:32/max_depth:4，config.rs:86 max_turns:10，subagent.rs:7 DEFAULT_SUBAGENT_TURNS:8——预算是常量但无降级。Anthropic 多 agent 烧 ~15× token，32 并发在 16GB 机或受限 provider 账号下会 OOM/429（我的记忆里本账号 ~10-14 并发就 429）。SRE 要求：(1) 把 32/4 从裸常量提为可配置（settings 新增 agent.runtime.limits 域，走 gen:schemas）；(2) 在 AgentControl.new（control.rs:97）之外新增'软上限/排队'——超过软阈值（如 16）的新 spawn 进入 FIFO 队列而非直接拒绝；(3) host 侧内存监控（process supervisor 已存在于 crates/slab-app-core/src/infra/process_supervisor.rs），内存超阈值时停止新 spawn subagent 并通知主 agent 'replan/降并发'；(4) 按任务复杂度缩放 subagent 数的 prompt 规则（事实类 1 个/对比类 2-4 个/复杂研究 8-10 个），避免'简单查询 spawn 50 个'反模式。 |
| P0｜循环/重复检测兜底（走 interrupt 不走 shutdown） | P0 | M | 痛点4（兜底：多轮重复任务检查） | 当前只有 thread.rs 的 max_turns 硬上限 + invalid_tool_call_retries 预算 + 并发/深度上限，无重复检测。外网调研（arXiv:2407.20859 / Hermes-agent SHA-256）给出工业实践：对近 N 轮 (tool_name, 关键 args) 做哈希去重，连续重复命中阈值判 stuck。落地（符合边界——确定性逻辑归 slab-agent-tools 或 slab-agent 编排内）：(1) 在 thread.rs 主循环维护滑动窗口签名集合；(2) 命中阈值时调用已解耦的 control.rs:381 interrupt（保留线程可继续 + human-in-the-loop），绝不 shutdown；(3) 写 stuck_detected 事件到 slab-agent-tracing。这是把'多轮重复任务检查'从需求变成代码的唯一位置，且复用 interrupt 而非新增硬杀路径。 |
| P0｜诊断包字段白名单（排除明文 secret）+ slab-server.log rotation | P0 | M | 主动补充：诊断包/可观测性/secret 泄漏（痛点3 workspace 项目目录也涉及 .slab/ 落盘内容安全） | 必答4要求字段清单。当前仓内无诊断包导出能力，且 assistant-markdown.test.tsx:262 断言里 slab-server.log 已达 919MB（无 rotation/size cap）。同时 app_config.rs:81 admin_api_token、descriptor.rs:424 server.admin.token 是明文 secret，pmid_service.rs:105 的 secret() 占位符机制只保护配置 API 往返，不保护日志。SRE 立项：(1) 新增 host 侧 export_diagnostics 命令（bin/slab-app/src-tauri，host-only，不扩 /v1/* 边界），打包字段白名单见 must_haves；(2) 给 slab-server.log 加 tracing-appender rolling + 单文件 size cap（建议 50MB × 保留 5 份）；(3) 日志侧加 secret redaction filter（对匹配 api_key/token/sk-/Bearer 的行做掩码），从源头保证诊断包不含明文。 |
| P1｜统一 secret store（迁移 admin token / provider key 到 OS keychain） | P1 | L | 主动补充：统一 secret store（与诊断包安全、隐私优先红线强相关） | app_config.rs:81 admin_api_token、descriptor.rs:424 server.admin.token 当前随配置落盘（settings.json/settings_overlay_path = .slab/settings.json）。本地优先+隐私优先的红线要求 secret 不落明文。SRE 建议：引入 keyring crate（跨平台：Windows Credential Manager / macOS Keychain / Linux Secret Service），pmid_service.rs:105 的 secret() 占位符机制已具备接入点——配置文件只存引用句柄，真实值走 keychain。诊断包天然不含明文（配置里就没有）。注意：这是新增跨 crate 能力，归属需论证（见 boundary_watchouts）。 |
| P1｜subagent span 关联与 agent 可观测性面板 | P1 | M | 主动补充：可观测性（agent trace/metrics、subagent span 关联） | subagent.rs 的 delegate_subagent 已用 parent_id 回链、独立 thread_id，但 tracing 侧无明确的 parent->child span 因果展示。SRE 要求每个 subagent span 带 parent_thread_id / depth / allowed_tools / turn_budget，便于：(1) 在面板上看到 DAG 式 agent 树；(2) 定位哪个 subagent 卡住/烧 token；(3) 与循环检测、max_turns reason 事件关联。复用现有 slab-agent-tracing，不新增协议。这是把'可观测性'从有 trace 文件升级到'能回答为什么停/卡在哪'的工程闭环。 |
| P1｜安装器/分发：.slab 引导与首次运行健康检查 | P1 | M | 主动补充：安装器/分发影响 | SRE 视角，安装期是 P0 数据安全窗口。Windows 安装器（bun run build:windows-installer）必须：(1) 首次启动引导用户选择 app_home 与默认 workspace 根目录，避免默认写 C:\\ 用户目录深处；(2) 健康检查 sidecar 是否能 spawn（sidecar.rs:120 .sidecar('slab-server') 失败时的降级提示）；(3) 检查 bundled_lib_dir（resources/libs）完整性。这是减少'装完打不开/找不到模型'工单的预防性投入。 |
| P2｜CI 回归门：多窗口/并发/恢复的场景化集成测试 | P2 | M | 主动补充：CI/回归门（覆盖痛点1/3/4 的 P0 变更） | 上述 P0 引入多窗口生命周期、sidecar 切换恢复、假完成修复、循环检测——都是'平时不触发、触发即数据丢失'类风险。必须在 CI 加场景化测试（复用 crates/slab-agent/src/tests.rs:1066 已有的 mock 框架 + 多窗口集成测试），作为合并门。E2E 用 bun run test:browser / test:components。 |
| P2｜plan_update 升级为 plan-and-execute + 持久化（防 200K 截断） | P2 | XL | 痛点4（主 agent 全局规划/DAG/完成判定） | crates/slab-agent-tools/src/plan.rs 当前只 normalize 回显 plan items，无 DAG/依赖/完成判定/持久化。外网调研（Anthropic Multi-Agent Research System）强调'save plan to Memory because 200K context will be truncated'。建议：(1) plan.rs 升级为带 depends_on 的 DAG 节点 + mark_done(task_id)/replan(plan_patch) 接口（确定性数据结构，归 slab-agent-tools，符合红线）；(2) plan 持久化到 slab-agent-memories，每轮开局重载；(3) 引入确定性 task.complete 工具表达'所有节点 done'，强化 turn.rs:198 已有的结构化判停（tool_calls.is_empty()），坚决不引入文本判停。SRE 视角这是 P2 因为它不阻塞稳定性，但是恢复/中断一致性的语义前提（恢复时要知道 plan 进度）。 |

### 补充痛点（用户未提）
- crates/slab-agent/src/thread.rs:430-450 的 max_turns 耗尽被标 ThreadStatus::Completed，是恢复语义的根因隐患——恢复时无法区分'真完成'与'turn 耗尽停摆'，导致 resume 逻辑无据可依（用户提的 4 条痛点未点名，但它是痛点4 兜底能落地的代码前提）。
- slab-server.log 无 rotation/size cap，测试里已出现 919MB 单文件（assistant-markdown.test.tsx:262），长期运行会写爆磁盘 + 拖慢诊断包导出。
- admin_api_token（app_config.rs:81）/ server.admin.token（descriptor.rs:424）随配置落盘为明文，.slab/settings.json 作为 workspace overlay 会被 git/同步工具带走——与'隐私优先'红线存在张力，缺统一 secret store。
- sidecar 切换是全量 kill（sidecar.rs:39 SERVER_SHUTDOWN_TIMEOUT=8s + child.kill()），切换期间前端 WS 断连若无统一重连退避策略，多窗口同时重连会形成 thundering herd。
- 并发上限 32/4（bootstrap.rs:168）是裸常量、无运行时可观测与降级，运维无法回答'当前并发多少/是否接近上限/为何拒绝 spawn'，排障全靠看日志。
- 插件 child WebView 与未来'原页面退化子窗口'共用 Tauri WebView 资源池，但当前无统一的'子窗口预算账本'，两类窗口可能互相挤占导致插件渲染面 OOM。
- interrupt（control.rs:381）已解耦、保留线程可继续，但前端是否提供'从 Interrupted/MaxTurnsReached 续跑'的入口未在 packages/slab-desktop/src 验证到——解耦的恢复能力若前端不暴露，等于没做。
- 诊断包尚未存在，但一旦要做，slab.db（workspace 模式 .slab/slab.db）、sessions、agent trace（含工具调用 args，可能有用户文件内容/prompt）都是潜在的隐私泄漏面，需要独立的字段白名单而非'打包整个 app_home'。

### Must-have
- 诊断包字段白名单（必须显式枚举，排除明文 secret）：(1) slab 版本/git commit/OS/架构；(2) sidecar 启动参数（bin/slab-app/src-tauri/src/setup/sidecar.rs 的 args，但 settings_path/database_url 只显示路径不显示内容）；(3) slab-server.log 末尾 N KB（经 secret redaction）；(4) agent thread 统计：active_thread_count、各 thread 的 status/turn_index/depth/max_turns reason（不含 messages 原文）；(5) 失败工具调用摘要（tool_name + error 字符串，不含 args 原文——args 可能含用户文件内容/secret）；(6) 资源快照：内存/CPU/磁盘。明确排除：.slab/slab.db、sessions 全量消息、config 中的 admin_api_token/provider key、trace 中工具 args 原文。
- 多窗口契约硬上限：主窗口外同时存活的受控子窗口 ≤ 8（理由见 P0 提案1），超出时 host 拒绝新建并提示用户关闭旧窗口；插件 child WebView 单独计数，两类合计内存超阈值（如 70% 物理内存）触发熔断停止新 spawn。
- workspace 切换前必须执行：枚举 active thread -> 逐个 control.interrupt -> 写 session 快照 -> 才能 shutdown_server_sidecar。任一步失败中止切换并报错，绝不静默 kill。
- thread 终止必须带 reason：completed / max_turns / repetition_detected / interrupted / errored / shutdown，写入 store + tracing + WS 事件，前端据此决定'续跑/重试/放弃'。修复 thread.rs:430-450 的假 Completed 是硬要求。
- 循环检测命中走 control.interrupt（保留线程）而非 shutdown，与 AGENTS.md '长任务保持 task-oriented' + Anthropic 'restarts are expensive' 一致。
- 所有跨进程（sidecar/JS runtime/Python runtime/插件 WebView）的 agent span 通过 slab-proto 的 parent_span_id/trace_context 关联（slab-agent 已有 trace_context.with_turn，需扩展到跨进程透传）。
- secret 不进诊断包、不进明文日志：日志侧加 redaction filter，config 侧走 secret() 占位符 + 目标 keychain；诊断包字段白名单由 SRE + 安全共同签字冻结，新字段需评审。

### 风险
- 多窗口引入后，若不强制'单一 WS 连接 + host 按 thread_id 路由'，每个子窗口自建连接池会导致 sidecar WS 连接数随窗口数线性增长，触发 slab-server 连接上限或单进程 fd 耗尽。
- workspace 切换若不先 interrupt running thread 就 kill sidecar，会留下'store 里 Running、实际进程已死'的幽灵线程；前端 WS 断开重连后状态不一致。修复必须覆盖 bin/slab-app/src-tauri 的切换路径，不只是 sidecar.rs。
- 把 max_turns 耗尽从 Completed 改为新状态（MaxTurnsReached）会触及 ThreadStatus 枚举（slab-types）、state.rs 状态迁移表、store schema（SQLx migration 只追加）、前端状态展示——是跨层变更，需 gen:api/gen:schemas 同步，回归面大。
- 诊断包若打包不当（如整目录 zip），会一次性泄漏 .slab/slab.db、sessions、含用户代码的 trace——比'不做诊断包'更危险。必须字段白名单先行，且默认排除数据库原文（只导元数据/统计）。
- keychain 依赖引入新跨平台二进制（Linux 无统一 Secret Service 时降级行为需定义），可能拖慢安装器与 CI；且 keyring crate 边界归属需论证（见 boundary_watchouts）。
- 并发降级若实现不当（如主 agent 反复 replan 降并发又被规则升回），会形成'降级-升级'振荡；需引入冷却窗口。
- subagent span 关联若依赖 trace 文件后处理而非结构化字段，跨进程（JS/Python runtime）span 难拼接——必须在事件协议层（slab-proto）加 parent_span_id。

### 边界红线注意
- P0 多窗口契约：不违反 AGENTS.md 'plugin WebView caller id 从 label 推导'红线——子窗口 label 必须编码窗口类型 + thread_id + 来源，caller id 由 label 反推，不从 payload 取。子窗口渲染面必须是 host 固定受信组件，不能让 agent/插件自由生成任意 HTML/JS（= Computer Use 变体，违反'不做完整 a2u'）。
- P0 sidecar 切换恢复：修改在 bin/slab-app/src-tauri（host 层）+ 复用 control.rs interrupt，不扩 /v1/* 新 API 树（红线）。恢复读 session 走现有 /v1/sessions。
- P0 假完成修复：扩 ThreadStatus 枚举触及 slab-types + state.rs 状态机 + store（SQLx migration 只追加，红线）+ 前端。不引入文本判停（共享上下文已确认 turn.rs:198 是结构化判停，保持）。新状态机逻辑仍在 slab-agent 纯编排内，不把 DAG/规划塞进 slab-agent（红线）。
- P0 并发降级：把 32/4 提为可配置走 settings（gen:schemas，红线）；排队/熔断逻辑属 slab-agent 编排或 host，确定性策略不污染 slab-agent-tools。不照搬 ADK/A2A 声明式框架（外网调研反模式）。
- P0 循环检测：确定性逻辑，若实现为工具则归 slab-agent-tools，若为编排内滑动窗口则归 slab-agent——二选一，不跨界。命中走 interrupt 不走 shutdown（与 Anthropic 'restarts are expensive' + AGENTS.md '长任务 task-oriented' 一致）。
- P0 诊断包：export_diagnostics 必须 host-only（bin/slab-app/src-tauri Tauri command），不扩 /v1/*（红线'桌面 host 不新增平行 API'）。日志 redaction 不破坏现有 --log-file（sidecar.rs）行为。
- P1 统一 secret store：keyring 依赖跨平台，需论证 crate 归属——建议新 crates/slab-secrets（独立职责），不污染 slab-config 的纯配置职责，不污染 slab-app-core（保持 HTTP-free，红线）。pmid_service.rs:105 的 secret() 占位符是接入点，配置文件只存 keychain 引用句柄。
- P2 plan DAG 升级：plan.rs（crates/slab-agent-tools）属确定性工具层，DAG 数据结构 + mark_done/replan 在此 crate 合规。但'主循环 thread.rs 调 replan 触发重规划'是编排行为——必须只让 LLM 通过 tool_call 触发 replan，不在 slab-agent 核心硬编码 DAG 调度逻辑（红线'slab-agent 保持纯编排'）。task.complete 是确定性工具，归 slab-agent-tools。

### 开放问题
- 多窗口下，插件 child WebView 与'原页面退化子窗口'是否共用同一 Tauri WebView 资源池？若是，预算账本是否需要统一到 host 层（bin/slab-app/src-tauri）而非分散在 packages/slab-desktop？这决定 P0 提案1 的实现位置。
- workspace 运行中切换（非启动期 init）的触发路径在 bin/slab-app/src-tauri 哪里？workspace.rs 只见 init()，运行时切换的 Tauri command 尚未定位到——需产品确认切换 UX（菜单/命令/拖拽）以确定 host 挂载点。
- MaxTurnsReached 新状态是否需要 SQLx migration 来持久化（store.update_thread_status 当前存字符串 reason）？还是复用现有 status + reason 字段即可？影响是否触发 gen:schemas/migration。
- keyring crate 应归哪个 crate？候选：crates/slab-config（与现有 secret() 占位符最近）或新 crates/slab-secrets（独立职责）。需架构签字避免污染 slab-config 的纯配置职责。
- 循环检测的'近 N 轮'N 值与'重复阈值'取多少？需用真实多轮任务 eval 标定（外网调研建议 SHA-256 of (tool,args)，但 args 含文件内容时哈希稳定性需验证）。
- 诊断包是 host-only Tauri command 还是走 /v1/* 新端点？AGENTS.md 红线'只扩 /v1/*'与'host 命令保持 host-only'两条都存在——倾向 host-only（不暴露到 server API），但需确认产品是否要远程支持（远程访问是可选增强）。
- 并发软上限/排队的语义：超过软阈值的新 spawn 是排队等待，还是直接拒绝让主 agent replan？前者更顺滑但有'排队中'状态要暴露给前端，后者更简单但可能让主 agent 反复重试。

---

## Agent / AI 架构师（主 agent 编排器、subagent 隔离、结构化终止、循环兜底、a2u 受控工具集设计者）

### 核心关切
- 完成判定必须 default-deny：当前 turn.rs:198 用 response.tool_calls.is_empty() 判 Final，是结构化的，但靠的是'LLM 不再发工具调用'这个消极信号。主 agent 全局规划后必须升级为显式 task.complete 工具（结构化 tool-call 判停），没有该调用一律不算完成——否则 agent 会在中途空转停住被误判为完成。坚决不引入任何文本/正则匹配判停（共享上下文确认代码里也不存在）。
- 主 agent 是纯 ReAct turn-by-turn（thread.rs:248 的 for turn_offset in 0..max_turns），没有 plan-and-execute 的 replan 分支，plan_update（plan.rs:109）只 normalize 回显，无持久化、无 DAG、无结果校验、无完成判定、无恢复。这是 4 个痛点里最大的架构债——但 DAG/规划/校验必须落在 slab-agent-tools（确定性工具），slab-agent 必须保持纯编排，红线不可破。
- 上下文隔离方向正确但深度不足：subagent.rs:108 起全新对话、只传委派 task，parent_id 回链已有，allowed_tools 白名单已有（subagent.rs:29）。但缺少 artifact 解耦（子 agent 应把产物写 .slab/ 只回引用，而非把全文塞回 completion_text）和 DelegateSubagentArgs 缺 output_format/objective/边界四要素（Anthropic 反复强调缺这些会重复劳动）。
- 兜底只有硬上限：thread.rs:248 max_turns、invalid_tool_call_retries 预算、并发 32/深度 4（control.rs:54 AgentControlLimits），没有任何循环/重复检测。工业实践共识是 max_turns 命中应走 interrupt（control.rs:381 已解耦、保留线程可续跑 + human-in-the-loop），而非静默 break 让长任务前功尽弃（当前 thread.rs:338 是 break 'turns）。
- 默认就上多 agent 是反模式：Anthropic 数据多 agent 比 chat 烧约 15x token，仅在'高价值+高并行+信息超单 context+复杂工具'时才用。编码等强依赖任务单 agent 常更优。Slab 默认应单 agent ReAct，subagent 仅用于只读研究/广度探索/独立产物生成。需要 prompt 工程层按查询复杂度缩放子 agent 数（事实 1 个、对比 2-4 个、研究 10+），防'为简单查询 spawn 50 个子 agent'。

### 提案

| 提案 | 优先级 | effort | 映射痛点 | 理由 |
|---|---|---|---|---|
| P0 升级 plan_update 为带依赖边的 DAG Plan 数据结构（落在 slab-agent-tools，非 slab-agent） | P0 | M | 痛点4 主 agent 全局规划/DAG | 把 crates/slab-agent-tools/src/plan.rs 的 PlanItemInput 从 {step, status} 升级为带 task_id + depends_on + status + result_ref 的 DAG 节点，新增 mark_done(task_id, result_ref) 与 replan(plan_patch) 接口。DAG/规划是确定性数据结构，归 slab-agent-tools，不污染 slab-agent 纯编排核心（守住 AGENTS.md 红线）。plan 工具内部做环检测、拓扑序校验。现有 normalize_plan 已做去重/计数，可在此之上扩展。 |
| P0 新增显式 task.complete 结构化完成判定工具（default-deny 终止协议） | P0 | S | 痛点4 结构化终止、default-deny 完成判定 | 新增确定性工具 task.complete(summary, artifact_refs)，由 LLM 在所有 plan 节点 mark_done 后调用判停。强化 turn.rs:198 的 tool_calls.is_empty() 语义为'必须看到 task.complete 或 tool_calls 为空且 plan 全节点 completed'双条件，任一不满足继续循环直到 max_turns。坚决不引入文本/正则判停（用户担心的问题代码里不存在，保持住）。task.complete 属确定性工具归 slab-agent-tools，符合边界。结构化终止协议见下文 must_haves。 |
| P0 主循环 thread.rs:248 新增 replan 分支 + 持久化 plan 到 memories 防 200K 截断 | P0 | M | 痛点4 全局规划、恢复 | 在 thread.rs:248 的 'turns 循环每轮结束（TurnOutcome::ToolCalls 后）允许 LLM 通过 replan 工具触发动态重规划。借鉴 Anthropic'save plan to Memory because context will be truncated'的教训：plan 写入 slab-agent-memories（Slab 已有该 crate），每轮开局从 memory 重载计划。这是 slab-agent 编排核心的小幅改动（仅加 replan 分支与 memory 读写），不引入规划逻辑本身（规划仍在工具层）。 |
| P0 循环/重复检测：近 N 轮工具调用签名去重 + 命中走 interrupt 而非 shutdown | P0 | M | 痛点4 循环/重复任务检查 | 对 (tool_name, 关键 args 的规范化签名) 做 SHA-256 哈希，维护近 N 轮（建议 N=3）的环形缓冲，连续重复命中阈值判 stuck。命中后调用 control.rs:381 的 interrupt（保留线程、可续跑、可人工介入）而非 shutdown（control.rs:367 不可恢复）。当前缺检测、缺处置。复用 slab-agent-tracing 记录 stuck 事件。 |
| P0 终止理由结构化（max_turns 命中从静默 break 升级为带 reason 的结构化终止） | P0 | S | 痛点4 兜底 | 把 thread.rs:338 的 break 'turns 升级为带 TerminationReason 的结构化输出（Completed / MaxTurns / RepetitionDetected / BudgetExhausted / Interrupted / InvalidToolCallBudget），写入 thread snapshot 与 tracing。前端可据此展示'为什么停'。当前 interrupted 已分支处理（thread.rs:340），扩展即可。避免 max_turns 硬杀无恢复（restarts are expensive and frustrating）——MaxTurns 应等同 interrupt 语义保留线程可续跑。 |
| P1 subagent 四要素补齐 + artifact 落盘只回引用 | P1 | M | 痛点4 subagent 上下文隔离 | 在 DelegateSubagentArgs（subagent.rs:21）已有 task/model/system_prompt/allowed_tools/max_turns 基础上，prompt 工程层强制 lead agent 给出 objective、output_format、来源指引、明确任务边界（Anthropic 缺这些会重复劳动）。同时 workspace 任务让 subagent 把产物写 .slab/ 工作目录，ToolOutput 只回传路径引用（completion_text 不再塞全文），减少 token 与 telephone game。契合 workspace 模式（docs/development/workspace-mode-design.md）。 |
| P1 端态 + checkpoint 结果校验（确定性工具，不用同一 LLM 自评） | P1 | M | 痛点4 结果校验 | 对编码/媒体类任务，plan 节点完成后跑确定性校验工具（cargo check / lint / diff / 媒体元数据），校验结果作为 plan 节点的 result_ref。明确反对用与 generator 同一模型同一上下文自评（会'自信确认自己的错误'反模式）。校验工具归 slab-agent-tools（确定性）。可复用现有 apply_patch/git 工具与 turn.rs:418 审批门。 |
| P1 a2u 受控工具集清单（tool 名 -> host 固定组件/动作派发表，非 Computer Use） | P1 | L | 痛点1 统一入口、痛点2 插件 a2u 打通 | 在 packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts 已有 tool_call 事件分支上，加受控 a2u 派发器。工具高阶、命名空间化（workspace.open / image.edit / audio.transcribe / video.generate / plugin.launch / hub.browse / review.show），内部封装导航+定位+渲染，而非暴露 open_tab/scroll/click_xy 低阶原语。模型只决定'调哪个工具+参数'，渲染哪个 React 组件由 host 固定。绝不走截图-坐标 Computer Use 路线（Slab 有完整 /v1 API，截图路线违反隐私优先且烧 token）。a2u 工具作为 host 注册的 ToolHandler，符合'插件/API 能力由 host/app-core 注册'红线。 |
| P1 a2u 工具审批三态分级（allow/sandbox/ask，对齐已有 ApprovalPort） | P1 | S | 痛点2 插件 a2u、痛点1 统一入口 | 对齐 Cursor/VS Code 的 allow/sandbox/ask 三态，强化 turn_tool_call.rs:418 已有 ApprovalPort：a2u 打开/渲染/只读类默认 allow（无副作用），改动/执行类 default ask，危险/外部网络类 sandbox 或拒。减少权限弹窗同时守住本地优先+隐私优先信任模型。反模式：Cursor YOLO mode 全自动无确认违反 Slab 审批门，不可作默认。 |
| P1 任务总结三动作按钮（打开项目 / 人工审阅 / feedback 调整）作为结构化输出 | P1 | M | 痛点1 统一入口、北极星任务总结动作 | task.complete 的 summary 之外，额外产出结构化 followup_actions: [{kind: open_project/request_review/feedback, target_ref}]。前端在任务总结处渲染三个按钮，每个对应一个受控 a2u tool call 的逆操作（用户点击触发 host 已知导航/动作，非任意跳转）。这是 ChatGPT Canvas 自动打开专门面 + Cursor Review->Find Issues 范式的组合。followup_actions 属确定性结构化输出，归 slab-agent-tools。 |
| P1 多 agent 按查询复杂度缩放（prompt 规则 + 默认单 agent） | P1 | S | 痛点4 subagent、痛点3 编码增强 | prompt 工程层强制：事实类查询单 agent（3-10 次调用），对比类 2-4 个子 agent，复杂研究 10+。防'为简单查询 spawn 50 个子 agent'。Slab 已有并发上限 32 + 深度 4（control.rs:54），方向正确。明确反模式：subagent 是'researchers not coders'，编码类主仓库修改留主线程（隔离会害它缺全局历史），仅把只读研究/广度探索/独立产物生成委派。 |
| P2 预算三重兜底看板（token/turn/工具调用次数，复用 slab-agent-tracing） | P2 | M | 痛点4 兜底 | 在 slab-agent-tracing（已有）上补预算看板：累计 token、累计 turn、累计工具调用次数，接近阈值提前告警/触发 interrupt。对应 Loop Drift 问题（即便有 stop 条件 agent 仍可能漂移）。这是 ADK 显式终止条件 + 工业实践 token/成本/时间三重兜底的轻量落地，不照搬 ADK 声明式 agent 类。 |
| P2 输出确定性 guardrail（write_file 后自动 lint、apply_patch 后语法检查） | P2 | M | 痛点4 结果校验、痛点3 编码增强 | 在 turn_tool_call.rs:418 审批门之外，对高风险工具输出加确定性 output guardrail，失败回灌给 LLM 重试（已有 invalid_tool_call_retries 预算可复用）。借鉴 OpenAI Guardrails 思路但不引入第二个模型。仅对高风险改动类工具启用，避免全局开销。 |
| P2 为每个 a2u 工具配 eval（真实多轮任务测 tool 描述命中率） | P2 | L | 痛点1/2 a2u 工具集质量 | Anthropic ACI 范式：用真实多轮任务（打开项目并定位文件、生成图片并打开预览、插件委派子 agent）测 tool 描述/参数设计的命中率，用结果反推 prompt-engineering。工具贵精不贵多，刻意保持 a2u 工具集小而高阶（避免 VS Code 128 工具上限的迷失）。 |

### 补充痛点（用户未提）
- plan_update 工具描述（plan.rs:57 'Record the current execution plan or todo list so it can be replayed in the agent timeline'）是低信号、被动语态的，模型不知道它该用来做全局规划与判完成。这是 ACI 反模式——工具描述应主动引导模型何时用、产出什么。需要随 P0 plan 升级一起改写描述。
- subagent 默认 max_turns=8（subagent.rs:9 DEFAULT_SUBAGENT_TURNS）+ 主 agent max_turns=10（config.rs:86），子 agent 单独跑可超过主 agent 单轮预算，但没有'主 agent 等待 subagent 的总预算'上限——一个慢子 agent 可能拖垮主线程响应体感。需要 subagent 超时/预算在 DelegateSubagentArgs 或 control 层暴露。
- 当前没有 budget guardrail（token/成本/时间）。本地说离线模型不烧钱，但云端 provider 场景下多 agent 15x token 会直接打爆用量。P2 预算看板对云端场景是 P0 级刚需，本地场景可 P2——建议按 provider 类型动态提级。
- delegate_subagent 是同步阻塞等待（subagent.rs:117 wait_for_terminal_snapshot），主 agent 在子 agent 跑时完全挂起。多个独立子任务本可并行 spawn（control.rs 已支持并发 32），但当前工具签名只允许一次一个。并行委派对'广度探索'类任务价值高，值得在 P1 subagent 升级时一并考虑。
- ToolContext（tool.rs:17）只有 thread_id/turn_index/depth，task.complete/dag/校验工具需要访问 plan 状态（当前 plan 只在工具 execute 内本地变量里，不持久）。需要 plan 状态存储位置：建议落 slab-agent-memories（已有 crate）而非新增 store 字段，符合边界。
- interrupt（control.rs:381）取消当前 turn 但保留线程，可续跑——但'续跑'的入口/协议未定义。max_turns 命中或重复检测命中后 interrupt 了，用户如何 resume？需要明确 resume 协议（续传 turn_index、是否回灌 interrupt 原因给 LLM），否则兜底了等于半截工程。
- 没有显式的 handoff/控制权转移原语。Slab delegate_subagent 是'委派并等结果'（同步 orchestrator-worker），不是 OpenAI Swarm 的 handoff（控制权永久转移）。这点设计是对的（避免隐式协作），但产品上'AI 在子窗口继续接管'的场景可能需要类 handoff 语义，需产品确认是否真需要。

### Must-have
- 结构化终止协议（default-deny）：完成判定 = (LLM 调用 task.complete 工具) AND (plan 全节点 status=completed) AND (确定性 verify 通过)。三者满足才标记 Completed，否则继续循环直到 max_turns 转 MaxTurns(interrupt 语义)。绝不引入文本/正则匹配判停。turn.rs:198 tool_calls.is_empty() 作为兜底 Final 但需在 plan 存在时被 task.complete 校验否决。
- 状态机（主 agent 编排器，落 slab-agent 编排核心，仅状态/循环/分支，不含规划逻辑）：核心 TurnOutcome 已有（turn.rs:52 Final/ToolCalls），扩展为 ThreadTermination { reason: TerminationReason, plan_summary, artifact_refs }。主循环 thread.rs:248 每轮：running -> (replan 分支?) -> tool_calls/replan -> 检测重复 -> 检测 task.complete -> 检测 budget -> 继续/interrupt/final。DAG/Plan/Task/Verification 的 Rust 结构草图放 slab-agent-tools（见下文 open_questions 待确认 crate 归属）。
- Rust 结构草图（slab-agent-tools，确定性工具层）：Plan { task_id: String, summary: Option<String>, nodes: Vec<PlanNode>, edges: Vec<(String,String)> // depends_on DAG }, PlanNode { id, title, status: PlanNodeStatus{Pending|InProgress|Completed|Blocked}, depends_on: Vec<String>, result_ref: Option<ArtifactRef>, verification: Option<VerificationResult> }, ArtifactRef { kind: File|Report|Media, path: PathBuf, digest: String }, VerificationResult { passed: bool, checks: Vec<Check>, ran_at }. task.complete(summary, artifact_refs, followup_actions: Vec<FollowupAction>) 为判停工具。
- 循环检测判据与处置：签名 = sha256(canonical(tool_name, args_subset))，环形缓冲近 N=3 轮；连续命中阈值=2 判 stuck（对 read_file/grep 等只读工具豁免）。处置阶梯：1) 命中 -> interrupt（control.rs:381，保留线程可续跑）-> 2) 回灌 stuck 原因给 LLM 换策略一次 -> 3) 再次命中 -> escalate 到人（人工介入，非 shutdown）。绝不直接杀进程。
- a2u 受控工具集清单与权限映射：workspace.open(allow)/image.edit(ask)/image.generate(ask)/audio.transcribe(ask)/video.generate(ask)/plugin.launch(ask, 走 WebView label 推导 caller id 红线)/hub.browse(allow)/review.show(allow)/task.complete(allow, 确定性校验)。无任意像素操控工具（computer_use_*）。每个工具副作用域限于'打开受信 host UI 面/读写本地 sandbox 文件/调 /v1 API'。
- 上下文隔离具体实现：subagent.rs:108 已隔离（全新 messages 只含委派 task、独立 thread_id、parent_id 回链、继承 config 可覆盖、allowed_tools 白名单 subagent.rs:29）。强化点：1) 主 agent 上下文绝不整段透传给 sub（只传 task 字符串 + 必要引用）；2) sub 只回 ToolOutput（结构化 result + artifact 路径引用），不回原始 messages；3) artifact 写 .slab/，主 agent 拿引用；4) 只读校验类子任务配 read-only allowed_tools 白名单。

### 风险
- DAG/规划逻辑被错误地塞进 slab-agent 编排核心（违反 AGENTS.md 红线：slab-agent 保持纯编排）。缓解：DAG 数据结构、task.complete、replan、verify 全部作为 ToolHandler 落在 slab-agent-tools；slab-agent 只在 thread.rs 循环加 replan 分支与 memory 读写，规划逻辑零行入编排核心。代码评审强校验此边界。
- task.complete 被模型误用过早调用（没真正完成就调），导致 default-deny 反而误停。缓解：task.complete 内部校验 plan 全节点 completed + 跑确定性 verify 工具通过；不通过则返回错误回灌 LLM 继续。这把'判定权'从模型手里夺回给确定性逻辑。
- 循环/重复检测误判（合理的重试被判 stuck）。缓解：签名只哈希 (tool_name, 关键 args 规范化)，对 read_file/grep 这类只读探索工具豁免或提高阈值；命中先 interrupt 而非直接判失败，给人工介入机会。
- 多 agent 烧 token（15x）。缓解：默认单 agent ReAct，prompt 工程层按复杂度缩放，并发/深度上限已有（control.rs:54）。P2 预算看板兜底。明确不在简单查询场景 spawn 子 agent。
- a2u 工具集膨胀失控（向 VS Code 128 工具靠拢）。缓解：刻意高阶、命名空间化、合并链式多步；每个工具配 eval；反模式清单明确禁止低阶原语（open_tab/scroll/click_xy）。
- subagent artifact 落盘引入新的文件系统副作用面，可能绕过 workspace sandbox 边界。缓解：artifact 只写 .slab/ 工作目录（已是 workspace 模式的工作目录），路径由工具侧拼接、不直接接受 LLM 任意路径，遵守 Tauri capabilities。
- max_turns 命中走 interrupt 保留线程，但若没有 resume 协议会成半截工程。缓解：interrupt 原因回灌、续传 turn_index 必须与 P0 终止理由结构化同步交付，否则不验收。

### 边界红线注意
- AGENTS.md 红线：slab-agent 保持纯编排。DAG/Plan/Task/Verification/task.complete/replan/verify/guardrail 全部必须落在 slab-agent-tools（确定性工具层），slab-agent 只能加 replan 分支与 memory 读写、循环检测的状态机骨架。任何把规划逻辑塞进 thread.rs/turn.rs 的实现即否决。
- AGENTS.md 红线：插件/API 能力由 host/app-core 注册。a2u 工具（workspace.open/image.edit/plugin.launch 等）必须作为 host（slab-desktop / app-core）注册的 ToolHandler 注入 ToolRouter，不能硬编码进 slab-agent-tools 的内置工具集（除 task.complete/plan 这类通用确定性工具）。
- AGENTS.md 红线：plugin WebView caller id 从 label 推导。plugin.launch a2u 工具调用插件时必须走现有 label 推导契约，不能绕过。a2u 渲染面也必须遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱边界，不能用任意 origin inline 内容（Claude Artifacts iOS 渲染失败教训）。
- AGENTS.md 红线：不新增平行 API 树，只扩 /v1/*。task.complete/followup_actions 若需服务端持久化（plan 状态、artifact 引用），走 /v1/sessions 或新 /v1/plan 端点，shape 变要 bun run gen:api；SQLx migration 只追加。
- AGENTS.md 红线：桌面 host 不新增第二套 LSP bridge。若 verify 工具要对编码产物做语法检查，走已有 /v1/workspace/lsp/{language} -> app-core 路径，不能在 agent 侧另起 LSP 客户端。
- 不照搬 ADK SequentialAgent/LoopAgent/ParallelAgent 声明式 agent 类体系——Slab 是 Rust 命令式 + slab-agent 纯编排，只借鉴'显式终止条件''artifact 解耦'思想。不引入 A2A 跨厂商互操作协议（外部 agent 不在 Slab 本地优先信任模型内）。
- 坚决不做 Computer Use（截图-坐标路线）：Slab 有完整 /v1/* API 和受控工具集，截图路线违反隐私优先、烧 token、坐标不可靠。a2u 工具副作用域严格限于'打开受信 host UI 面/读写本地 sandbox 文件/调 /v1 API'，绝不暴露任意像素操控。
- To-Be 新增文件需显式标注边界归属：task_complete.rs / plan_dag.rs / verify.rs / repetition_guard.rs（建议）均落 crates/slab-agent-tools/src/；replan 分支与 TerminationReason 状态机骨架落 crates/slab-agent/src/thread.rs 与 turn.rs（编排核心，仅状态/循环）；a2u 派发表与 followup action 渲染落 packages/slab-desktop/src/pages/assistant/（host 注册）。任何越界归属需破例论证。

### 开放问题
- Plan/DAG 状态的持久化载体：放 slab-agent-memories（已有 crate）还是给 AgentStorePort 新增 plan 字段？倾向前者（符合边界、防 200K 截断），但需确认 memories crate 是否支持按 thread_id 读写结构化 plan（需读 slab-agent-memories 实际接口）。
- task.complete 的 verify 子步是否对所有任务类型强制？编码类强制编译/lint 没争议，但'写文案/生成图片'类任务的确定性 verify 标准难定（图片质量难确定性判定）。是否对不同 plan node 类型配可插拔 verifier，未配置 verifier 的节点用'人工审阅'标记而非自动判定？
- 循环检测的 args_subset 规范化规则：read_file 路径相同算重复，但 grep 同 pattern 不同 path 算不算？需要明确各类工具的'关键 args'抽取规则，否则误判。建议每工具自带 canonical_args() 方法。
- max_turns 命中走 interrupt 后的 resume 协议：续传从哪个 turn_index、是否把 interrupt 原因（重复/budget）作为 system message 回灌给 LLM、用户能否手动改 plan 后续跑？需产品+前端同步定义，否则兜底不闭环。
- 并行委派（一次 spawn 多个独立子 agent）是否纳入本期？当前 delegate_subagent 同步阻塞，并行需要改工具签名（接受 tasks: Vec）+ 前端展示。对'广度探索'价值高但对'编码'价值低，是否按任务类型 gate？
- a2u 工具的触发：纯靠 LLM 决定调哪个 a2u 工具，还是 host 侧基于'输出形态判据'（生成媒体/长代码/多文件 diff）自动注入提示？Canvas 范式建议后者（确定性判据 + 用户显式覆盖），但具体判据阈值（>10 行？多文件？）需产品定义。
- followup_actions（打开项目/人工审阅/feedback）与现有 Assistant 跨页跳转（store/useAssistantDraftStore）如何对接？是否复用同一跳转机制，还是 task.complete 产出独立的 action schema 由前端新派发表消费？
- PluginWebView caller id 必须由 WebView label 推导（红线）——plugin.launch a2u 工具触发插件 WebView 时，caller id 的推导链路是否已覆盖此工具调用路径？需确认 plugin crate 现有 label 推导逻辑能否复用，避免破例。

---
