# 附录 C · 红队审查报告（Red-Team Report）

> 本附录是会议 Phase 3 的对抗性审查输出，已作为 must_add / must_cut 反向修正各 TD。默认怀疑立场。

## 总体判断

## 总体判断：**需调整后可推进 Phase 0-1，Phase 2-4 必须重做减法**

主席的事实层经我交叉验证基本诚实（turn.rs:198 结构化判停、thread.rs:424-451 假完成、plan.rs:91 只回显、subagent.rs:117 同步阻塞、risk.rs 仅识别 shell、bootstrap.rs:168 硬编码 32/4、routes 失效路径、AGENTS 红线全文）——**事实没有臆造**，这一点应予肯定。但综合结论有系统性偏差：

### 核心问题：按「云端研究级 agent 系统」的复杂度设计「本地桌面工具」
主席大量借鉴 Anthropic Multi-Agent Research System（DAG/replan/并行 subagent/200K context truncation）和 VS Code/ChatGPT Canvas 范式，但 Slab 是本地优先 + GLM workflow ~10-14 并发就 429（MEMORY）+ 用户 max_turns=10 的桌面场景。把云端跨小时研究系统的编排框架套到本地短任务上，是系统性过度工程。证据：non_goals 自己写「不默认就上多 agent」「多 agent 烧 15x token」，Phase 4 exit criteria 却列并行 subagent + 8-10 spawn——**自相矛盾**。

### 红线风险集中在三个新文件归属
1. **plugin.open ToolHandler**：文字承诺 host 层，但 a2u_surface 数据结构 + capability 注册路径若滑进 app-core，会把 slab-plugin 反向依赖引入 HTTP-free 业务核心。
2. **crates/slab-secrets（keyring）**：crates/ 下纯库引 OS keyring 副作用，破坏端口适配器分层。必须 bin/slab-app 或 bin/slab-runtime 实现 + crates/ 只放 SecretPort trait。
3. **ThreadStatus 新增 enum 变体**：跨层数据结构变更，回归面远超 Phase 2 时间盒。建议复用 status+reason 字符串，零 migration。

### 真正的隐藏地基（主席漏掉的）
主席把「结构化终止 + 循环兜底 + 假完成修复 + 能力可达性」列为四个隐藏地基，但**漏了三个更现实的**：
- **离线降级**（北极星硬要求，零 ADR）
- **敏感路径审批黑名单**（read 默认 allow 架空隐私红线）
- **per-thread token 硬预算**（runaway 成本失控，比循环检测更现实）

### 必须砍的
DAG/replan、AgentTimeline 可视化、并行 subagent、capabilities.available 独立工具、surface 分屏/浮窗、场景化 onboarding——这六项都是「为 5% 场景给 100% 任务加框架」，YAGNI。

### 必须加的
离线模式、敏感路径黑名单、token 预算、artifact_refs 路径校验、effects host 推断、interrupt 原子快照、任务级回滚——这七项是隐私/成本/数据一致性红线，不可缺。

### 推进建议
- **Phase 0 可推进**（契约收敛 + 日志 rotation + secret redaction 是无争议地基）
- **Phase 1 需调整**（surface 状态机降级为单 surface 切换，artifact_refs 路径校验进 exit criteria）
- **Phase 2 需重做减法**（砍 DAG/replan，ThreadStatus 不新增 enum 变体，循环检测阈值提到 3-4 且与 ADR-005 interrupt 语义先打通）
- **Phase 3-4 需重做**（并行 subagent 与 non_goals 冲突，secret store 归属违规，场景化 onboarding 砍掉）

**一句话**：主席的方案是一份诚实的工程蓝图，但被「大厂 agent 系统范式」绑架，本地桌面工具的简洁性被牺牲。砍掉 DAG/并行 subagent/可视化三座大山，补上离线/敏感路径/token 预算三个洞，才是守住「本地优先隐私优先离线可用」北极星的版本。


## 过度工程风险（建议砍）

- **ADR-003 DAG + replan 过度设计**：plan.rs:91 现状是 turn-by-turn ReAct，绝大多数用户任务（答疑/单文件改/一次 media 生成）不需要带依赖边的 DAG。Anthropic Multi-Agent Research System 的 DAG 是为「超长跨小时研究任务」设计的，Slab 是本地桌面工具，用户单任务 max_turns=10（[tool.rs 上下文] subagent 8）。把 plan 升级为「DAG + mark_done + replan(plan_patch) + 独立 namespace 持久化 + result_ref 回填」是按 5% 复杂任务场景给 100% 任务套上重编排框架。代价：tool 表多两个工具、prompt 模板膨胀、每次都要走 plan 序列化、replan 分支测试面爆炸。**建议**：Phase 2 只做「task.complete default-deny + result_ref 回填到现有 normalize_plan」，DAG/replan 推迟到有真实长任务 case 再上，YAGNI。
- **ADR-006 循环检测 SHA-256 签名 + 阈值=2 + 只读豁免 + 阶梯处置**：近 N=3 + 阈值=2 意味着只要连续 2 轮触发同签名就 stuck——在本地优先场景，agent 反复 read_file 同一文件（看 diff→改→再看 diff）极易误报。即使只读豁免，副作用类（write_file 改同文件多版迭代、apply_patch 增量）阈值=2 几乎必误报。这是为「卡死 agent」的极端 case 给所有迭代式工作流装误报雷。**建议**：阈值提到 3-4 或只对「(tool+args 全等）」去重而非「关键 args 规范化」，且阶梯第一档直接「回灌 LLM 换策略」而非 interrupt（interrupt 已被 break 绕过，要先修 ADR-005 才有意义）。
- **ADR-007 ToolContext 扩容注入 workspace/session/记忆/计划句柄用 trait object**：tool.rs:17 现状三字段。主席承认这是「workspace-scoped context、plan 持久化、循环签名计算共同卡点」——但循环签名只需 thread 内 TurnOutcome 历史（已在 thread.rs scope），plan 持久化只需 thread_id（已有），workspace 句柄只在 workspace.open 这一个 a2u 工具需要。为一个工具给所有 ToolContext 挂 trait object 是过度泛化，会污染所有 ToolHandler 的构造/测试（主席自己列了 tests.rs/subagent.rs 全部要改）。**建议**：按需注入——a2u 类工具单独定义 WorkspaceScopedTool trait 或在 ToolContext 加单个 `Option<WorkspaceRef>`，不要上 trait object 注入一套句柄。
- **ADR-010 AgentTimeline 时间线组件 + DAG 节点图 + subagent 子时间线 + checkpoint**：thoughts(use-assistant-agent.ts:113) 现状是扁平 tool 流。给 max_turns=10 的任务画 DAG 节点图 + subagent 子时间线，是 ChatGPT Canvas 级别的 UI 工程，但本地任务规模根本撑不起一张有意义的 DAG。checkpoint 概念在没有真 DAG 的前提下是空中楼阁。**建议**：只做「任务总结三动作按钮 + 进度条（X/N plan items）」，AgentTimeline 推迟到真有 plan 节点数据后再做。
- **P5 capabilities.available(domain) 能力可达性发现工具 + a2u 引导**：新工具 + 返回「高信号低 token 摘要 + 可选 ID」+ Agent 主动告知「需先装 X」——这是一个新的 discovery 子系统。但本地装了什么模型/插件，manifest 和 plugin registry 已有数据，Agent 完全可以读现有 list_dir/plugins 列表推断。单独造一个 capabilities.available 工具是给 Agent 又开一条「自省通道」，增加 tool 表 + prompt 膨胀。**建议**：复用现有 mcp.list/plugins 列表 + 在 system prompt 注入能力清单，不新增工具。
- **Phase 4 并行 delegate_subagent(tasks: Vec) + 按复杂度缩放 prompt 规则（事实 1/对比 2-4/研究 8-10）**：主席自己在 non_goals 写「不默认就上多 agent」「多 agent 烧 15x token」，又在 Phase 4 把并行 subagent 列为 exit criteria——自相矛盾。本地优先 + GLM workflow ~10-14 并发就 429（MEMORY 索引）的约束下，spawn 8-10 个并行 subagent 几乎必触发限流。这是照搬 Anthropic 云端研究系统的范式到受限本地环境。**建议**：Phase 4 砍掉并行 subagent，只保留「单 subagent 同步委派 + 预算看板」。

## 边界红线违规警告

- **ADR-009 plugin.open ToolHandler 归属自相矛盾，有滑向 slab-agent-tools 的风险**：主席写「plugin.open 落 host 层不进 slab-agent-tools，守纯编排红线」，但 PluginCapabilityKind::a2u_surface 是确定性数据结构在 slab-types，pluginCall capability 注册成 agent 可见工具——「注册」这步若落在 app-core 注册表里（runtime.rs:48 refresh_memory_tools 模式），实际是把插件生命周期/sandbox 调度塞进 app-core。app-core 是 HTTP-free 业务核心，但插件 sandbox 生命周期属 host 职责（AGENTS.md:38 Tauri child WebViews 默认）。**红线风险**：plugin.open 的 ToolHandler 实现若引用 slab-plugin 的 registry/integrity/permission，会把插件运行时依赖反向引入 app-core，破坏分层。**要求**：plugin.open 必须在 bin/slab-app/src-tauri host 层实现，通过 host→app-core 的 port trait 注入，不能让 app-core 直接调 slab-plugin。会议结论文字承诺了但未在 To-Be 表里钉死 host 实现路径。
- **ADR-014 统一 secret store 新 crates/slab-secrets（keyring）的归属未论证充分**：主席建议新 crate「独立职责不污染 slab-config/slab-app-core」。但 keyring 跨平台二进制 + OS 集成是运行时副作用，crates/ 下纯库 crate 引入 OS keyring 依赖会污染纯逻辑层。AGENTS.md:28 slab-app-core HTTP-free、slab-runtime-core 只调度——keyring 调用属 host/runtime composition。**红线风险**：把 keyring 放 crates/slab-secrets 让任意 crate 可依赖，最终会被 slab-agent-tools/slab-plugin 反向引入。**要求**：secret store 实现必须在 bin/slab-app/src-tauri 或 bin/slab-runtime（composition root），crates/ 只放 SecretPort trait（纯 port），否则破坏端口适配器分层。
- **ADR-012 workspace 切换走 sidecar 优雅重启，session 快照写到 .slab/sessions 后 /v1/sessions 恢复——但 session 归属绑定到 project 未定义**：主席自己标为开放问题但把它放 Phase 3 exit criteria。若 session_id 跨 workspace 切换后在新 sidecar 进程里恢复，旧 workspace 的 thread 状态/plan/memory 会泄漏到新 workspace 的 Agent 上下文——这违反 workspace 隔离（docs workspace-mode-design.md 每个 workspace 独立 .slab/slab.db/sessions/models）。**红线风险**：session 跨 workspace 恢复若无 project 绑定，等于绕过 workspace 沙箱边界。**要求**：ADR-012 必须先钉死 session 与 project/workspace 的一对一绑定，不能放开放问题。
- **capability 走 surface-window-<surfaceId> label 推导（Phase 4 离窗化）触碰 AGENTS.md:42 caller-id-from-label 红线**：主席承认这是「与最小权限拉扯需安全评审」，但把通配 label 前缀（surface-window-*）写进 capability，等于一个插件 surface 拿到所有 surface-window-* label 的权限。AGENTS.md:42 明确「must derive the caller plugin id from the WebView label」——通配前缀让 caller id 不可靠推导。**要求**：Phase 4 离窗化前必须每个 surface 一个独立 label，禁止通配，否则违反 caller-id-from-label 红线。

## 被忽略的视角 / 失败模式

- **离线场景被严重低估**：北极星宣称「本地优先、离线可用」，但全会议没有一个 ADR 验证「断网状态下 AgentShell + a2u + plan + task.complete 全链路可用」。capabilities.available 若依赖查询云端模型可用性、web_search 工具、外部 MCP——离线时 Agent 会断在工具报错。**缺口**：必须有一个 ADR 定义「离线降级模式」——agent 启动时探测 provider 可达性，离线时自动收窄工具集 + 在 UI 标注「离线模式，部分能力不可用」。否则统一入口在飞机上直接变砖。
- **低配机（8GB/集成显卡）视角缺失**：主席多次提 16GB OOM 风险，但 Slab 作为桌面工具要跑 diffusion/whisper/candle——8GB 机器开一个 diffusion 推理 + 一个 Agent thread + 一个 WebView surface 就可能 OOM。**缺口**：没有 ADR 定义「资源预算准入」——启动大模型推理前检查剩余内存，超阈值拒绝并 a2u 引导用户降级到小模型或云端。诊断包内存快照要包含模型显存占用。
- **审批疲劳（approval fatigue）反方向风险**：主席在 ADR-008 把 read_file 这类「只读」默认 allow——但 read_file 读敏感文件（.ssh/id_rsa、.env、credentials.json）也 allow，等于把密钥喂给云端 provider。**缺口**：只读 ≠ 安全。必须有「敏感路径黑名单」——read_file 命中 ~/.ssh、.env、*credentials*、*token* 时强制 ask，与 ADR-008 的 allow 默认叠加。否则隐私优先红线被 read_file 默认 allow 架空。
- **插件恶意（malicious plugin）威胁模型缺失**：ADR-009 让插件声明 a2u_surface + effects，但 effects 字段「插件自报」（plugin.rs:290 effects: Vec<String>）。主席在 ADR-008 强调「插件不可自报风险等级」——却允许插件自报 effects，自相矛盾。恶意插件声明 effects=[] 实际在 surface 里 fetch 外网泄漏数据。**缺口**：effects 必须由 host 静态推断（基于 runtime 类型：js→sandbox、python→pyodisolate、wasm→extism），插件声明的 effects 只作 hint 不作信任依据。
- **多 agent 失控（runaway）的成本/token 爆炸兜底不完整**：主席提「多 agent 烧 15x token」+ ADR-013 并发预算，但**没有 token 硬预算**（BudgetExhausted 终止理由列了但无 ADR 定义预算来源）。本机 GLM workflow ~10-14 并发 429，但 token 计费是 provider 侧——Agent 不知道自己烧了多少 token。**缺口**：必须有「per-thread token 预算」ADR——LLM 调用累计 token 超 thread 预算触发 BudgetExhausted 终止。否则一个 runaway agent 一夜烧光用户 provider 配额。这是比循环检测更现实的成本失控风险。
- **workspace 切换数据丢失（race condition）**：ADR-012 切换前枚举 active thread→interrupt→写 session 快照→kill。但 interrupt 是异步的（cancellation.cancel() + 下一轮 turn 开头才检查），8s shutdown timeout（sidecar.rs:39）内若 thread 正在跑一个长 LLM 调用，interrupt 来不及生效就被 kill——session 快照写的是中断前的不一致状态。**缺口**：必须有「interrupt grace period + 快照原子性」——interrupt 后等待 thread 真正进入 Interrupted 状态（watch channel）才写快照，超时则中止切换而非强 kill。主席写了「任一步失败中止切换」但没定义 interrupt 的等待语义。
- **隐私数据流向标注的执行机制缺失**：主席在 P4 提「隐私数据流向在计划审阅/任务总结处标注本地/云端/外部 MCP」——但这要求每个工具声明自己的数据流向（local/cloud-provider/external-mcp）。现有 ToolSpec 没有 data_flow 字段。**缺口**：需在 ToolSpec/tool 注册时强制声明 data_destination，否则标注无从生成。这是横切改动，主席低估了。
- **用户误操作恢复（undo）视角缺失**：a2u 工具 write_file/apply_patch/git commit 默认 ask，但用户批准后若 agent 改错文件，没有一键 undo。现有 git 工具能回滚但要求 agent 主动调。**缺口**：任务总结的 review 按钮应支持「回滚本次任务所有文件改动」（基于 task 开启时的 git stash 快照），否则 agent 改坏一堆文件用户只能手动 git checkout。

## 可行性风险

- **ADR-005 ThreadStatus 扩 MaxTurnsReached/Stopped + state.rs 状态迁移表 + SQLx migration + 前端展示，回归面巨大**：ThreadStatus 是跨层枚举（slab-types→state.rs→store→前端→WS 事件）。新增状态意味着：state.rs 状态机所有 transition 分支要补、SQLx migration 要 ADD（AGENTS.md:32 只追加）、所有 serialize/deserialize 点、前端 TS 类型（gen:api）、WS 事件 schema、resume 逻辑。主席标为「跨层变更回归面大需 gen 同步」但低估了——这是 P0 不该动的核心数据结构。且 open question 7 自己问「是否需 SQLx migration 还是复用现有 status+reason」——说明方案未定就进 Phase 2 exit criteria。**风险**：MaxTurnsReached 若作为独立 enum 变体，所有 match ThreadStatus 的代码都要补分支，漏一处就 panic。**要求**：复用现有 status + reason 字符串字段（update_thread_status 已支持 Option<&str> reason），不新增 enum 变体，零 migration。
- **ADR-006 循环 guard 内联 slab-agent thread.rs 需架构签字 + 与 ADR-005 修复强耦合**：主席承认「interrupt 已被 break 'turns 绕过」——这意味着循环检测命中 interrupt 后，thread.rs 的 for 循环 break 路径会直接走 ThreadStatus::Completed/Errored 而非 Interrupted（thread.rs:424-451 已验证）。循环 guard 内联在 thread.rs 读 TurnOutcome，但 interrupt 的 cancellation 信号触发的是 `cancellation.is_cancelled() → interrupted=true; break`（thread.rs:255-258），这条路径已经在循环开头被检查。问题：循环 guard 检测到 stuck 后调 control.interrupt，interrupt 走 cancellation.cancel()，下一轮 turn 开头才检查 interrupted——但 guard 命中时是当前 turn 内，cancellation 要等下一轮才生效。**实现风险**：guard→interrupt→实际中断有 1 轮延迟，且若 guard 命中正好是最后一轮（turn_offset == max_turns-1），interrupt 永远不生效。**要求**：guard 必须能直接 break 'turns 并设置一个 stuck flag 走独立终止路径，不能依赖 interrupt 的异步 cancellation。
- **ADR-013 并发预算可配置 + FIFO 排队 + 内存熔断 + 冷却窗口防振荡**：bootstrap.rs:168 硬编码 32/4。改为可配置 + 软阈值 16 FIFO + 内存监控超阈值停止 spawn + 冷却窗口——这是一个完整的背压子系统。process_supervisor 已存在但未必暴露 per-process 内存给 agent runtime。**风险**：(1) FIFO 排队语义需暴露给前端（主席承认），否则用户看到 spawn 卡住无反馈；(2) 冷却窗口阈值若设错会振荡（降级→升级→降级）；(3) 内存监控跨平台（Windows/Linux/macOS RSS 取法不同）。这是 SRE 级工程，Phase 4 时间盒 5-6 周做完并发+场景化+离窗化+secret store+安装器健康检查+CI 门，严重低估。
- **ADR-002 surface 状态机复用 WorkspaceStage 作容器 + sidebar rail 52px 视觉契约**：layouts/index.tsx 靠 isChatShell 二态散落。升级为「主对话流常驻 + 副 surface 分屏/浮窗/内联卡片」叠加态——这是 Layout 重构，触及所有页面的 Outlet 挂载点。**风险**：多 surface 并存焦点/键盘/aria 管理主席自己承认「复杂度上升需焦点陷阱+Escape 收敛否则 a11y 退化」。React Router 的 Outlet 单挂载模型不支持原生分屏，要改成 portal + 自管理 focus trap——这是 a11y 雷区。**要求**：Phase 1 surface 状态机只做「单 surface 切换」（一次只开一个面），分屏/浮窗推迟，避免 a11y 退化阻塞 Phase 1 交付。
- **turn_completed 扩 artifact_refs + reason（gen:api）**：/v1/agents/responses 的 SSE/WS 事件 schema 变更，前端 use-assistant-agent.ts 事件解析要同步。**风险**：artifact_refs 文件路径若指向 workspace 之外（如 /etc/passwd 或 .. 跨越），agent-action-card 的 open 按钮直接 shell.openSurface 会泄漏/打开任意路径。主席在 ADR-010 后果提了「必须做权限校验」但 exit criteria 没列。**要求**：artifact_refs 必须在 host 层做 workspace 根路径前缀校验，非 workspace 内路径拒绝 open，作为 Phase 1 exit criteria 硬门。
- **subagent.rs:117 wait_for_terminal_snapshot 同步阻塞**：主席标为 Phase 4 改并行。但 delegate_subagent 是 ToolHandler，工具执行是 async fn——改为并行 spawn(tasks: Vec) 意味着一个 tool call 内 fan-out 多个 child thread 并 join。**风险**：现有 control.spawn_child_for_parent 返回单个 child_thread_id，改并行要返回 Vec 且每个 child 的 status/abort/cancellation 独立管理，控制面 registry 要支持批量。这触及 control.rs 的线程注册表数据结构。Phase 4 与 ADR-013 并发预算强耦合（并行 spawn 触发 32 上限），两个要一起设计不能分阶段。

## 必须补上（must_add，已吸收进各 TD）

- **新增 ADR：离线降级模式**——agent 启动探测 provider 可达性，离线时自动收窄工具集（禁 web_search/外部 MCP/云端模型），UI 标注「离线模式」。这是北极星「离线可用」的硬要求，当前完全缺失。
- **新增 ADR：敏感路径审批黑名单**——read_file/grep/list_dir 命中 ~/.ssh、.env、*credentials*、*token*、*.pem 时强制 ask，覆盖 ADR-008 的 read 类默认 allow。守隐私优先红线不被只读默认架空。
- **新增 ADR：per-thread token 硬预算**——LLM 调用累计 token 超 thread 预算触发 BudgetExhausted 终止（ThreadStatus 已支持 reason）。防 runaway agent 烧光 provider 配额。比循环检测更现实的成本失控兜底。
- **新增 ADR：artifact_refs workspace 路径前缀校验**——agent-action-card 的 open/review 按钮在 host 层校验路径必须在工作区根下，跨目录/绝对路径拒绝。作为 Phase 1 exit criteria 硬门，防 agent 产出指向 workspace 之外的危险引用。
- **新增 ADR：effects 由 host 静态推断（基于 runtime 类型），插件自报 effects 只作 hint**——堵 ADR-009 的插件自报 effects 漏洞。js→Tauri sandbox、python→PyO3 isolate、wasm→extism，由 runtime 类型决定 effects 信任等级。
- **新增 ADR：interrupt grace period + session 快照原子性**——补 ADR-012 的 interrupt 等待语义。interrupt 后等 watch channel 确认 Interrupted 状态才写快照，超时中止切换不强 kill。防 workspace 切换数据丢失 race。
- **新增 ADR：任务级文件改动快照 + 一键回滚**——任务开始时 git stash 快照，任务总结 review 按钮支持「回滚本次所有改动」。补 undo 视角缺口。
- **Phase 0 exit criteria 新增：敏感路径黑名单 + token 预算的字段定义先行**（即使实现推迟，schema 先冻结，与诊断包白名单同等级别签字）。

## 建议移除（must_cut，已从计划移除）

- **砍 ADR-003 的 DAG + replan(plan_patch) + 独立 namespace 持久化**：保留 task.complete default-deny + plan result_ref 回填到现有 normalize_plan。DAG/replan 推迟到有真实跨小时任务 case。Phase 2 时间盒从 4-5 周压到 3 周。
- **砍 ADR-010 AgentTimeline DAG 节点图 + subagent 子时间线 + checkpoint**：保留任务总结三动作按钮（open/review/feedback）+ 简单进度条（X/N plan items）。DAG 可视化推迟到 Phase 3 之后。
- **砍 Phase 4 并行 delegate_subagent(tasks: Vec) + 按复杂度缩放（事实 1/对比 2-4/研究 8-10）**：与 non_goals「不默认就上多 agent」+ GLM ~10-14 并发 429 直接冲突。保留单 subagent 同步委派 + 预算看板。Phase 4 时间盒从 5-6 周压到 4 周。
- **砍 P5 capabilities.available 独立工具**：改为 system prompt 注入现有 plugins/mcp.list 能力清单。减少 tool 表膨胀。
- **砍 Phase 1 surface 状态机的分屏/浮窗/内联卡片三态叠加**：Phase 1 只做单 surface 切换（一次一个面，替换 Outlet 内容），分屏/浮窗推迟。避免 a11y 焦点陷阱阻塞交付。
- **砍 Phase 4 场景化 onboarding 三类（开发/创作/办公）预配工具白名单 + 推荐模型组合**：这是产品差异化工程，与统一入口核心无关，且需大量用户研究。推迟到统一入口稳定后，本阶段不做。
