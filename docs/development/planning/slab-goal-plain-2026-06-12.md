# Slab 增强路线图

> 日期：2026-06-12  
> 依据：当前源码为准，`docs/development/production-design/` 仅作为已存在的技术设计背景。  
> 性质：目标文档，描述 Slab 需要持续补齐的产品能力、缺失点、验收口径和执行顺序。  
> 边界：本文只保留源码和现有产品文档作为依据，也不重复 production-design 中已经存在的技术路线图。

## 1. 文档定位

这份路线图回答的是：Slab 作为本地优先 AI 桌面工作台，长期还缺哪些用户可感知的能力，以及这些能力应该如何落到现有架构边界里。

- 以源码现状为基线的能力总账。
- 以用户工作流为中心的长期增强路线。
- 连接 Chat、Agent、Workspace、Model Hub、Media、Plugin、MCP、Settings、Tasks、Distribution 的产品级目标。
- 每次落地都能回到现有 `/v1/*` API、app-core、runtime、plugin、agent、workspace、settings schema 和前端页面验证。

## 2. 源码基线

当前源码已经形成这些基础能力，后续路线图应在这些基础上增强，而不是另起一套平行体系。

### 2.1 进程与边界

- 桌面宿主：`bin/slab-app` 负责 Tauri shell、sidecar、权限和 WebView 容器。
- HTTP 网关：`bin/slab-server` 承接 `/v1/*` API、WebSocket、OpenAPI、runtime/plugin sidecar 监督。
- 业务核心：`crates/slab-app-core` 保持 HTTP-free，承载业务服务、状态、存储、provider 解析和 agent 接入。
- 推理运行时：`bin/slab-runtime` 是唯一 runtime composition root，`crates/slab-runtime-core` 只承载调度和后端协议。
- Agent 控制面：`crates/slab-agent` 保持纯编排；内置确定性工具在 `crates/slab-agent-tools`；插件/API 能力由 host/app-core 注册。
- 插件运行时：`plugin.json` v1 是静态来源，JS/Python/WASM backend 与 Tauri child WebView UI 保持边界。
- Workspace LSP：`packages/slab-desktop -> /v1/workspace/lsp/{language} -> slab-app-core -> LSP process` 是唯一主链路。

### 2.2 当前 API 与实时通道

当前可确认的关键入口包括：

- Chat：`POST /v1/chat/completions`
- Agent：`GET|POST /v1/agents/responses`
- Models：`/v1/models`、`/v1/models/download`、`/v1/models/load`、`/v1/models/unload`、`/v1/models/switch`
- Tasks：`/v1/tasks`、`/v1/tasks/{id}`、`/v1/tasks/{id}/result`、`/v1/tasks/{id}/cancel`、`/v1/tasks/{id}/restart`
- Audio：`/v1/audio/transcriptions`
- Image：`/v1/images/generations`
- Video：`/v1/video/generations`
- Plugin：`/v1/plugins/rpc`、`/v1/plugins/events`、`/v1/plugins/import-pack`、启停和删除接口
- Workspace LSP：`/v1/workspace/lsp/{language}`
- Settings：`/v1/settings`、`/v1/settings/{pmid}`
- UI State：`/v1/ui-state/{key}`

### 2.3 当前 Agent 基础

- Agent thread 状态已有 `pending/running/interrupting/interrupted/completed/errored/shutdown`。
- Tool call 状态已有 `pending/running/completed/failed`。
- 内置工具注册面已有 `shell`、`read_file`、`write_file`、`list_dir`、`grep`、`web_search`、`fs_watch`、`apply_patch`、MCP 调用与动态 MCP proxy、`delegate_subagent`，Git 工具可按开关注册。
- shell 默认受 sandbox 与 shell rules 约束。
- memory pipeline 已有 settings、startup hook、instruction hook、memory root、phase1/phase2 参数。
- hook 基础已存在：本地脚本 hook、Agent lifecycle event、插件 hook source 接入。

### 2.4 当前 MCP 基础

- `slab-mcp-client` 已支持 stdio 外部 MCP 的 `initialize`、`notifications/initialized`、`ping`、`tools/list`、`tools/call`。
- `crates/slab-mcp` 已有多 server client、cached tools、server_name 路由和 stdio launch config。
- `bin/slab-mcp-server` 当前仍是协议壳：`tools/list` 返回空列表，`tools/call` 返回 tool not found。
- app-core 当前打开 `agent.tools.mcp.enabled` 时会创建空 `McpClient`，但尚未接入持久化 MCP server launch config。

### 2.5 当前插件基础

- `manifestVersion: 1` 是必填；缺失或非 1 会被拒绝。
- `runtime.ui.entry` 是必填；`runtime.js`、`runtime.python`、`runtime.wasm` 是可选 backend。
- permissions 已覆盖 network、ui、agent、lsp、slabApi、files。
- contributes 已覆盖 routes、sidebar、commands、settings、agentCapabilities、agentHooks、languageServers。
- `agentCapabilities[].exposeAsMcpTool` 已有 manifest 字段，并要求对应 agent permission。
- JS runtime 已按 permission 控制 fetch、文件授权标签和 Slab API 回调。
- 插件 WebView caller id 必须由 WebView label 推导，不信任插件 payload。

### 2.6 当前产品页面基础

前端已有 Assistant、Hub、Settings、Workspace、Plugins、Image、Audio、Video、Task、About 等页面骨架。路线图的重点不是新增入口数量，而是让这些页面形成闭环：可恢复、可解释、可诊断、可配置、可扩展、可验证。

## 3. 产品北极星

Slab 的长期目标是本地优先 AI 桌面工作台：

- 用户在一个桌面产品里完成对话、模型管理、媒体处理、代码工作区、插件扩展和自动化。
- 默认数据、模型、会话、任务、插件、工作区状态留在本机。
- 云端 provider、远程访问、外部 MCP、插件市场都是可选增强，不改变本地优先默认路径。
- Agent 不是独立后端，也不是单纯聊天机器人，而是 Slab 内部所有能力的可控编排层。
- 插件不是前端装饰，而是长期生态扩展面：UI、命令、Agent 能力、Hook、LSP、MCP 暴露都要统一治理。
- 每个长期能力都必须能被用户看见、控制、撤销、审计和验证。

## 4. 长期执行原则

- 只扩展现有 `/v1/*` API，不新增平行 API 树。
- 不把 HTTP、Tauri、SQLx 逻辑放进纯核心 crate。
- 不把业务逻辑塞进 `crates/slab-runtime-core`。
- 不把内置确定性工具放进 `crates/slab-agent`。
- 不在桌面 host 里新增第二套 LSP bridge。
- 不把 native LSP 二进制打进插件包或安装包，除非产品目标明确改变。
- 不把 Module Federation 作为默认插件模型。
- 不绕过 Tauri CSP、capabilities、permissions、sidecar 和插件沙箱边界。
- 不把 secret 长期散落在普通 JSON 明文字段里。
- 不用“功能数量”衡量 Agent 或插件成熟度，用工作流闭环衡量。
- SQLx migration 只追加，不修改历史迁移。
- 后端 API shape 变化后跑 `bun run gen:api`。
- settings、model、plugin schema 变化后跑 `bun run gen:schemas` 或对应生成命令。

## 5. 能力路线图

### 5.1 Assistant 与 Agent 交互面

目标：把 Assistant 从“能发消息”增强为可长期工作的 Agent 控制台。

已有基础：

- Assistant 页面已接入模型列表、下载、加载、切换、会话列表和 Agent hook。
- `/v1/agents/responses` 已支持 GET/POST，GET 可用于 WebSocket/SSE transport。
- Agent thread 与 tool call 已有状态类型。

缺失点：

- 用户看不到完整的 Agent 执行结构：当前步骤、工具调用、等待审批、失败原因、可恢复点之间的关系不够清晰。
- 工具调用记录需要更适合回放：参数摘要、输出摘要、错误类型、耗时、审批结果、触发 hook、产生的文件变更。
- 中断、继续、重试、恢复会话需要成为一等交互，而不是隐藏在后端状态中。
- 模型下载、加载、切换与发送消息之间需要更平滑：等待状态、失败重试、用户取消、已有任务复用。
- 系统消息、推理内容、工具结果、Markdown、代码块、artifact 引用需要统一渲染规则。
- Assistant 页面需要能解释“为什么现在不能执行”：没有模型、模型未下载、runtime 不可用、等待审批、工具被策略阻止、MCP server 不在线。

目标能力：

- 线程视图：显示 thread id、状态、当前 turn、运行模型、上下文来源、工具集合。
- 执行时间线：按 turn 展示用户输入、模型响应、工具调用、hook 介入、审批、错误和最终输出。
- 可操作状态：中断、继续、重试失败工具、从最后稳定点恢复、复制错误诊断。
- 审批队列：集中显示待审批工具、风险说明、命令/路径/网络目标、一次性批准或拒绝。
- 工具回放：折叠展示输入输出，支持只读预览文件 diff、shell 输出、MCP 输出、插件输出。
- 模型预检：发送前检查模型下载、加载、能力匹配、context window、tool calling/reasoning 支持。
- 会话归档：会话标题、摘要、标签、最近任务、相关 workspace、相关 memories 可检索。
- 错误体验：所有错误都能落到可操作原因，而不是原始堆栈。

验收：

- 一条端到端烟测覆盖：选择模型、必要时下载、加载、发送、触发工具、审批、中断、恢复、回放。
- Agent thread 和 tool call 状态能在 UI 上一致映射。
- `/v1/agents/responses` 类型变更后 `bun run gen:api` 无漂移。

### 5.2 Agent 编程工作流

目标：让 Agent 能可靠参与本地代码工作，而不是只做聊天回答。

已有基础：

- `slab-agent-tools` 已有 shell、文件读写、list、grep、apply_patch、fs_watch、web_search、subagent、MCP。
- Workspace 已有文件树、搜索、Git 面板、终端/控制台和 Monaco 编辑器。
- LSP 主链路已经在 app-core 侧解析 provider。

缺失点：

- 文件编辑还需要更强的“最小变更”体验：patch 预览、失败原因、冲突处理、用户确认、回滚。
- 文件搜索需要 glob、文件名搜索、ignore/gitignore 规则、大小限制、二进制跳过和结构化结果。
- grep 需要上下文行、多模式、文件类型过滤、结果分页、跨 workspace 限流。
- Agent 需要可用的只读代码智能：diagnostics、symbols、definition、references、hover、completion 摘要。
- Agent 修改文件后，Workspace UI、Git diff、文件树 dirty state、诊断刷新需要联动。
- shell 工具需要 profile 化：PowerShell、cmd、bash、项目脚本、超时、工作目录、环境变量模板。
- 用户澄清能力需要产品化：Agent 在缺信息时能提出短问题，前端能暂停并恢复执行。
- Todo/Plan 需要变成可见状态，而不是只存在模型输出文本里。

目标能力：

- `file.glob`：按 workspace root、ignore 规则、最大结果数、文件类型过滤返回结构化文件清单。
- `file.patch.preview`：生成 diff、检查目标文件版本、展示影响范围。
- `file.patch.apply`：只对用户确认或低风险 patch 生效，失败时返回冲突位置。
- `code.diagnostics`：从 app-core LSP 服务读取当前文件或项目诊断摘要。
- `code.symbols`：返回文件/项目 symbol tree，支持按语言过滤。
- `code.references`：为重命名、删除、调用链分析提供只读引用查询。
- `workspace.context_pack`：从文件树、搜索结果、Git diff、终端输出、LSP 诊断裁剪上下文。
- `user.ask`：Agent 发出短问题，UI 收集用户回答并恢复当前 turn。
- `plan.update`：记录任务计划、状态、阻塞原因，支持用户手动调整。
- `shell.profile`：常用脚本、允许规则和风险等级可视化。

验收：

- 新 Agent 工具只进入 `crates/slab-agent-tools` 或 host/plugin 边界。
- 危险工具必须经过 approval 或 shell policy。
- Workspace LSP 路由保持 `/v1/workspace/lsp/{language}`。
- 文件修改能在 Workspace 和 Git diff 中可见。

### 5.3 Agent 记忆与上下文治理

目标：让长期记忆成为可控、可解释、可删除、可评估的产品能力。

已有基础：

- settings 已有 `agent.memories.enabled`、`model`、`memory_root`、phase1/phase2、保留时间等配置。
- app-core 已接入 memory startup hook 和 instruction hook。
- `slab-agent-memories` 已有 memory 模板、读写与生成逻辑。

缺失点：

- 用户缺少记忆中心：不知道记忆是否开启、保存在哪里、写入了什么、何时被注入。
- 记忆生成和使用缺少可视化审计：来源 session、摘要、引用、注入 turn、命中原因。
- 记忆删除、禁用、导出、重建、迁移需要产品化。
- 记忆质量缺少反馈机制：错误记忆、过期记忆、重复记忆、敏感记忆应能标记。
- 工作区级记忆和全局记忆需要边界：不同项目不应默认互相污染。
- 插件和 hook 写入记忆必须有权限与审计。

目标能力：

- 记忆设置面：启用状态、memory root、模型选择、扫描频率、保留策略、隐私提示。
- 记忆浏览器：按来源 session、项目、标签、更新时间、使用次数查看。
- 注入日志：每次 Agent turn 显示注入了哪些记忆、为什么注入、token 预算。
- 记忆编辑：用户可删除、禁用、合并、重新生成、导出。
- 记忆隔离：全局记忆、workspace 记忆、插件记忆分别标识。
- 敏感数据防护：API key、路径、个人信息可识别并要求用户确认。
- 质量反馈：用户标记“错误/过期/有用”，作为后续筛选信号。

验收：

- `agent.memories.enabled=false` 时不注入记忆，也不显示伪状态。
- 开启记忆后，用户能看到 memory root、最近扫描和最近注入记录。
- 删除或禁用记忆后，新 turn 不再引用该条记忆。

### 5.4 Hook 与自动化

目标：把 Hook 从底层扩展点变成可治理的自动化系统。

已有基础：

- Agent lifecycle event 已包含 agent/LLM/tool 开始结束等事件。
- settings 已有 `agent.hooks.enabled` 和本地脚本列表。
- 插件 manifest 已有 `contributes.agentHooks`，并可声明 JS/Python transport。

缺失点：

- 用户缺少 Hook 管理界面：启停、事件范围、运行时、失败记录、权限说明。
- 本地脚本 hook 和插件 hook 缺少统一审计视图。
- Hook 失败策略需要明确：阻断、跳过、重试、降级、通知用户。
- Hook 修改工具参数、阻断工具调用时，需要能被用户看见和回放。
- Hook 的权限模型需要和插件权限、shell policy、文件访问边界对齐。
- Hook 开发体验需要最小 SDK、示例、测试命令和日志。

目标能力：

- Hook 中心：列出本地脚本和插件 Hook，显示事件、运行时、启用状态、最近结果。
- 事件筛选：按 on_agent_start、on_llm_start、on_tool_start 等选择触发范围。
- 风险说明：Hook 能读写什么、是否能改 tool args、是否能 block。
- 审批联动：高风险 Hook 修改或阻断工具时进入审批或显示强提示。
- 失败隔离：单个 Hook 失败不拖垮 Agent，除非用户选择 fail-closed。
- 调试日志：输入 payload 摘要、输出 outcome、耗时、错误堆栈脱敏。

验收：

- Hook 执行结果能在 Agent 时间线中看到。
- 插件 Hook 必须经过 manifest permission 验证。
- 本地脚本 Hook 禁用后不再触发。

### 5.5 MCP 集成中心

目标：让 MCP 成为 Slab 与外部工具生态互联的稳定入口。

已有基础：

- MCP client 支持 stdio 初始化、ping、tools/list、tools/call。
- MCP 多 server client 已有 cached tools 与 server_name 路由。
- Agent 工具注册中已有 MCP call 与 proxy tool 的基础入口。
- 独立 `slab-mcp-server` 已有协议响应壳。

缺失点：

- app-core 尚未把持久化 MCP server 配置接到 Agent MCP client。
- 缺少 MCP 管理 UI：新增 server、启停、测试连接、查看工具、查看日志。
- 缺少资源能力：resources/list、resources/read、prompt、sampling 等协议面尚未规划落地。
- 缺少传输扩展策略：stdio 稳定后再进入 HTTP/SSE 或 streamable HTTP。
- 缺少 secret 管理：MCP server 环境变量和 token 不应明文散落。
- `slab-mcp-server` 目前不能暴露 Slab 工具，只能返回空 tools。
- 插件 `exposeAsMcpTool` 与 Slab MCP server 暴露路径需要合并治理。

目标能力：

- MCP server 配置：name、command、args、env secret 引用、cwd、enabled、workspace scope。
- 连接生命周期：启动、initialize、健康检查、重连、禁用、错误分类、日志。
- 工具目录：按 server 分组展示 tools、input schema、风险等级、最近调用。
- Agent 工具来源：Agent 时间线标明工具来自内置、MCP server、插件或 Slab API。
- MCP resources：支持列出和读取外部资源，进入 Agent 上下文选择器。
- MCP secret：支持环境变量引用、本地 secret 引用、UI writeOnly 输入。
- Slab MCP server：安全暴露文件只读、搜索、任务查询、模型状态、插件能力等可控工具。
- 插件 MCP 暴露：插件能力先进入统一 registry，再由 MCP server 暴露，避免重复命名。

验收：

- 开启 MCP 后至少一个 stdio server 能持久化、重启后自动恢复、工具可被 Agent 调用。
- `bin/slab-mcp-server` 的 `tools/list` 不再是空列表时，所有工具必须有权限、来源和审计。
- MCP 配置中的 secret 不以普通明文字段回显。

### 5.6 插件生态与市场

目标：让插件成为用户和开发者可长期依赖的扩展生态。

已有基础：

- 本地插件目录和 pack 导入入口已存在。
- `plugin.json` v1 已覆盖 runtime、permissions、contributes 和 integrity。
- JS/Python/WASM backend 与 WebView UI 有基础运行时。
- 插件可以贡献 routes、sidebar、commands、settings、agentCapabilities、agentHooks、languageServers。

缺失点：

- 插件中心需要更完整：安装、导入 pack、启停、卸载、更新、权限、日志、错误恢复。
- 插件市场需要私有/本地 registry manifest，后续再考虑公开市场。
- 插件权限需要用户可读：network hosts、file labels、slabApi permissions、agent/lsp 权限。
- 插件运行时需要资源治理：超时、CPU/内存限制、日志隔离、崩溃隔离。
- 插件开发体验需要模板、打包、校验、签名、调试、SDK 文档。
- 插件贡献点冲突需要处理：命令 id、路由、sidebar、agent capability、MCP tool 名称。
- 插件设置 contribution 需要进入 Settings UI，并支持 schema 验证和迁移。
- 插件 LSP 与内置/native LSP 的优先级、状态、失败原因需要可见。

目标能力：

- 插件详情页：manifest、版本、来源、integrity、权限、贡献点、运行时状态。
- 权限审批：安装时展示权限，运行时高风险权限支持再次确认和撤销。
- 插件日志：按 plugin id 聚合 JS/Python/WASM/WebView 日志和 host callback 错误。
- 插件更新：registry manifest 检测新版本、兼容性、更新日志、回滚。
- 私有 registry：本地文件或内网 URL 提供插件索引，支持离线导入。
- SDK 稳定化：TS/Python API、权限类型、示例插件、CI 校验命令。
- 插件任务化：长耗时插件能力进入 `/v1/tasks`，可取消、重试、查看 artifact。
- 贡献点治理：冲突检测、命名空间规范、用户禁用单个贡献点。

验收：

- `bun run gen:plugin-packs` 能生成有效 pack。
- 插件越权调用被拒绝并记录。
- 插件 WebView caller id 继续从 WebView label 推导。
- 插件启停不需要重启整个 Slab。

### 5.7 Workspace、LSP 与开发环境

目标：把 Workspace 做成 Agent 和用户共享的代码工作台。

已有基础：

- Workspace 页面已有 workbench 入口。
- LSP WebSocket 路由已存在。
- app-core 负责 workspace root、文件系统、LSP provider 解析与进程生成。
- 内置 web LSP 作为构建产物；native LSP 走 PATH 或系统搜索。

缺失点：

- Workspace 状态需要更完整：当前 root、文件树刷新、打开文件、dirty 文件、Git 状态、终端状态。
- LSP provider 状态需要可视化：可用、缺失、启动中、崩溃、版本、日志。
- 代码编辑与 Agent 修改需要冲突处理：用户未保存、Agent patch、外部文件变化。
- Search、Git diff、diagnostics、terminal output 需要能被裁剪成 Agent 上下文。
- 多 workspace 或最近 workspace 管理需要产品化。
- 大仓库性能需要索引、分页、文件忽略、后台扫描状态。

目标能力：

- Workspace 管理：最近打开、固定、移除、验证路径、显示权限范围。
- 文件树增强：ignore 规则、搜索、创建/重命名/删除确认、外部变化刷新。
- 编辑器增强：dirty 标记、保存失败、格式化、诊断面板、跳转、引用。
- LSP 状态面板：provider 来源、进程命令、启动日志、错误、重启。
- Agent 上下文选择：当前文件、选区、diff、diagnostics、terminal、search results 可勾选。
- 冲突处理：Agent 修改前检查文件版本，冲突时展示三方信息。
- Git 工作流：status、diff、stage、commit、discard 必须有清晰风险提示。

验收：

- LSP 链路不绕过 app-core。
- Agent 从 Workspace 取代码上下文时有明确来源和 token 预算。
- 文件修改后 UI、LSP、Git diff 状态一致。

### 5.8 Model Hub 与推理可靠性

目标：让模型发现、下载、加载、推理、卸载成为可恢复的长期基础设施。

已有基础：

- `/v1/models`、download、load、unload、switch 已存在。
- ModelState、WorkerState、RuntimeSupervisor、GrpcGateway 已形成运行时状态链。
- settings 已有 models cache/config、download_source、auto_unload。
- Runtime 覆盖 ggml/llama/whisper/diffusion/candle/onnx 等方向。

缺失点：

- 下载去重、断点续传、校验失败恢复、磁盘空间预检需要更强闭环。
- Runtime 崩溃后 ModelState 与实际进程状态需要自动校正。
- 模型能力需要显式矩阵：chat、embedding、image、video、audio、tool calling、reasoning、context window。
- 加载失败需要可操作原因：文件缺失、格式不支持、后端不可用、GPU OOM、context 超限。
- 自动卸载与推理请求、用户主动卸载之间需要竞态治理。
- provider 凭证、私有模型源、镜像源需要安全配置。
- 前端需要更清楚地区分 cloud/local、downloaded/ready/loaded/active/pending。

目标能力：

- 下载任务治理：同模型同 artifact 去重、断点信息、校验、清理临时文件、失败重试。
- 模型健康检查：文件存在、hash、backend 支持、load dry-run、最后加载错误。
- 能力矩阵：模型卡片直接展示能力和限制，Assistant/Media 自动过滤不匹配模型。
- Runtime 恢复：runtime 重启后重置或重新校验所有 loaded state。
- OOM 策略：稳定错误码、建议降低 context、切换量化、切换 CPU/后端。
- 版本与来源：记录下载来源、artifact、selected_download_source、许可证、大小。
- 模型操作审计：下载、删除、加载、卸载、切换、失败原因可追踪。

验收：

- 模型下载重复点击不会产生重复写入。
- runtime 崩溃后 UI 不继续显示虚假的 loaded。
- 发送前能判断模型是否支持当前任务能力。

### 5.9 媒体与创作工作流

目标：让 Audio、Image、Video 不只是 API 示例，而是完整创作工作台。

已有基础：

- Audio transcription、Image generation、Video generation 都已挂到任务型 API。
- `/v1/tasks` 可查询结果、取消和重启。
- FFmpeg、subtitle、media task、runtime inference 已有服务基础。
- video-subtitle-translator 插件展示了插件化媒体工作流的方向。

缺失点：

- 媒体任务需要统一任务面板：输入、参数、进度、日志、artifact、重试、导出。
- Audio 需要批量转录、时间轴编辑、说话人/语言、字幕导出、错误重跑。
- Image 需要历史、参数 preset、seed 管理、reference image、artifact 管理、批量导出。
- Video 需要进度、预览、参考素材、失败恢复、格式导出、长任务后台化。
- 字幕工作流需要解析、编辑、翻译、渲染、格式互转、版本差异查看。
- 大文件上传/处理需要大小限制、临时文件清理、磁盘空间提示。
- 模型能力与媒体任务需要联动：Whisper、Diffusion、Llama 翻译、VAD 模型状态。

目标能力：

- 统一媒体任务卡：任务类型、输入文件、模型、参数、进度、日志、结果 artifact。
- Audio Studio：上传、转写、分段、纠错、导出 SRT/VTT/TXT/JSON。
- Image Studio：prompt、negative prompt、size、seed、steps、sampler、reference、历史图库。
- Video Studio：生成、转码、提取音轨、字幕渲染、预览、导出。
- Subtitle Studio：导入、时间轴调整、翻译、合并、渲染、质量检查。
- Artifact 管理：文件位置、大小、格式、打开、复制路径、删除。
- 插件增强：允许插件贡献媒体处理 pipeline，但进入任务系统和权限治理。

验收：

- 每种媒体任务都能在 `/v1/tasks` 看到状态和结果。
- 任务失败后用户能看到输入、参数、错误、可重试动作。
- artifact 不丢失、不误删，并能从任务结果打开。

### 5.10 Settings、凭证与安全

目标：让配置、凭证、权限成为可理解、可迁移、可审计的产品能力。

已有基础：

- `slab-config` 独立管理 settings document、PMID、schema 和类型安全视图。
- settings 覆盖 general、database、logging、telemetry、tools、agent、runtime、providers、models、plugin、workspace、server。
- server admin token 已有配置字段。
- websearch provider schema 已标记 API key writeOnly。

缺失点：

- Settings UI 需要把复杂 JSON 字段变成更安全的表单，而不是只暴露结构化 JSON。
- secret 类型需要统一：provider key、MCP env、plugin secret、server admin token。
- 配置变更影响范围需要提示：是否需要重启、是否影响 runtime、是否影响插件、是否影响当前任务。
- 工作区覆盖、全局默认、插件设置之间需要清晰优先级和恢复默认。
- 配置损坏、迁移失败、外部修改需要用户可恢复。
- 远程访问仍应是可选高级能力，不应改变本地默认信任模型。

目标能力：

- 设置影响提示：每个设置显示影响模块、是否立即生效、是否需重启。
- Secret 字段：writeOnly、不可回显、支持环境变量引用、支持本地 secret store。
- 配置审计：最近修改、来源、旧值摘要、新值摘要、回滚。
- 配置导入导出：隐去 secret，可选择包含/不包含模型和插件路径。
- 工作区覆盖：明确哪些设置可 workspace override，显示继承关系。
- 远程访问：只有用户显式开启时才配置 bind address、admin token、CORS、速率限制。
- 权限总览：Agent、插件、MCP、shell、file、network 权限集中查看。

验收：

- secret 不在 GET settings 响应中明文回显。
- 修改关键设置后 UI 能提示是否需要重启相关服务。
- 本地默认路径不强制 OAuth/JWT。

### 5.11 Tasks、可观测性与恢复

目标：所有长任务都能被追踪、恢复、取消、重试和诊断。

已有基础：

- `/v1/tasks` 已有 list/get/result/cancel/restart。
- app-core 有 WorkerState、OperationManager、TaskService。
- 媒体和模型下载已经部分走任务化。
- Agent trace 可在 telemetry 开启时落到本地文件。

缺失点：

- 任务类型需要统一：model download、audio transcription、image generation、video generation、plugin job、agent job、workspace scan。
- 任务日志和 artifact 需要标准化。
- 任务取消语义需要覆盖 backend、runtime、插件、FFmpeg、MCP。
- 任务失败原因需要分类：用户取消、输入错误、权限拒绝、依赖缺失、runtime 崩溃、OOM、网络失败。
- 应用重启后的任务恢复/中断标记需要一致。
- 前端任务中心需要成为全局状态入口，而不是孤立页面。

目标能力：

- 全局任务中心：筛选类型、状态、来源页面、创建时间、进度、错误。
- 任务详情：输入摘要、参数、日志、artifact、关联模型、关联插件、关联 Agent thread。
- 取消与重试：可取消任务必须声明 cancel handler；重试复用输入参数并生成新 task。
- 任务通知：完成、失败、需要用户动作时通知。
- 任务恢复：启动时中断 running tasks，标记原因，并提供重试。
- 统一错误码：后端、runtime、plugin、MCP、media 转换统一映射到用户可理解原因。

验收：

- 所有长于几秒的操作都应进入 task 或 agent thread 时间线。
- 应用关闭时 running tasks 不留下虚假 running。
- 任务结果和 artifact 可通过 `/v1/tasks/{id}/result` 获取。

### 5.12 分发、安装与离线体验

目标：让 Slab 在普通用户机器上可安装、可升级、可离线使用。

已有基础：

- 顶层工作流是 Bun + Cargo。
- Windows installer、sidecar staging、language server bundle、runtime artifacts 已有方向。
- FFmpeg 和 language server 有资源打包与回退路径。

缺失点：

- 安装器需要检查 sidecar、runtime、FFmpeg、web LSP bundle、插件 pack、模型目录权限。
- 首次启动需要清晰引导：模型目录、下载源、硬件后端、隐私、插件权限。
- 离线包需要规划：基础 runtime、内置 web LSP、示例插件、可选模型 pack。
- 升级需要迁移 settings、数据库、插件、模型 metadata。
- 崩溃日志和诊断包需要可导出。
- 不同平台的能力差异需要在 UI 中明确：GPU backend、native LSP、FFmpeg、sandbox。

目标能力：

- 首次设置向导：路径、模型源、默认模型、硬件检测、隐私和网络开关。
- 安装健康检查：sidecar 可执行、权限、资源文件、端口、数据库、日志目录。
- 离线模式：禁用云 provider 后仍能使用本地模型、workspace、已安装插件和媒体工具。
- 更新流程：版本检查、迁移提示、失败回滚、变更日志。
- 诊断包：日志、settings 摘要、runtime status、plugin status、task error、系统信息。
- 发布验收矩阵：Windows 优先，同时保留 macOS/Linux 能力差异表。

验收：

- `bun run build:app` 和发布前 `bun run build:windows-installer` 可作为分发证明。
- 新安装用户能在无手动命令的情况下完成首次模型下载或导入模型。
- 诊断包不包含明文 secret。

## 6. 阶段化执行

### 阶段 0：基线校准

目标：让代码、OpenAPI、生成类型、产品文档和 UI 行为说同一种语言。

要做：

- 清理文档中与源码不一致的 API 路径、状态、模块边界。
- 建立 API 路由、settings schema、plugin manifest、agent event、runtime protobuf 的校验清单。
- 确认 production-design 是技术设计背景，不把它当成待复制的路线图。
- 给每个能力域建立 source-of-truth 文件指针。
- 当前阶段 0 校准入口：`docs/development/planning/slab-source-of-truth-2026-06-13.md`。

验收：

- `bun run gen:api`
- `bun run gen:schemas`
- 文档只引用源码和 production-design 背景。

### 阶段 1：可用性闭环

目标：优先解决用户每天会遇到的断点。

要做：

- Assistant 发送、模型下载/加载、任务等待、错误重试闭环。
- Task 中心统一模型下载、媒体任务和失败恢复。
- Hub 模型状态从下载到 loaded/active 一致显示。
- Settings 中 Agent memory、hook、MCP、websearch 的基础配置可见。
- Runtime 崩溃后 ModelState 自动校正。

验收：

- 助手、模型、任务三条路径有最小 smoke。
- runtime 不可用时前端显示可操作原因。

### 阶段 2：Agent 产品化

目标：让 Agent 执行过程可看、可控、可恢复。

要做：

- Agent 时间线、工具回放、审批队列、中断恢复。
- 精确编辑、glob、增强 grep、用户澄清、plan/todo 工具。
- Memory 中心和 Hook 中心最小版本。
- Agent trace、tool metadata、hook outcome 进入 UI。

验收：

- `cargo check -p slab-agent -p slab-agent-tools -p slab-agent-memories -p slab-app-core`
- `bun run check:frontend`
- 至少一条烟测覆盖工具审批和恢复。

### 阶段 3：Workspace 智能化

目标：把 Workspace 与 Agent 合成一个代码工作流。

要做：

- LSP provider 状态页。
- Agent 只读代码智能工具。
- Git diff、diagnostics、terminal、search result 作为可选上下文。
- Agent patch 与用户编辑冲突处理。

验收：

- LSP 链路仍只走 `/v1/workspace/lsp/{language}`。
- Agent 读取代码智能只通过 app-core。
- 文件修改能被 Workspace/Git/LSP 一致感知。

### 阶段 4：插件与 MCP 生态

目标：扩展能力稳定进入 Slab，而不是停留在本地实验。

要做：

- 插件中心完善权限、日志、启停、更新、registry。
- MCP server 配置持久化、连接管理、工具目录、resources。
- `slab-mcp-server` 暴露安全 Slab 工具集。
- 插件 agentCapabilities、agentHooks、languageServers、MCP 暴露统一治理。

验收：

- `cargo test -p slab-mcp-client -p slab-mcp -p slab-mcp-server`
- `bun run gen:plugin-packs`
- 插件越权和 MCP 越权都有测试覆盖。

### 阶段 5：模型、媒体与创作成熟化

目标：把推理和媒体处理做成可长期运行的工作台能力。

要做：

- 下载去重、断点、校验、磁盘空间、来源记录。
- 模型能力矩阵和任务能力匹配。
- Audio/Image/Video/Subtitle 工作台统一 task/artifact/log。
- GPU OOM、backend 缺失、FFmpeg 缺失、模型缺失有稳定错误码和建议。

验收：

- `cargo check -p slab-runtime -p slab-runtime-core -p slab-app-core`
- 媒体任务失败路径能展示可操作原因。
- artifact 可导出、可删除、可追踪来源。

### 阶段 6：分发、安全与运维

目标：让 Slab 在真实桌面环境中可安装、可升级、可诊断。

要做：

- Windows installer 完整验收。
- 首次设置向导和健康检查。
- secret store、权限总览、远程访问可选开关。
- 诊断包和发布验收矩阵。

验收：

- `bun run check`
- `bun run test`
- `bun run build:app`
- 发布前 `bun run build:windows-installer`

## 7. 能力优先级

近期优先做这些，因为它们能最大化提高长期路线的地基质量：

1. 清理路线图中的外部参照依赖，并把所有路径、状态、边界改成源码可验证描述。
2. Assistant 运行闭环：模型准备、发送、等待、工具回放、审批、中断、恢复、错误可操作。
3. Task 中心闭环：模型下载、媒体任务、插件任务、Agent 任务统一可见。
4. Agent 编程工具补齐：glob、增强 grep、patch preview/apply、user.ask、plan.update。
5. Memory/Hook 管理界面：启停、审计、注入记录、失败记录。
6. MCP 持久化配置：stdio server 管理、工具目录、调用来源、resources。
7. Plugin 中心：权限、日志、启停、pack 导入、私有 registry。
8. Model/Runtime 稳定性：下载去重、ModelState 恢复、能力矩阵、OOM 错误。
9. Workspace/LSP 状态：provider 可用性、诊断、Agent 上下文选择。
10. 媒体任务工作台：Audio/Image/Video/Subtitle 的 task/artifact/log 一致化。

## 8. 远期机会池
> 这些能力不要着急做，而是应该打磨前面的功能。比如模型上下文如何压缩，如何降低token 成本，如何让本地模型和云模型协作，开源模型真实可用的数量，如何固化agent 功能并提供测试，云供应商兼容，产品UI/UX 都狠关键

这些能力可以进入长期机会池，不需要因为当前实现难度或数量多而排除，但必须在前置能力稳定后再进入主线：

- 多 workspace 同时运行与跨 workspace Agent 编排。
- Notebook 式混合文档、代码、模型输出和 artifact。
- Cron/定时任务与本地自动化。
- 远程设备配对和局域网访问。
- 团队协作、共享插件 registry、共享模型目录。
- 云端账号、同步和设备间状态同步。
- 插件签名、公钥信任链和公开市场。
- 沙箱强隔离增强，例如更细粒度网络、文件和进程权限。
- 远程 runtime worker 或多机器推理调度。
- 面向外部 IDE/CLI 的 Slab MCP server 深度集成。

## 9. 每次执行的最低校验

按改动面选择最窄验证，不为了形式跑最大集合：

- 后端 API shape：`bun run gen:api` + `cargo check -p slab-server -p slab-app-core`
- settings/model/plugin schema：`bun run gen:schemas`
- Agent core/tools/memory：`cargo check -p slab-agent -p slab-agent-tools -p slab-agent-memories`
- MCP：`cargo test -p slab-mcp-client -p slab-mcp -p slab-mcp-server`
- Plugin runtime：`cargo check -p slab-plugin -p slab-js-runtime -p slab-python-runtime` + `bun run gen:plugin-packs`
- Frontend：`bun run check:frontend`
- Runtime：`cargo check -p slab-runtime -p slab-runtime-core`
- App/package：`bun run build:app`
- 发布前：`bun run build:windows-installer`
- 全局收口：`bun run check`，必要时再跑 `bun run test`

长期路线允许分阶段推进，但每次落地必须维持四个一致性：源码一致、生成类型一致、产品文档一致、用户可见行为一致。
