# Slab Agent 技术设计文档（重规划版）

- 文档版本: v2026-05-26
- 状态: Implemented baseline
- 作者: Copilot
- 适用范围: slab-agent 控制面、turn 循环、工具执行管道、审批流、MCP 接入、可观测性与测试

## 1. 文档定位与替代关系

本文是对以下目标态文档的落地重规划：

- 旧文档: `docs/development/planning/slab-agent 2026-5-25.md`

关系说明：

1. 旧文档保留为“目标态愿景 + 流程蓝图”。
2. 本文定义“当前已实现事实（As-Is）+ 与目标态差距（Gap）+ 下一阶段实施设计（To-Be）”。
3. 后续开发与验收以本文为主，旧文档作为参考输入，不作为直接验收基线。

## 2. 约束与边界

本设计遵守仓库级硬约束（见 `AGENTS.md`），关键边界如下：

1. 保持现有调用链路：`bin/slab-server` -> `crates/slab-app-core` -> `crates/slab-agent`。
2. 不新增并行 API 树，继续使用 `/v1/*`。
3. 不将 HTTP 语义下沉到 `crates/slab-agent`。
4. 工具能力继续通过 `ToolRouter` 注入，避免在线程执行期动态重装配。
5. 保持插件/MCP 调用路径与现有安全边界一致。

## 3. As-Is：当前实现快照（代码事实）

本节是对当前代码状态的证据化摘要，作为 Gap 分析基线。

### 3.1 控制面与线程生命周期

已实现：

1. 根线程创建、子线程创建、输入续跑、状态订阅、优雅关闭。
2. 并发线程上限（32）与深度上限（4）。
3. 线程状态广播与持久化。
4. interrupt 与 shutdown 已解耦：interrupt 取消当前 running turn 并保留线程可继续，shutdown 终止线程。

证据：

- `crates/slab-agent/src/control.rs`
- `crates/slab-agent/src/thread.rs`
- `crates/slab-app-core/src/domain/services/agent.rs`

语义现状：

1. `/v1/agents/{id}/interrupt` 调用 `service.interrupt()`，发出 response-style cancelled 事件并将线程进入 interrupted 语义。
2. `/v1/agents/{id}/shutdown` 保持终止语义，后续 input 不可继续。

证据：

- `bin/slab-server/src/api/v1/agent/handler.rs`

### 3.2 Turn 主循环与工具执行

已实现：

1. 按 turn 组装工具规格（支持 `allowed_tools` 过滤）。
2. LLM 返回文本与工具调用分支处理。
3. 工具调用前后 hook（PreToolUse/PostToolUse）。
4. 工具审批请求/拒绝/通过路径。
5. 工具调用记录持久化与 response-style SSE 事件推送。
6. turn cancellation token 覆盖 LLM 调用、approval 等待与工具执行前后检查。

证据：

- `crates/slab-agent/src/turn.rs`
- `crates/slab-agent/src/tool.rs`

注意项：

1. 工具参数 JSON 解析失败时，当前策略是写入 tool 消息并继续 turn，不会立即中断该轮。

### 3.3 工具装配与沙箱

已实现：

1. 工具统一通过 `register_all_tools` 注入。
2. 支持 shell、fs、grep、web_search、apply_patch、git（条件）、mcp（条件）。
3. shell 工具依赖 sandbox driver 可用性，若不可用则 `ShellPolicy::Block`。

证据：

- `crates/slab-agent-tools/src/lib.rs`
- `crates/slab-app-core/src/context/mod.rs`

### 3.4 MCP 集成

已实现：

1. `McpCallTool` 与基于 `cached_tools_blocking()` 的动态代理注册。
2. `agent.tools.mcp.enabled` 作为 settings-backed 开关，默认关闭。
3. MCP proxy 工具命名使用 `mcp__{server}__{tool}`，与内建工具冲突时跳过并记录日志。

证据：

- `crates/slab-agent-tools/src/lib.rs`

当前限制：

1. app-core 仅在 `agent.tools.mcp.enabled = true` 时注入 MCP client；当前未接入持久化 MCP server 启动配置，默认仍不暴露外部 MCP 工具面。

证据：

- `crates/slab-app-core/src/context/mod.rs`

### 3.5 事件流、审批与存储

已实现：

1. SSE response event stream 输出（文本 delta/done、response lifecycle、tool、approval、compact、metrics 等）。
2. 审批决策通过 `call_id + thread_id` 双键匹配。
3. thread/message/tool_call 三类持久化。

证据：

- `bin/slab-server/src/api/v1/agent/handler.rs`
- `crates/slab-app-core/src/infra/*`
- `crates/slab-agent/src/port.rs`

### 3.6 Agent 事件通道与降级链路（server -> frontend）

当前事实（默认 agent 路径）：

1. server 暴露的是 `/v1/agents/{id}/events` SSE response event stream。
2. 前端 assistant agent hook 使用 `EventSource` 直接订阅该 SSE 接口，并解析 response-style envelope。
3. 本版不实现“WebSocket 优先 + SSE 回退”；验收口径明确为当前 SSE response event stream。

证据：

- `bin/slab-server/src/api/v1/agent/handler.rs`
- `packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts`

## 4. Gap：对照旧TD的差距矩阵

状态定义：

- Done: 已有实现且语义基本匹配。
- Partial: 有实现但语义或覆盖不足。
- Missing: 未见有效实现。

### 4.1 Phase 1 启动与初始化

1. 配置加载与工具注册: Done。
2. MCP 初始化并纳入工具装配: Done（settings-backed，默认关闭）。
3. 沙箱策略初始化: Done（可用性探测 + 阻断降级）。

### 4.2 Phase 2 Op 提交与调度

1. 用户输入触发 turn 调度: Done。
2. 审批决策回路: Done。
3. Interrupt 独立语义（中断而非关闭）: Done。
4. ConfigUpdate/SteerInput 的显式 Op 模型: Missing（当前无独立公开模型）。

### 4.3 Phase 3 Turn 主循环

1. Prompt/Tools/History 组装: Done。
2. 流式事件处理: Done。
3. WebSocket 优先 + SSE 回退（agent 事件通道，server -> frontend）: Non-goal for this baseline（当前实现为 SSE response event stream）。
4. 增量 delta 提交策略: Missing（当前为按 turn 的完整上下文调用，未形成“同 turn 仅追加 delta 输入”的可验证实现）。

### 4.4 Phase 4 工具执行管道

1. 工具执行、审批、审计记录: Done。
2. 工具风险分析: Partial（已接入统一 risk analyzer 与审批 metadata，shell AST 深度分析仍可后续增强）。
3. apply_patch 原子写入: Done（通过工具接入，具体实现在工具 crate）。
4. MCP 工具调用: Done（能力存在，默认关闭，开启后注入）。

### 4.5 Phase 5 上下文压缩

1. token 水位检测与 compact 流程: Partial（已接入阈值检查、CompactPort 与 no-op provider；尚不替换历史）。

### 4.6 Phase 6 事件输出与 UI 渲染

1. response-style 事件输出: Done。
2. 事件有序/无丢失/可重放保障与监控指标: Partial（基础回放与 metrics event 已有，后续可扩展外部指标后端）。

### 4.7 后续仍未完成或本版非目标清单（逐条对照）

本节仅统计“默认配置、默认启动路径”下仍未完成、只完成骨架、或本版明确不做的能力。

1. `ModelClientSession` 独立会话对象（含连接生命周期管理）: Missing。
说明：当前为端口化 `LlmPort` 单次调用，没有旧TD中的独立 session 对象。
证据：`crates/slab-agent/src/port.rs`、`crates/slab-agent/src/turn.rs`

2. agent 事件通道 websocket 优先 -> sse fallback: Non-goal for this baseline。
说明：当前前端直接使用 SSE EventSource 订阅 SSE response event stream，本版不实现 websocket fallback。
证据：`bin/slab-server/src/api/v1/agent/handler.rs`、`packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts`

3. Op 模型中的 `ConfigUpdate`: Missing。
说明：当前未暴露独立 op 类型与对应调度入口。
证据：`crates/slab-agent/src/control.rs`、`crates/slab-agent/src/config.rs`

4. Op 模型中的 `SteerInput`: Missing。
说明：当前仅有 `send_input` 追加用户消息，没有“回合中动态 steer”通道。
证据：`crates/slab-agent/src/control.rs`

5. 真实 compact 历史替换: Missing。
说明：当前已接入 `CompactPort`、阈值估算与 no-op provider；默认只发出 compact/metrics 事件，不替换历史。
证据：`crates/slab-agent/src/compact.rs`、`crates/slab-agent/src/thread.rs`

6. shell AST 深度风险结构化分析并参与策略决策: Partial。
说明：当前已有统一 `ToolRiskAnalyzer` 与审批 risk metadata，shell AST 深度分析可后续接入。
证据：`crates/slab-agent/src/risk.rs`、`crates/slab-agent/src/turn.rs`

7. MCP persisted server launch/config source: Partial。
说明：`agent.tools.mcp.enabled` 已控制注入路径，但 app-core 当前只构造空 MCP client，尚未接入持久化 MCP server 启动配置。
证据：`crates/slab-app-core/src/context/mod.rs`

8. 外部 metrics 后端或聚合报表: Missing。
说明：当前以 tracing fields 与 response-style metrics event 输出为基线，不引入新的外部 metrics 后端。
证据：`crates/slab-agent/src/event.rs`、`crates/slab-agent/src/thread.rs`、`crates/slab-agent/src/turn.rs`

9. 配置热更新最小集: Missing。
说明：当前仍以 next input/next turn 的配置快照为主，尚未定义独立 ConfigUpdate op。
证据：`crates/slab-agent/src/control.rs`、`crates/slab-agent/src/config.rs`

## 5. To-Be：下一阶段实施设计

## 5.1 目标

在不打破现有架构边界前提下，完成以下目标：

1. 区分 interrupt 与 shutdown 语义，补齐控制面一致性。
2. 将 MCP 作为可控开关接入默认装配路径。
3. 建立最小可用的上下文压缩骨架（阈值检测 + 接口桩）。
4. 为工具风险控制与事件可靠性建立可测验收。
5. 提升端到端测试与可观测性，确保后续迭代可回归。

## 5.2 分阶段计划

### P0（必须先做）

P0-1 Interrupt 语义落地（与 shutdown 解耦）

1. 在控制面引入“中断当前执行单元”的能力（不立即销毁线程实体）。
2. API `/interrupt` 调用中断语义，`/shutdown` 保持终止语义。
3. 线程状态机补充 Interrupting/Interrupted 或明确事件语义。

涉及文件：

- `crates/slab-agent/src/control.rs`
- `crates/slab-agent/src/thread.rs`
- `crates/slab-app-core/src/domain/services/agent.rs`
- `bin/slab-server/src/api/v1/agent/handler.rs`

P0-1b Agent 事件通道策略确认（SSE response event stream）

1. 明确 server -> frontend 的 agent 事件通道策略：当前为 SSE response event stream。
2. `/v1/agents/{id}/events` 保留 SSE 端点，payload 替换为 response-style envelope。
3. 本版不落地双通道，文档与验收口径均以 SSE response event stream 为准。

涉及文件：

- `bin/slab-server/src/api/v1/agent/handler.rs`
- `packages/slab-desktop/src/pages/assistant/hooks/use-assistant-agent.ts`
- `docs/development/planning/slab-agent-td-2026-05-26.md`

P0-2 MCP 默认装配开关化接入

1. 在 app-core 装配层增加 MCP client 注入路径（默认可关闭，配置可开启）。
2. 工具命名冲突策略显式化（内建工具优先或命名空间前缀）。

涉及文件：

- `crates/slab-app-core/src/context/mod.rs`
- `crates/slab-agent-tools/src/lib.rs`

P0-3 最小端到端回归用例

1. 覆盖 spawn -> input -> tool approval -> complete。
2. 覆盖 interrupt 与 shutdown 行为差异。
3. 覆盖 MCP 开启/关闭两种装配。

建议位置：

- `bin/slab-server/tests/*`
- `crates/slab-agent/src/tests.rs`（扩展）

### P1（重要增强）

P1-1 上下文压缩骨架

1. 在 agent 主路径加入 token 估算阈值检查接口。
2. 达阈值触发 compact provider（先接口后实现），支持 no-op fallback。
3. 记录压缩前后 token 指标与摘要替换事件。

建议落点：

- `crates/slab-agent/src/compact.rs`（新增）
- `crates/slab-agent/src/thread.rs`
- `crates/slab-agent/src/port.rs`

P1-2 工具风险分析链增强

1. shell 工具参数解析后进入统一风险分析接口。
2. 审批事件增加风险标签字段（可为空，逐步完善）。

建议落点：

- `crates/slab-agent/src/turn.rs`
- `crates/slab-agent/src/port.rs`
- `crates/slab-agent-tools/src/shell.rs`

### P2（可演进）

P2-1 可观测性

1. 增加 turn 耗时、工具耗时、审批等待时长、失败率指标。
2. 增加 lagged/replay 比例观测。

P2-2 配置热更新最小集

1. 明确可热更新字段（例如 model、allowed_tools）。
2. 保持线程内一致性（next-turn 生效）。

## 6. 验收标准（替代旧TD不可测条目）

以下验收项必须可通过自动化或明确步骤验证。

### 6.1 语义与行为验收

1. `POST /v1/agents/{id}/interrupt` 不再等同 `shutdown`。
2. `POST /v1/agents/{id}/shutdown` 后线程不可继续 `input`。
3. 被 interrupt 的线程在策略允许下可继续下一次 input。
4. 对 agent 事件通道给出可验证行为：
   - 当前实现为 SSE response event stream。
   - 不实现 websocket fallback 时，需验证文档与实现一致。

### 6.2 工具与审批验收

1. 工具审批拒绝时写入失败状态与事件，并继续可控流程。
2. 工具审批通过时写入 running -> completed 状态链。
3. MCP 开关关闭时不注册 MCP 工具，开启时可见并可调用。

### 6.3 事件流验收

1. `/events` 订阅可收到 replay + live。
2. 事件序列包含可追踪 call_id 与 thread_id 关联。
3. 在高频 tool_call 下无 panic、无线程泄漏。

### 6.4 测试命令建议

1. `cargo test -p slab-agent`
2. `cargo test -p slab-server`
3. `cargo clippy --workspace --all-targets`

## 7. 风险与回滚

### 7.1 风险

1. Interrupt 语义改造可能影响前端状态判断与轮询逻辑。
2. MCP 默认接入可能引入工具名冲突和权限暴露风险。
3. Compact 骨架若直接接入真实后端，可能引入延迟抖动。

### 7.2 回滚策略

1. interrupt 语义集中在控制面与线程取消 token，出现兼容问题可局部回滚该路径。
2. MCP 注入采用显式开关，默认关闭可快速止损。
3. compact 先 no-op provider，上线初期仅打点不替换历史。

## 8. 交付物与里程碑

M1（P0 完成）

1. interrupt 与 shutdown 行为解耦。
2. MCP 开关注入可用。
3. 关键端到端用例通过。

M2（P1 完成）

1. compact 骨架接通。
2. shell 风险标签进入审批事件。

M3（P2 完成）

1. 指标体系可观测。
2. 可热更新配置最小集落地。

## 9. 非目标（本版不做）

1. 不在本版引入分布式 agent 调度。
2. 不在本版重写模型 SDK 连接层。
3. 不在本版重做前端 UI 交互，仅保证事件契约与语义清晰。
4. 不在本版实现 WebSocket 优先或 WebSocket fallback。
5. 不在本版接入真实 compact 模型调用，compact provider 默认 no-op。

## 10. 实施顺序建议（执行清单）

1. 先做 P0-1（interrupt/shutdown 解耦）并补测试。
2. 再做 P0-2（MCP 开关注入）并补冲突策略。
3. 最后做 P0-3（E2E 回归），冻结 P0。
4. P1 与 P2 按风险优先推进，不阻塞主线发布。
