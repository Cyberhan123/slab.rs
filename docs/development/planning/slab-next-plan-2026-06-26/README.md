# Slab Next 规划集（2026-06-26）

> 本规划集由一次**跨职能多 agent 会议模拟**产出。会议以"用户视角 + 插件开发视角"为主轴，围绕把 Slab 从"多页面并列的 AI 工具箱"升级为"以 Agent 为统一入口的本地优先 AI 工作台"展开。
> 所有结论以现有源码为准（关键事实已交叉验证到 `file_path:line`），遵守 [AGENTS.md](../../../AGENTS.md) 边界红线，参考了 2 份外网研究（多 agent 编排 + agentic UI）。
> 文档存在滞后性；**不确定时以代码为准**。

---

## 0. 一句话结论

Slab 的"统一入口"不是把页面塞进对话，而是把 Assistant 升级为 **AgentShell**——一个以 Agent 为编排者、通过**受控 a2u tool call** 派发受信 surface 的 shell。它的成立依赖四个隐藏地基同时修复：① 结构化终止纪律（`task.complete` default-deny + DAG 规划 + 确定性校验）；② 循环兜底走 `interrupt` 不硬杀；③ "假完成"数据一致性（`max_turns` 耗尽被标 `Completed` 的代码级隐患）；④ 能力可达性发现。

---

## 1. 文件清单

| 文件 | 性质 | 说明 |
|---|---|---|
| [00-meeting-conclusions.md](00-meeting-conclusions.md) | **会议结论（核心）** | 14 条 ADR、6 能力支柱、Phase 0-4 路线、风险、非目标。所有 TD 的母决策。 |
| [01-product-design.md](01-product-design.md) | **产品设计文档** | 北极星、目标用户与场景、统一入口三态语义、任务总结 action、北极星指标、产品里程碑。 |
| [02-ux-ui.md](02-ux-ui.md) | **前端 UX/UI 建议文档** | AgentShell 信息架构、多窗口/tab 取舍、a2u 渲染管线与状态机、AgentTimeline、线框与组件清单。 |
| [03-frontend-td.md](03-frontend-td.md) | **前端开发计划 TD** | 路由/shell 重构、多窗口基建、a2u-dispatcher、任务总结 action、AgentSurfaceStore、任务卡 + Wave。 |
| [04-backend-td.md](04-backend-td.md) | **后端开发计划 TD** | Plan Mode/DAG、`task.complete` default-deny、循环检测、假完成修复、ToolContext 扩容、a2u 工具注册边界。 |
| [05-infra-td.md](05-infra-td.md) | **基础设施 Infra 工程 TD** | 多窗口进程/CSP、sidecar 受控迁移、统一 secret store、可观测性、并发预算与熔断、诊断包、CI 门。 |
| [06-research-digest.md](06-research-digest.md) | 附录 A · 研究摘要 | 2 份外网研究（Anthropic/OpenAI/Google/Vercel 等）的结构化借鉴与反模式。 |
| [07-meeting-minutes.md](07-meeting-minutes.md) | 附录 B · 会议纪要 | 6 角色（PM/插件/前端/后端/SRE/Agent 架构师）原始发言、提案、补充痛点、开放问题。 |
| [08-redteam-report.md](08-redteam-report.md) | 附录 C · 红队报告 | 对抗性审查：过度工程、边界违规、被忽略视角、must_add / must_cut（已反向修正各 TD）。 |

**阅读顺序建议**：先 `00` 建立全局 → `01` 理解产品语义 → 按角色读对应 TD（前端 `02`+`03` / 后端 `04` / SRE `05`）→ 需要追溯时查附录 `06-08`。

---

## 2. 能力支柱（6 个）

| 支柱 | 一句话 | 主映射痛点 |
|---|---|---|
| **P1 统一入口 AgentShell + a2u 受控派发** | Assistant 常驻主区，原页面降级为 surface，host 侧固定派发表 | 痛点1/2 |
| **P2 插件 a2u 四段闭环 + 能力注入** | `PluginCapabilityKind::a2u_surface` + host 注册 `plugin.open` + capability→tool 闭合 | 痛点2 |
| **P3 Agent 多轮编排** | Plan DAG + `task.complete` default-deny + 循环兜底 + 假完成修复 | 痛点4 |
| **P4 审批分级 + 本地优先信任模型** | 强化 `ToolRiskAnalyzer` 为 allow/sandbox/ask 三态，分级由 app-core 静态配置 | 审批一刀切/隐私 |
| **P5 workspace 项目化 + 能力可达性** | sidecar 优雅重启 + task 受控迁移 + settings 合并语义 + capabilities 发现 | 痛点3 |
| **P6 兜底与可观测** | 诊断包白名单 + log rotation + secret store + 预算看板 + 并发熔断 | secret 落盘/限流 |

---

## 3. 决策索引（ADR-001..014）

| ADR | 决策 | 主 owner |
|---|---|---|
| 001 | 定义"统一入口"三态语义 + host 侧固定派发表 | 产品/前端 |
| 002 | 主窗内 surface 状态机为最小改造路径（非 Tauri WebviewWindow） | 前端 |
| 003 | plan 升级带依赖边 DAG + mark_done/replan + 持久化进 slab-agent-memories | 后端/Agent |
| 004 | `task.complete` default-deny 结构化完成判定工具 | 后端/Agent |
| 005 | 修复 max_turns 耗尽被标 Completed 的"假完成" + 结构化终止理由 | SRE/后端 |
| 006 | 循环/重复检测兜底（走 interrupt 不走 shutdown） | Agent |
| 007 | 扩容 ToolContext 注入 workspace/session/记忆/计划句柄 | 后端 |
| 008 | 工具审批三态分级（allow/sandbox/ask），强化现有 ToolRiskAnalyzer | 后端/Agent |
| 009 | 插件 a2u 四段闭环 + capability→tool 注册 | 插件/后端 |
| 010 | 任务总结三动作按钮（open/review/feedback）+ AgentTimeline | 产品/前端 |
| 011 | 修复失效跨页路径 + 统一 AgentSurfaceStore | 前端 |
| 012 | workspace 切换 = sidecar 优雅重启 + running task 受控迁移 | SRE |
| 013 | 并发预算可配置 + 背压降级（含 GLM 限流） | SRE |
| 014 | 诊断包字段白名单 + log rotation + secret redaction | SRE |

---

## 4. 路线图（Phase 0-4，对齐 slab-goal-plain 阶段 0-6）

| Phase | 周期 | 目标 | 关键 exit criteria |
|---|---|---|---|
| **0 基线+契约收敛** | 2-3 周 | 不动业务，收敛契约/失效路径，红线纪律工程化 | ADR-007/011 落地、CI gen 门、settings 合并语义钉死、诊断包白名单签字、log rotation 上线 |
| **1 统一入口 MVP** | 3-4 周 | AgentShell + a2u 派发 + 任务总结三动作 | ADR-001/002/008/010、内置 a2u 工具、北极星指标埋点、E2E"打开 slab.rs" |
| **2 Agent 编排增强** | 4-5 周 | Plan Mode + default-deny + 循环兜底 + 假完成修复 | ADR-003/004/005/006、AgentTimeline、E2E"达上限可续跑" |
| **3 插件 a2u + workspace 智能化** | 4-5 周 | 插件四段闭合、workspace 项目化、能力可达性 | ADR-009/012、capabilities.available、确定性 verify 工具、插件 a2u 模板 |
| **4 多 agent 成熟+场景化+分发安全** | 5-6 周 | 并行委派、场景 onboarding、离窗化、secret store、安装器健康检查 | ADR-013/014、subagent 并行、统一 secret store、CI 场景化集成测试门 |

---

## 5. 边界纪律（贯穿所有 TD）

所有 TD 共同遵守的硬边界（违反即否决，见 [AGENTS.md](../../../AGENTS.md)）：

- 确定性逻辑（DAG / `task.complete` / verify / plan 标准化）→ `crates/slab-agent-tools`；`slab-agent` 保持纯编排。
- 插件/API 能力（`plugin.open` / `open_project` / a2u）→ **host 层**注册，经 port trait 注入 app-core，**不进 `slab-agent-tools`**，不让 app-core 反向依赖 `slab-plugin`。
- `slab-app-core` 保持 HTTP-free；诊断包 host-only；多窗口全落 host 层。
- 只扩 `/v1/*`；LSP 只走 `/v1/workspace/lsp/{language}`；SQLx migration 只追加。
- 终止理由复用现有 `update_thread_status(.., Option<&str> reason)`，**零 migration**。
- caller id 从 WebView label 推导（a2u 最高风险红线）。

---

## 6. 产出方式与可信度

- **会议模拟**：6 角色视角 brainstorm（PM / 插件生态 / 前端·UX / 后端 / SRE / Agent·AI 架构师）+ 2 份外网研究 → 主席综合（14 ADR + 路线）→ 红队对抗审查（must_add/must_cut 反向修正）→ 5 份 TD 并行撰写。
- **代码取证**：所有 As-Is 引用均交叉验证到 `file_path:line`（如 [turn.rs:198](../../../crates/slab-agent/src/turn.rs#L198)、[thread.rs:424-451](../../../crates/slab-agent/src/thread.rs#L424)、[plan.rs:91](../../../crates/slab-agent-tools/src/plan.rs#L91)、[subagent.rs:117](../../../crates/slab-agent-tools/src/subagent.rs#L117)、[risk.rs:7](../../../crates/slab-agent/src/risk.rs#L7)、[plugin.rs:392](../../../crates/slab-types/src/plugin.rs#L392)）。
- **可信度声明**：文档是 To-Be 规划，落地前仍需以代码为准；少量内部交叉链接在多 agent 并行撰写时可能使用工作标题，已统一归一化到本 README 的文件清单。

> 上游文档：[slab-goal-plain-2026-06-12.md](../slab-goal-plain-2026-06-12.md)、[slab-source-of-truth-2026-06-13.md](../slab-source-of-truth-2026-06-13.md)、[slab-agent-td-2026-05-26.md](../slab-agent-td-2026-05-26.md)、[workspace-mode-design.md](../../workspace-mode-design.md)。
