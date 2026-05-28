# Slab 实现审计与结构整理报告（2026-05-28）

**审计目标**

- 从“项目声明能力”对照“当前仓库已实现能力”。
- 识别从早期到当前版本仍存在的高风险问题。
- 给出代码目录与文档目录的可执行整理建议。

**审计范围**

- 仓库入口文档：`README.md`、`README_zh.md`、`docs/`
- 服务端 API 面：`bin/slab-server/src/api/v1/`
- 历史审计汇总：`docs/development/audits/*`
- 现状抽样：`bin/slab-app/src-tauri/tauri.conf.json`、`crates/slab-proto/src/`

---

## 1. 功能实现对照（声明 vs 代码）

| 能力 | 声明状态 | 代码证据 | 结论 |
|---|---|---|---|
| 聊天（Chat） | 已支持 | `bin/slab-server/src/api/v1/chat/` | 已实现 |
| 音频转写（Audio） | 已支持 | `bin/slab-server/src/api/v1/audio/` | 已实现 |
| 图像生成（Images） | 已支持 | `bin/slab-server/src/api/v1/images/` | 已实现 |
| 模型管理（Models） | 已支持 | `bin/slab-server/src/api/v1/models/` | 已实现 |
| 任务队列（Tasks） | 已支持 | `bin/slab-server/src/api/v1/tasks/` | 已实现 |
| 插件能力（Plugins） | 已支持 | `bin/slab-server/src/api/v1/plugins/`、`plugins/` | 已实现（需继续安全收敛） |
| 视频相关工作流（Video/Subtitles） | 已支持 | `bin/slab-server/src/api/v1/video/`、`bin/slab-server/src/api/v1/subtitles/` | 已实现 |

---

## 2. 累积审计问题（按优先级）

### P0（发布前必须处理）

1. 认证覆盖面仍偏窄，部分高风险路由保护不足
- 参考历史核实：`docs/development/audits/backend-frontend-merged-audit-2026-05-26.md`
- 影响：本地服务接口在启用令牌策略时仍可能出现保护不一致。

2. 插件与宿主安全边界仍需收紧
- `withGlobalTauri: true` 仍开启：`bin/slab-app/src-tauri/tauri.conf.json`
- Plugin SDK 仍暴露通用 `invoke(command, args)` 能力（历史审计已确认）。

3. 令牌比较方式应改为常数时间比较
- 参考历史核实：`docs/development/audits/backend-frontend-merged-audit-2026-05-26.md`

### P1（下个迭代应完成）

1. CORS 与默认地址策略需要统一收敛
- 历史核实显示 `localhost` 与 `127.0.0.1`、默认端口来源存在分散定义。

2. OpenAPI 到前端类型链仍有局部断言绕过
- 参考：`docs/development/audits/slab-project-audit-2026-04-18.md`

3. Proto 转换层空值语义不完全一致
- 参考：`docs/development/audits/slab-project-audit-2026-04-18.md`

### P2（持续治理）

1. 依赖安全审计和制品校验自动化仍不完整
- 参考历史核实：`docs/development/audits/backend-frontend-merged-audit-2026-05-26.md`

---

## 3. 文档与目录结构问题（本次新发现）

1. docs 构建曾被 planning 文档语法错误阻塞
- 文件：`docs/development/planning/plan-2026-5-20.md`
- 现状：本次已修复为合法 Markdown 结构。

2. development 首页存在失效链接
- 文件：`docs/development/index.md`
- 现状：本次已改为“仅链接当前存在文件”。

3. 中英文 README 插件状态描述不一致
- 文件：`README.md`、`README_zh.md`
- 现状：本次已统一口径，中文文档改为“插件生命周期管理已具备”。

4. docs 目录命名规范不统一
- 现状：`docs/development/planning/` 中同时存在空格文件名、非统一日期格式（如 `2026-5-20`）
- 风险：URL 可读性与长期维护性下降。

---

## 4. 本次已执行整理

1. 修复文档构建阻塞
- 重写 `docs/development/planning/plan-2026-5-20.md` 为结构化合法 Markdown。

2. 修复 development 索引可达性
- 更新 `docs/development/index.md`，移除失效链接并补充现存审计/规划入口。

3. 统一 README 功能口径
- 更新 `README_zh.md` 中插件章节，和英文版本保持一致。

---

## 5. 目录结构优化建议（不破坏现有代码路径）

1. docs 命名规范化（优先）
- 建议统一为：`kebab-case` + `YYYY-MM-DD`，避免空格和不规范日期。
- 示例：`slab-agent-2026-05-25.md`（替代 `slab-agent 2026-5-25.md`）。

2. development 子目录按“用途”继续收敛
- `audits/` 仅放核实结论。
- `planning/` 仅放未完成计划。
- 完成项迁移到 `worklogs/` 或新增 `history/`。

3. 审计报告建立统一索引页
- 建议新增 `docs/development/audits/index.md`，按日期和优先级聚合。

4. 代码目录结构保持稳定，优先做入口文档和边界声明整理
- 当前仓库模块边界基本清晰（`bin/`、`crates/`、`packages/`、`plugins/`）。
- 短期不建议大规模物理移动目录，避免引入跨语言构建链路回归。

---

## 6. 结论

- 项目核心能力（聊天、转写、图像、模型、任务、插件、视频）均有明确实现落点。
- 主要问题已从“功能缺失”转为“安全边界、类型链路、文档治理和配置收敛”。
- 本次已完成可直接落地的文档修复与索引整理，后续优先建议按 P0/P1 顺序推进。