# Slab Desktop 前端对齐执行计划集 (2026-06-19)

> 本计划集是 [前端审计文档](../../audits/slab-deskotp-audits-2026-6-19.md) 的**直接执行响应**。
> 审计归纳出 **5 大系统性根因（R1–R5）+ 9 个 P0**，本计划集将其拆解为 **5 个相互平行、职责清晰的专项计划（A–E）** 与 **1 个综合收尾保障计划（F）**。

---

## 计划集总览

| Plan | 文件 | 根因 | 任务数 | 关键产出 |
|---|---|---|---|---|
| **A** 安全与正确性止损 | [01-security-correctness.md](01-security-correctness.md) | R3 | 6（P0×4） | 插件越权闭环、未保存守卫、Restart 门控 |
| **B** 后端能力释放 | [02-capability-release.md](02-capability-release.md) | R1 | 7（P0×2） | 媒体进度、agent 工具体系、Hub 闭环、统一错误层 |
| **C** 异步与流式状态对齐 | [03-async-streaming-reliability.md](03-async-streaming-reliability.md) | R2 | 8（P0×2） | AbortController、turn_failed 保留、SSE resume、MutationCache |
| **D** 交互体验闭环 | [04-interaction-ux.md](04-interaction-ux.md) | R4 | 12（P1×8） | 审批/重发、AI 解释代码、Monaco 懒启动、Setup 确认门 |
| **E** 设计系统重构 | [05-design-system.md](05-design-system.md) | R5 | 7（P0×1） | Token 体系、软分割线、StateSurface、reduced-motion |
| **F** 综合收尾与保障 | [06-closure-guardrails.md](06-closure-guardrails.md) | 横切 | 9（持续） | 契约同步、回归 E2E、CI、i18n、perf、回滚、跨平台 |

**根因 → 计划映射**：R1→B · R2→C · R3→A · R4→D · R5→E · 横切兜底→F

---

## 主干路线图（Waves）

| Wave | 时间 | 并行任务 | 里程碑 |
|---|---|---|---|
| **W0 止损** | Wk1 | A 全部 P0 ‖ T-E-4 ‖ T-C-1/T-C-2 | 安全闭环 + 流式可取消且不毁内容 |
| **W1 地基** | Wk2 | T-B-7 · T-E-1 · T-A-5 | 解锁下游（T-B-5/6、T-C-6、T-E-2/3/7） |
| **W2 能力+可靠性** | Wk3–4 | T-B-1/2/3/4 ‖ T-C-3/4/5/7/8 ‖ T-E-2/3/6 | 进度可视化 + SSE resume + Token 落地 |
| **W3 交互闭环** | Wk5–6 | Plan D（Track 1+2）‖ T-E-7 | 审批/重发/工作区 IDE + magic-px 清除 |
| **W4 打磨** | Wk7 | T-D-4/7/10/12 ‖ T-E-5 | 代码块复制、终端多标签、Skeleton shimmer |
| **Gate** | 持续 | F-1 ~ F-9 | 契约/回归/CI/i18n/perf/文档/回滚/平台/a11y |

完整时序、关键路径与接缝定义见 [06-closure-guardrails.md §2](06-closure-guardrails.md)。

---

## 跨计划关键交接（执行时务必对齐）

| 产出 → 消费 | 接缝 | 闭环要求 |
|---|---|---|
| T-A-1（插件鉴权）→ T-B-5/T-B-6 | 安全通道 | 卸载/更新/rpc/events 须在 T-A-1 合并后启用 |
| T-B-7（错误层）→ T-C-6 / T-B-1~6 / T-D-5,8,12 | `getLocalizedErrorMessage`/`isServerError` | **单点阻塞**，W1 不 slip |
| T-C-1（AbortController）→ T-C-3 / T-D-2 | 流式取消底层 | T-D-2 与 T-C-1 同 PR 或紧随 |
| T-E-1（Token）→ T-E-2/3/7 / B / C / D | `.glass-surface`/`--focus-ring`/`<StateSurface>` | D 在 Token 前用临时实现+TODO |
| T-D-9（Monaco 懒启动）→ T-D-6 / T-D-10 | 编辑器选区/配置 | T-D-6 依赖选区就绪 |

---

## 阅读与执行建议

1. **先读** [审计文档](../../audits/slab-deskotp-audits-2026-6-19.md) §1.4（9 个 P0）与 §4（五大最高杠杆动作），建立全局优先级。
2. **按 Wave 推进**：W0 止损优先（安全 P0 不可拖）；W1 地基（T-B-7/T-E-1）是后续并行的前提。
3. **每张任务卡**含：严重度·类型·预估 / 证据(file:line) / 问题 / 方案(可执行步骤) / 验收标准(checklist) / 依赖——可直接拆为 issue/PR。
4. **接缝变更**：任一交接产出方变更，回 [06-closure-guardrails.md §2.2](06-closure-guardrails.md) 更新并通知消费方。
5. **发布前**：逐项核对 [发布检查清单](06-closure-guardrails.md)（F-1~F-9 + 五大最高杠杆动作）。

---

## 五大最高杠杆动作（资源有限时优先）

1. **T-B-1** 消费 `task.progress` — 单点解锁全部媒体生成进度体验（Plan B）
2. **T-E-1 + T-E-2** Token 化 + 软分割线 — 通透感/无边界即时可见（Plan E）
3. **T-A-1** 插件越权止损 — 唯一安全 P0，必须先行（Plan A）
4. **T-B-7** 统一错误层 — 让全域错误"说人话、能翻译、可重试"（Plan B）
5. **T-B-3** Hub 模型管理闭环 — 补齐下载/卸载/使用（Plan B）

---

- **状态**：Draft / Pending Review
- **生成方式**：5 个专项计划由独立 Agent 并行起草并对照当前代码取证；Plan F 与本索引由架构师综合收尾。
- **上游**：[slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md)
