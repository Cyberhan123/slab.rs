# 附录 A · 外网研究摘要（Research Digest）

> 本附录是多 agent 会议 Phase 1 的两份外网研究结构化摘要，作为会议结论与各 TD 的研究依据。所有结论均以现有源码为准，研究结论仅作设计参考。

---

## 研究 1 · 生产级多 agent 编排系统调研：主 agent 规划/DAG/校验、subagent 隔离、终止纪律、循环兜底（2026-06-26，面向 Slab Next）

### 关键发现

- **来源**：Anthropic《How we built our multi-agent research system》https://www.anthropic.com/engineering/multi-agent-research-system
  - **洞察**：orchestrator-worker 架构：lead agent 分析查询→制定策略→并行 spawn 3-5 个 subagent。每个 subagent 自带 objective/output_format/工具集/任务边界四要素。lead agent 持久化 plan 到 Memory（因 200K token 会被截断），subagent 用 interleaved thinking 评估工具结果、回传压缩摘要而非原始输出，lead 综合后决定是否继续。token 用量解释 BrowseComp 80% 方差——多 agent 本质是‘用足够 token 解决问题’。
  - **对 Slab 的借鉴**：Slab 当前 plan_update（crates/slab-agent-tools/src/plan.rs）只 normalize 回显，无持久化/无边界四要素。可直接借鉴：把 plan 持久化进 slab-agent-memories（防 200K 截断丢计划），给 delegate_subagent（subagent.rs:21 的 DelegateSubagentArgs）显式补 output_format 字段，主循环‘综合后判断是否再 spawn’对应 thread.rs:248 的 for turn 循环需要新增 replan 分支。
- **来源**：Anthropic《Building effective agents》https://www.anthropic.com/research/building-effective-agents
  - **洞察**：定义 5 种 workflow 模式 + Agent 循环。其中 orchestrator-workers 适合‘子任务无法预先确定’的复杂场景；evaluator-optimizer 是‘一 LLM 生成、另一 LLM 评判反馈’的循环，直到满足质量标准或达最大迭代。明确反对：agent 不要用于可预测线性流程（那时 prompt chaining 更可靠）。
  - **对 Slab 的借鉴**：Slab 是纯 ReAct（turn.rs:57 单 turn、thread.rs:248 循环），缺 plan-and-execute 的 replan 与 evaluator-optimizer 的结果校验。可在 slab-agent 内部新增轻量 plan_phase（生成/重规划）与可选 verify_phase（对最终产物跑确定性校验，如编译/diff/lint），不必引入第二个模型。
- **来源**：Anthropic 同上 + Plan-and-Execute 学术对照（LangGraph tutorial / arXiv:2509.08646 / The AI Engineer 四模式对比 https://theaiengineer.substack.com/p/the-4-single-agent-patterns）
  - **洞察**：Plan-and-Execute 三段式：Planner LLM 生成有编号任务清单/DAG → Executor（小模型）逐步执行 → Replanner 拿执行结果动态重规划或产出最终答复。关键区别：ReAct 永远只见眼前一步，Plan-and-Execute 先看全局。Replan 在工具报错/返回空时触发，多数任务可逼近 ReWOO 速度。ToT 用深度/广度上限 + 状态评估阈值做搜索终止。
  - **对 Slab 的借鉴**：Slab 用户明确要‘DAG + 全局规划 + 恢复’。借鉴点：plan_update 工具升级为带依赖边（DAG）的 plan 数据结构 + 一个 mark_done(task_id)/replan 接口；执行结果回填 plan 节点供 replan。DAG 仅作为 plan 工具内部表示（确定性逻辑，符合 slab-agent-tools 归属），不入 slab-agent 编排核心。
- **来源**：Claude Code subagents 官方文档 https://code.claude.com/docs/en/sub-agents + Task vs subagent 对比 https://amitkoth.com/claude-code-task-tool-vs-subagents/ + Reddit 嵌套深度 https://www.reddit.com/r/ClaudeAI/comments/1u71d27/
  - **洞察**：隔离模型：每个 subagent/Task 拥有独立 200K context、独立 system prompt、独立工具白名单，只接收父 agent 显式传入的内容；嵌套深度上限 5（2025-06-10 上线）；从更浅 context resume 不会绕过已触发的深度上限；subagent 是‘研究人员而非编码者’——上下文隔离在保护父 agent 的同时也会让子 agent 缺少完整历史。
  - **对 Slab 的借鉴**：Slab subagent.rs 已隔离（独立 thread_id、全新对话、parent_id 回链）且深度上限 4、并发上限 32——方向正确，与 Claude Code 一致。可借鉴：显式 allowed_tools 白名单（已有 allowed_tools 字段 subagent.rs:29）、为只读校验类子任务配置 read-only 工具集；避免反模式：不要让编码子 agent 做‘修改主仓库’类需要全局历史的任务（隔离会害它）。
- **来源**：OpenAI Agents SDK《Handoffs》https://openai.github.io/openai-agents-python/handoffs/ + Swarm repo https://github.com/openai/swarm + 发布 https://openai.com/index/new-tools-for-building-agents/
  - **洞察**：两大原语：Handoffs（显式控制权转移，A 把会话交给 B，可带 input filter/前置逻辑）与 Guardrails（input/output 校验，与 agent 并行跑，失败可 trip 中止）。Tracing 内置记录 generation/tool_call/handoff/guardrail 全链路。Swarm 哲学：handoff 必须显式、可测、可调试（反对隐式协作）。
  - **对 Slab 的借鉴**：Slab delegate_subagent 是‘委派并等待结果’（同步），更接近 orchestrator-worker 而非 handoff（控制权不永久转移）。可借鉴 Guardrails 思路：在 turn_tool_call.rs:418 审批门之外，对工具输出加确定性 output guardrail（如 write_file 后自动 lint）。Tracing 对应 slab-agent-tracing——已有，可补 handoff/校验事件埋点。
- **来源**：OpenAI 同上 + arXiv:2407.20859（通过无限循环的对抗攻击）https://arxiv.org/html/2407.20859v1 + Hermes-agent SHA-256 重复检测 https://github.com/NousResearch/hermes-agent/issues/481 + Reddit ‘max turns + human in loop’ https://www.reddit.com/r/AI_Agents/comments/1qnavt9/
  - **洞察**：循环/重复检测工业实践：(a) max-turns 硬上限 + 命中后 human-in-the-loop（非直接杀进程）；(b) 对工具调用序列做哈希/签名（如 SHA-256 of (tool, args)），连续重复命中阈值即判 stuck；(c) Loop Drift——即便有 stop 条件 agent 仍可能漂移，需预算（token/成本/时间）三重兜底；(d) 对抗场景下攻击者诱导 agent 重复命令直到 max_iter。
  - **对 Slab 的借鉴**：Slab 目前只有 max_turns 硬上限（thread.rs:248 的 0..max_turns）+ invalid_tool_call_retries + 并发/深度上限，无重复检测。明确缺口：可加一个轻量‘近 N 轮工具签名去重’计数器，命中阈值触发 interrupt（control.rs:381 已解耦的 interrupt，保留线程可继续）而非 shutdown；并复用 slab-agent-tracing 做预算（turn/token/工具调用次数）看板。
- **来源**：Google ADK LoopAgent/SequentialAgent/ParallelAgent https://adk.dev/agents/workflow-agents/loop-agents/ + 8 模式指南 https://developers.googleblog.com/developers-guide-to-multi-agent-patterns-in-adk/
  - **洞察**：ADK 提供 3 个声明式 workflow agent：SequentialAgent（顺序）、LoopAgent（N 次或直到终止条件）、ParallelAgent（并行），可嵌套（Sequential in Loop）。终止条件对 LoopAgent 是显式 max_iterations 或 termination 条件——即把‘循环纪律’提到一等公民。
  - **对 Slab 的借鉴**：Slab 是命令式 for 循环，LoopAgent 的‘显式终止条件’思想可借鉴：把 max_turns 从裸常量升级为带‘终止理由’的结构（完成/超限/重复/用户中止），并暴露给 tracing。但 Slab 不应照搬 ADK 声明式 agent 类——违反 slab-agent 纯编排且 Slab 是 Rust 命令式，应只在 plan 工具层表达循环意图。
- **来源**：Google A2A 协议 https://developers.googleblog.com/en/a2a-a-new-era-of-agent-interoperability/ + ADK↔A2A 转换 https://cloud.google.com/blog/products/ai-machine-learning/unlock-ai-agent-collaboration-convert-adk-agents-for-a2a
  - **洞察**：A2A 是跨厂商/跨进程 agent 互操作开放协议（Agent Card、任务、artifact），用于让不同平台的 agent 安全协作。ADK 是‘建 agent’，A2A 是‘agent 之间通话’，MCP 是‘agent 与工具/数据’。
  - **对 Slab 的借鉴**：Slab 本地优先、隐私优先，A2A 这种跨主体互操作目前不在信任模型内（外部 agent 不可信）。可借鉴的仅是‘artifact 解耦’思想（subagent 把产物写文件系统，只回传引用，减少 telephone game）——这点 Anthropic 文章也强调，适合 Slab workspace 的 .slab/ 工作目录。不引入 A2A 协议本身。
- **来源**：Anthropic 同上（Multi-Agent Research System 末段‘End-state evaluation’与‘Subagent output to filesystem’）+ Reflexion 论文谱系（The AI Engineer 四模式）
  - **洞察**：结果校验/完成判定的两条正反经验：(1) 端态评估优于逐步评估——agent 可能走不同合法路径，应判‘最终状态对不对’而非‘是否走了预定步骤’，复杂工作流拆成离散 checkpoint；(2) 反模式：让同一个模型自评会‘自信地确认自己的错误’，evaluator 应是独立模型或确定性校验；(3) subagent 产物直写文件系统、只回传引用，减少信息损耗与 token。
  - **对 Slab 的借鉴**：用户要‘结果校验 + 任务完成判定’。借鉴：(1) 显式 task.complete 结构化完成判定（Slab 已是结构化 tool-call 判停 turn.rs:198，应加 task.complete 工具表达‘我已完成所有 plan 节点’）；(2) 校验用确定性工具（编译/测试/diff），不靠 LLM 自评；(3) 编码/媒体 subagent 把产物写 workspace，主 agent 拿引用。task.complete 属确定性工具，归 slab-agent-tools，符合边界。

### 可采纳模式
- 主 agent 升级为 plan-and-execute + replan：在 crates/slab-agent-tools/src/plan.rs 内把 plan 升级为带依赖边的 DAG（task_id + depends_on + status + result_ref），新增 mark_done(task_id) 与 replan(plan_patch) 接口；主循环 thread.rs:248 在每轮结束后允许 LLM 调 replan 触发动态重规划。DAG/规划是确定性数据结构，归 slab-agent-tools，不污染 slab-agent 纯编排。
- 持久化 plan 防 200K 截断：plan 写入 slab-agent-memories（Slab 已有该 crate），每轮开局从 memory 重载计划——直接对应 Anthropic ‘save plan to Memory because context will be truncated’ 的教训。
- 显式 task.complete 结构化完成判定：新增确定性工具 task.complete(summary, artifact_refs)，由 LLM 在所有 plan 节点 mark_done 后调用判停。保持并强化 Slab 已有的结构化判停（turn.rs:198 tool_calls.is_empty() 即 Final），坚决不引入文本/正则判停。
- delegate_subagent 补四要素：在 DelegateSubagentArgs（subagent.rs:21）已有 task/model/system_prompt/allowed_tools/max_turns 基础上，prompt 工程层强制 lead agent 给出 objective、output_format、工具与来源指引、明确任务边界（Anthropic 反复强调缺这些会重复劳动）。
- 端态 + checkpoint 结果校验：对编码/媒体类任务，完成后跑确定性校验工具（编译/lint/diff/媒体元数据），校验结果作为 plan 节点的 result_ref；避免用同一 LLM 自评（防‘自信确认错误’反模式）。
- subagent 产物落盘 + 回传引用：workspace 任务让 subagent 把文件/报告写入 .slab/ 工作目录，只回传路径引用给主 agent，减少 token 与 telephone game——契合 Slab workspace 模式（docs/development/workspace-mode-design.md）。
- 循环/重复检测兜底：新增‘近 N 轮工具调用签名去重’计数（对 (tool_name, 关键 args) 做哈希，连续重复命中阈值判 stuck），命中后走 interrupt（control.rs:381 已解耦、保留线程可继续 + 人工介入）而非直接 shutdown；叠加 token/turn/工具调用次数预算看板（复用 slab-agent-tracing）。
- 终止理由结构化：把 max_turns 命中从静默 break 升级为带 reason 的结构化终止（completed / max_turns / repetition_detected / budget_exhausted / interrupted），写入 tracing 与 session，便于前端展示‘为什么停’。
- 校验/完成判定的轻量 guardrail：在 turn_tool_call.rs:418 审批门之外，对高风险工具输出加确定性 output guardrail（如 write_file 后自动 lint、apply_patch 后自动语法检查），失败回灌给 LLM 重试（已有 invalid_tool_call_retries 预算可复用）。

### 应避免的反模式
- 文本/正则判停：用关键词匹配决定 agent 何时结束。Slab 当前已是结构化判停（turn.rs:198），保持住，绝不退回文本匹配——用户担心的问题代码里不存在，不要反向引入。
- 让同一 LLM 自评完成：evaluator 用与 generator 同一模型同一上下文会‘自信确认自己的错误’。Slab 的校验应走确定性工具（编译/测试/lint）或独立角色，不要用主 agent 自我宣布完成代替校验。
- 为多 agent 而多 agent：Anthropic 明确多 agent 比 chat 烧约 15× token，只在‘任务价值足够高 + 高度可并行 + 信息超单 context + 复杂工具’时才用；编码等依赖强的任务单 agent 常更优。Slab 默认应单 agent ReAct，仅在研究/广度探索/产物隔离场景才 spawn 子 agent。
- 隐式协作/隐式 handoff：OpenAI Swarm 反复强调 handoff 必须显式、可测、可调试。Slab delegate_subagent 已是显式工具调用，勿引入‘agent 之间暗中互相触发’的隐式控制流。
- 照搬 ADK/A2A 声明式框架或跨主体协议：Slab 是 Rust 命令式 + 本地优先隐私模型，ADK 的 SequentialAgent/LoopAgent 类体系与 A2A 跨厂商互操作都不符合 slab-agent 纯编排边界与信任模型。只借鉴‘显式终止条件’‘artifact 解耦’思想，不引入协议本身。
- 把 DAG/规划/校验塞进 slab-agent 编排核心：违反 AGENTS.md 红线（slab-agent 保持纯编排，确定性能力进 slab-agent-tools）。DAG 数据结构、task.complete、verify 工具必须落在 slab-agent-tools，由 host/app-core 注册。
- max_turns 硬杀无恢复：命中上限直接杀进程会让长任务前功尽弃（Anthropic：‘restarts are expensive and frustrating’）。应像 interrupt 那样保留线程 + 检查点 + 可续跑，配合 human-in-the-loop，而非硬终止。
- subagent 做需要全局历史的编码修改任务：Claude Code 社区共识 subagent 是‘researchers not coders’，隔离上下文会让它缺少完整历史而误改。Slab 编码类主仓库修改应留在主线程，仅把只读研究/广度探索/独立产物生成委派给 subagent。
- 无限并发 spawn：Anthropic 早期教训是‘为简单查询 spawn 50 个子 agent’。Slab 已有并发上限 32 + 深度 4，还应再加‘按查询复杂度缩放子 agent 数’的 prompt 规则（事实类 1 个/3-10 次调用，对比类 2-4 个，复杂研究 10+），防过度投资。

### 参考来源
- https://www.anthropic.com/research/building-effective-agents
- https://www.anthropic.com/engineering/multi-agent-research-system
- https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- https://code.claude.com/docs/en/sub-agents
- https://code.claude.com/docs/en/agents
- https://openai.github.io/openai-agents-python/handoffs/
- https://openai.github.io/openai-agents-python/agents/
- https://openai.com/index/new-tools-for-building-agents/
- https://github.com/openai/swarm
- https://adk.dev/agents/workflow-agents/loop-agents/
- https://adk.dev/agents/workflow-agents/sequential-agents/
- https://developers.googleblog.com/developers-guide-to-multi-agent-patterns-in-adk/
- https://developers.googleblog.com/en/a2a-a-new-era-of-agent-interoperability/
- https://cloud.google.com/blog/products/ai-machine-learning/unlock-ai-agent-collaboration-convert-adk-agents-for-a2a
- https://www.langchain.com/blog/planning-agents
- https://theaiengineer.substack.com/p/the-4-single-agent-patterns
- https://arxiv.org/pdf/2509.08646
- https://arxiv.org/html/2407.20859v1
- https://github.com/NousResearch/hermes-agent/issues/481
- https://www.reddit.com/r/AI_Agents/comments/1qnavt9/the_infinite_loop_fear_is_real_how_are_you_preventing_your/
- https://www.promptingguide.ai/techniques/react
- https://amitkoth.com/claude-code-task-tool-vs-subagents/

---

## 研究 2 · Agent 驱动 UI（agentic UI / agent-to-UI）：tool-call 驱动的 UI 打开/导航/渲染机制，及其对 Slab「最小化 a2u」的可借鉴点（外网调研，2026-06-26）

### 关键发现

- **来源**：https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces
  - **洞察**：Vercel AI SDK 的 Generative UI 是把「tool call 的结果」直接映射到一个 React 组件来渲染。机制：你给模型一组 tools，模型决定调用某个 tool（如 displayWeather），tool.execute() 返回结构化数据，前端在 UIMessage.parts 里识别 `part.type === 'tool-<toolName>'` 并按 `part.state`（input-available / output-available / output-error）切换渲染组件。注意：tool 本身只产出数据，渲染哪个 React 组件是前端固定的派发表（tool 名 → 组件），不是模型生成任意 UI。
  - **对 Slab 的借鉴**：这是 Slab「最小化 a2u」最贴近的范式。Slab 已有 tool_call 事件流（packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts 里已处理 tool_call），且 Assistant 已是 index route（packages/slab-desktop/src/routes/index.tsx）。可借鉴：把每个「受控 UI 工具」（如 open_workspace / open_image_editor / review_changes / open_plugin）定义成一个确定性工具，在 tool_call 事件处加一个 tool 名 → React 组件/动作 的前端派发表，让 agent 通过结构化 tool call 来打开/渲染 UI 面，而不是让用户去点侧边栏。模型只决定「调哪个工具 + 参数」，UI 组件库由 host 固定，符合「不做完整 Computer Use」的红线。
- **来源**：https://www.anthropic.com/engineering/writing-tools-for-agents
  - **洞察**：Anthropic 的 ACI（agent-computer interface）核心论点：工具是「确定性系统与非确定性 agent 之间的契约」，要为 agent 而不是为人类 API 调用者来设计。关键原则：(1) 选择正确的工具——把高频链式多步操作合并成单个高阶工具（如 schedule_event 内部封装 list_users+list_events+create_event），用 search_logs 而非 read_logs；(2) 命名空间化工具（asana_search / jira_search）减少 agent 在上百个工具间的混淆；(3) 工具返回高信号、低 token 的上下文，把 UUID 解析成自然语言名，避免 brute-force 返回全部数据；(4) 提供 response_format=concise|detailed 让 agent 自控粒度；(5) 用 eval 驱动 tool 描述的 prompt-engineering，小改描述能显著降错。
  - **对 Slab 的借鉴**：直接指导 Slab 的 a2u 工具集设计。当前 plan.rs 只回显计划、turn.rs:198 用 tool_calls.is_empty() 判完成。可借鉴：(a) 不要为每个原页面都暴露低阶 list/open 工具，而应做高阶「任务式」UI 工具——例如 open_project_for_review(workspace_id, focus_files[]) 一个工具内部完成导航+定位+渲染审阅面，而不是 open_tab + select_panel + scroll 三步；(b) 工具名按域命名空间（workspace.open / image.edit / plugin.launch）；(c) 工具返回自然语言摘要 + 可选 ID，喂回给 agent 的 TurnOutcome；(d) 给每个 a2u 工具配 eval（真实多轮任务），用结果反推 tool 描述。
- **来源**：https://code.visualstudio.com/docs/copilot/agents/agent-tools
  - **洞察**：VS Code agent mode 的 UI 注入机制：(1) 工具按三类注册——built-in / MCP server / extension(LM Tools API)；(2) 每个请求可用 tools picker 按需启用/禁用工具（默认折叠 tool call 细节，chat.agent.thinking.collapsedTools 控制）；(3) 用户可编辑 tool 入参后才 Allow（参数可改）；(4) terminal tool 把命令输出内联到 chat（Show Output '>' / Show Terminal），长命令可 Continue in Background 并设 timeout；(5) 工具集（tool set）把相关工具打包成一个 `#name` 引用；(6) 有 128 tools/request 上限，超了用 virtual tools 阈值自动收窄。
  - **对 Slab 的借鉴**：高度可借鉴 Slab 现有结构：Slab 已有审批门（turn_tool_call.rs:418 每个 tool 阻塞等 ApprovalPort），VS Code 的「参数可编辑后才 Allow」正好对应 Slab 的 ApprovalPort 决策。可借鉴：(a) tool call 细节默认折叠、可展开——符合「最小化 UI」；(b) 把插件 contributes.agentCapabilities[].exposeAsMcpTool 与 VS Code 的「extension tool / tool set」对应，让插件以「工具集」形式注入而非整页 WebView；(c) 长任务（diff/构建/视频生成）用 background + timeout 模式，避免阻塞 agent loop。避免：把 128 工具上限当默认目标——Slab 应刻意少而高阶。
- **来源**：https://cursor.com/blog/agent-best-practices
  - **洞察**：Cursor 的 agent harness 三件套：Instructions(系统提示+rules) / Tools(文件编辑/搜索/终端) / Model，并按模型分别调参。最相关 a2u 机制：(1) Plan Mode（Shift+Tab）——agent 先 research codebase、问澄清问题、产出 markdown 计划文件（含文件路径+代码引用）等用户批准后才动手，计划可存 .cursor/plans/ 续作；(2) Commands（.cursor/commands/*.md）——把「多步工作流」封装成 /pr /fix-issue /review 等单次 `/` 触发，agent 自主跑全流程；(3) Skills + hooks（.cursor/hooks.json stop hook）——agent 停下时 hook 返回 followup_message 强制继续循环，直到 scratchpad 写 DONE 或达到 MAX_ITERATIONS，实现「跑到测试通过为止」；(4) 完成后 Review→Find Issues 做专门的 review pass，diff 实时可见、Stop 可中断；(5) 并行多 agent + git worktree 隔离。
  - **对 Slab 的借鉴**：直击 Slab 用户痛点 4（多轮任务支撑）和痛点 1（统一入口）。可借鉴：(a) Plan Mode 正是 Slab plan.rs 应升级的方向——把 plan.rs 从「回显 todo」升级成「产出可审阅计划（含文件/资源引用）→ 用户批准 → 才执行」，对应共享上下文里「主 agent 全局规划、结果校验、完成判定」诉求；(b) Commands 模式让 Slab 把「打开项目+跑任务+总结」编成可复用流程，任务总结处给出「打开项目/人工审阅/feedback 调整」三个动作按钮；(c) stop-hook 续跑循环对应「全局 max turns + 重复检测」兜底。避免：Cursor 的 YOLO mode（全自动无确认）违反 Slab 审批门设计，不可照搬。
- **来源**：https://openai.com/index/introducing-canvas/
  - **洞察**：ChatGPT Canvas 的触发机制：Canvas 不是每次都开，是「ChatGPT 检测到任务适合」时自动打开（如输出 >10 行、需要迭代编辑的写作/编码），用户也可在 prompt 里写 'use canvas' 强制打开。它本质是一个「专门的 side-panel 编辑面」，由模型的「打开 canvas」这个隐式动作触发，用于超过简单 chat 的内容。
  - **对 Slab 的借鉴**：这是「agent 决定何时打开专门 UI 面」的范本。对 Slab：与其让用户手动点 Image/Video/Workspace 页，不如让 agent 在「检测到任务适合」（如要生成图片、要打开项目编码、要审阅 diff）时，通过一个 open_canvas(surface, payload) 式的结构化 tool call 自动打开对应面。触发判据应是「输出形态」（生成二进制媒体 / 长代码 / 多文件改动）而非 LLM 主观意愿，且用户能像 'use canvas' 一样显式覆盖。这呼应共享上下文里「原页面退化为子窗口/新 tab 式集成」。
- **来源**：https://medium.com/@nuno.roberto/claude-artifacts-turning-chat-into-shareable-software-4985fdba94a2
  - **洞察**：Claude Artifacts 不是每条消息都触发：内部启发式判断内容是否值得单独 panel（代码块/SVG/HTML/React 组件/Mermaid）。触发时在 chat 旁开 side panel 渲染 live、可交互的 HTML/React/SVG/Mermaid。底层是模型输出特殊结构块，web client 解析后渲染。iOS 上因 WebView origin 不匹配会渲染失败——说明「渲染面」对宿主环境/origin 敏感。
  - **对 Slab 的借鉴**：印证「agent 通过结构化输出触发专门 UI 面」是成熟范式。对 Slab 的具体借鉴：(a) 让 agent 在 tool call 的 output 里产出「artifact 描述」（类型 + 内容指针），前端按类型派发到预渲染面（代码→workspace、图→image 预览、diff→review 面）；(b) origin/WebView 敏感这点警示 Slab：插件 WebView 的 caller id 必须从 label 推导（已是红线），artifact 渲染面也要遵循 Tauri CSP/capabilities 边界。反模式：不要让模型自由生成任意 HTML/JS 渲染（= Computer Use 的变体），渲染面必须是 host 固定的受信组件。
- **来源**：https://workos.com/blog/anthropics-computer-use-versus-openais-computer-using-agent-cua
  - **洞察**：Anthropic Computer Use 与 OpenAI CUA 都走「截图 + 鼠标键盘坐标动作」路线：模型看截图→输出 action(screenshot/click/move/type/scroll)→宿主执行→回传新截图，ReAct 循环。代价：截图吃 token 巨大、坐标定位不可靠、速度慢、成本高；建议「避免不必要的截图」「缓存重复/静态界面元素」。它们解决的是「无 API 的通用 GUI」自动化问题。
  - **对 Slab 的借鉴**：这是 Slab 要明确「不做」的反面参照。Slab 有完整 /v1/* API 和受控工具集，没必要也不应走截图-坐标路线。共享上下文里「不做完整 a2u，而是通过 tool call 注入受控工具集」正是对这两种 Computer Use 的刻意规避。可借鉴的边界：(a) 仅当存在「无 API 的外部 GUI」（如某些网页插件）时才考虑受限的 browser tool，且默认 ask 审批；(b) 主路径永远是结构化 tool call + 确定性 host 渲染。
- **来源**：https://cursor.com/changelog/0-46-x
  - **洞察**：Cursor v0.46 把 Chat/Composer/Agent 三模式统一成单一 Agent，并支持 YOLO mode（自动跑 terminal/MCP 无确认）、browser tool（预览/测试）、.cursor/mcp.json 项目级 MCP 配置、MCP resources 作为上下文。Cursor 3.6 的 Auto-Review 把每个 shell/MCP/fetch 调用分类为 allow/sandbox/ask，大幅减少权限弹窗。
  - **对 Slab 的借鉴**：Cursor 的 allow/sandbox/ask 三态分类正好对应 Slab 已有的 ApprovalPort（turn_tool_call.rs:418），可强化为「按工具静态分级 + 动态门」：a2u 类打开/渲染工具默认 allow（无副作用），改动类 default ask，危险类 sandbox/拒绝。反模式：YOLO 全自动违反 Slab 本地优先+隐私优先的信任模型，不可作为默认。
- **来源**：https://www.anthropic.com/news/3-5-models-and-computer-use
  - **洞察**：Computer Use 本质是把「截屏/鼠标/键盘/读屏」定义成一组特殊 tool，让 LLM 在 ReAct 循环里调用——和普通 function calling 是同一机制，只是工具的副作用是「操控像素」。这印证：agentic UI 的底层都是 tool calling，区别只在工具的副作用域（结构化 API vs 像素操控）。
  - **对 Slab 的借鉴**：给 Slab 一个清晰的取舍框架：a2u 的「最小化」= 选择副作用域最窄、确定性最高的工具集。Slab 应只暴露副作用为「打开受信 host UI 面 / 读写本地受 sandbox 文件 / 调用 /v1 API」的工具，绝不暴露「操控任意像素」的工具。这样既复用 tool-calling 的成熟范式，又守住 Computer Use 的红线。

### 可采纳模式
- 采用「tool 名 → host 固定 React 组件/动作」的派发表范式（Vercel Generative UI / Claude Artifacts 范式）：agent 通过结构化 tool call（如 workspace.open / image.edit / plugin.launch / review.show）决定「打开哪个面 + 传什么 payload」，但渲染哪个组件、布局如何由 host 完全固定。在 packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts 已有的 tool_call 事件分支上加一个受控的「UI 工具」派发器。这满足「agent 打开/导航 UI，而非用户去点页面」，且不越「不做完整 Computer Use」红线。
- 把 a2u 工具做成「高阶、任务式、命名空间化」而非低阶原语（Anthropic ACI 原则）：用 open_project_for_review(workspace_id, focus_files[]) 这种内部封装导航+定位+渲染的工具，而不是 open_tab+select_panel+scroll。工具名按域前缀（workspace. / image. / audio. / video. / plugin. / hub.）。这降低 agent 在工具间迷失的概率，也减少 token 消耗。
- 升级 plan.rs（crates/slab-agent-tools/src/plan.rs）从「回显 todo」到 Plan Mode（Cursor 范式）：agent 先产出可审阅的结构化计划（含文件路径/资源引用 + 步骤 DAG + 完成判据），等用户（或 ApprovalPort）批准后才执行；计划持久化到 .slab/ 以便续作。同时引入显式 task.complete 结构化完成判定（强化 turn.rs:198 已有的 tool_calls.is_empty() 语义，避免依赖文本匹配判停——这点共享上下文已确认代码不存在文本判停，应保持）。
- 在任务总结处提供三个动作按钮（共享上下文产品北极星）：「打开项目（在 workspace/对应面打开 agent 产出物）」「人工审阅（进入 review/diff 面）」「feedback 调整（回写意见续跑 agent）」。每个按钮对应一个受控 a2u tool call 的逆操作（用户点击 → 触发 host 已知的导航/动作），而非任意跳转。这是 ChatGPT Canvas「自动打开专门面」+ Cursor「Review→Find Issues」范式的组合。
- 用 ChatGPT Canvas 的「自动触发判据」思路：a2u 打开专门面的触发应基于「输出形态/任务类型」（生成媒体/长代码/多文件 diff/需迭代编辑）而非 LLM 主观意愿，且始终允许用户显式覆盖（类似 'use canvas'）。触发逻辑放 host 侧（确定性），不在 agent 上下文里决策，保持可预测。
- 采用 Cursor/VS Code 的 allow/sandbox/ask 三态工具审批分级（对齐 Slab 已有 ApprovalPort，crates/slab-agent 中 turn_tool_call.rs:418）：a2u「打开/渲染/只读」类工具默认 allow（无副作用）；改动/执行类 default ask；危险/外部网络类 sandbox 或拒。减少权限弹窗同时守住本地优先+隐私优先信任模型。
- 把 a2u 工具事件流（tool_call）做成「默认折叠细节、可展开」的极简 UI（VS Code chat.agent.thinking.collapsedTools 范式）：tool call 只显示一行摘要（如「打开 workspace：slab.rs」），点击展开参数/结果。符合「最小化 UI」。
- 为每个 a2u 工具配 eval（Anthropic 范式）：用真实多轮任务（打开项目并定位文件、生成图片并打开预览、插件委派子 agent）测 tool 描述/参数设计的命中率，用结果反推工具描述的 prompt-engineering。

### 应避免的反模式
- 不要做截图-坐标驱动的 Computer Use（Anthropic Computer Use / OpenAI CUA 路线）：Slab 有完整 /v1/* API 和受控工具集，走截图路线会带来高 token 成本、坐标不可靠、违反隐私优先。仅当面对「无 API 的外部 GUI」时才考虑受限 browser tool，且默认 ask 审批。这违反共享上下文边界红线（不做完整 a2u）。
- 不要让模型自由生成任意 HTML/JS/React 渲染（= Computer Use 的变体）：Claude Artifacts 的 live HTML 渲染对开放网络 OK，但 Slab 是本地优先桌面，渲染面必须是 host 固定的受信组件，否则绕过 Tauri CSP/capabilities 红线。
- 不要照搬 Cursor YOLO mode（全自动无确认跑 terminal/MCP）：违反 Slab 审批门设计（turn_tool_call.rs:418）和本地信任模型。审批分级可以减少弹窗，但「全自动」不能成为默认。
- 不要为每个原页面都暴露低阶原语工具（open_tab / select_panel / scroll / click_xy）：Anthropic ACI 明确指出工具应高阶、命名空间化、合并链式多步。低阶原语会让 agent 在工具间迷失、token 浪费、动作序列脆弱。
- 不要让 agent 用文本「表示」任务完成来判断终止（共享上下文已确认 Slab 代码不存在文本判停，要保持）：必须用结构化 task.complete 工具或 tool_calls.is_empty() 语义（turn.rs:198）。plan.rs 当前只回显、不校验/不判完成，是待补的反模式状态。
- 不要让 a2u 工具的副作用域无界：每个工具的副作用必须限定在「打开受信 host UI 面 / 读写本地 sandbox 文件 / 调 /v1 API」之内，绝不触碰「操控任意像素 / 访问未授权资源」。插件 WebView caller id 必须从 label 推导（红线），a2u 工具调用插件时同样必须走此契约。
- 不要让 a2u 渲染面忽略 origin/宿主环境敏感性（Claude Artifacts iOS 渲染失败教训）：插件/动态面的渲染必须遵守 Tauri CSP/capabilities/permissions/sidecar 沙箱边界，不能用任意 origin 的 inline 内容。
- 不要把「打开专门面」的触发完全交给 LLM 主观判断（无判据）：应基于确定性输出形态判据 + 用户显式覆盖（Canvas 范式），否则会出现 agent 反复开关面、用户体验不可预测。
- 不要为追求功能堆叠而上 100+ 工具（VS Code 128 上限的警示）：Anthropic 指出工具贵精不贵多，重叠/低信号工具会分散 agent 注意力。Slab 应刻意保持 a2u 工具集小而高阶。

### 参考来源
- https://ai-sdk.dev/docs/ai-sdk-ui/generative-user-interfaces
- https://vercel.com/blog/ai-sdk-3-generative-ui
- https://www.anthropic.com/engineering/writing-tools-for-agents
- https://code.visualstudio.com/docs/copilot/agents/agent-tools
- https://code.visualstudio.com/blogs/2025/04/07/agentMode
- https://cursor.com/blog/agent-best-practices
- https://cursor.com/changelog/0-46-x
- https://openai.com/index/introducing-canvas/
- https://openai.com/index/computer-using-agent/
- https://medium.com/@nuno.roberto/claude-artifacts-turning-chat-into-shareable-software-4985fdba94a2
- https://workos.com/blog/anthropics-computer-use-versus-openais-computer-using-agent-cua
- https://www.anthropic.com/news/3-5-models-and-computer-use
- https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool
- https://www.digitalapplied.com/blog/anthropic-computer-use-api-guide
- https://www.digitalapplied.com/blog/cursor-2-0-agent-first-architecture-guide

---
