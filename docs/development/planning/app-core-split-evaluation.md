# slab-app-core 拆分评估（2026-08-31）

> **文档定位**：2026-08-31 代码整理期的立项评估产出。本轮**未动任何 app-core 代码**；
> 本文给出规模地图、拆分候选、依赖方向规则与分步路线图，供后续专项决策。
> 所有数字由当日在 `crates/slab-app-core/src` 实测（`find … | wc -l`）。

## 1. 为什么评估

`slab-app-core` 是全仓最大 crate：**193 个 .rs 文件、约 70,400 行**，占全仓 Rust 近三分之一。
后果：

- 编译面大：任何 domain/infra 改动都触发整个 crate 重编，`bin/slab-server` 顶层验证随之变慢。
- 边界腐化风险：巨石内部"随手加"没有结构性阻力，HTTP seam、持久化、协议翻译、服务编排
  混在同一编译单元，靠约定而非依赖图维持。
- 审计/测试定位成本高：单文件巨测（`openai_compat_tests.rs` 3,765 行、`rollout_store.rs`
  2,200 行、`memory.rs` 2,128 行）加剧了这一点。

## 2. 现状规模地图

| 模块 | 文件数 | LOC | 内容 |
|---|---:|---:|---|
| `domain/services/` | — | 34,608 | agent 11.6k、model 6.7k、chat 4.7k、workspace 2.7k、plugin 2.0k、llm 1.4k、ffmpeg 0.7k、task 0.2k、散件 4.6k（`setup.rs` 1,072 最大） |
| `infra/` | 69 | 23,080 | agent 10.6k（含 `rollout_store.rs` 2,200）、db 5.0k、rpc 2.3k、runtime 2.3k、model_packs 1.6k、plugin_runtime 0.8k |
| `schemas/` | 18 | 6,351 | utoipa request/response DTO |
| `context/` | 4 | 604 | 服务上下文 + axum `FromRef` seam（feature-gated） |
| `application/` | 4 | 125 | 装配层 |
| 根文件 | 7 | — | `config.rs`、`error.rs`、`launch.rs`、`model_auto_unload.rs`、`runtime_supervisor.rs`、`test_support.rs`、`lib.rs` |

既有边界约束（AGENTS.md）：HTTP-server-free（出站 reqwest 与 axum extractor feature 是仅有的两个
sanctioned seam，见 crate README）；SQLx `migrations/` append-only；LSP provider 解析与进程
spawning 留在本 crate；`bin/slab-server` 是唯一 axum feature 启用者。

## 3. 拆分候选（按 价值/风险 排序）

### C1（先行）：`slab-agent-response` — OpenAI 兼容协议翻译

- **范围**：`domain/services/agent/response/`（含 3,765 行 `openai_compat_tests.rs`）。
- **理由**：纯函数协议翻译（Slab 内部事件 ↔ OpenAI Responses DTO），依赖极少（slab-proto 的
  openai 模块 + slab-types），测试密集且自包含，是依赖最干净的切面。`/v1/agents/responses` 是
  source-of-truth 路由，SSE/WS 同 payload 的约束让它天然该有独立 crate 的显式契约面。
- **风险**：低。无 I/O、无 DB、无 axum 依赖。
- **迁移**：新 crate `crates/slab-agent-response` → app-core 依赖它 → 逐文件搬移（response/
  目录整体）→ `cargo check -p slab-server` 早编译验证消费者。

### C2：rollout 持久化归拢 — 与 `slab-agent-rollout` 合流评估

- **现状**：`crates/slab-agent-rollout` 已是 JSONL 真源 writer；app-core `infra/agent/rollout_store.rs`
  （2,200 行）是 store/service 面。两 crate 的职责切分要先行确认（谁拥有恢复乱序去重、fork/rollback
  语义），再决定是"store 下沉进 slab-agent-rollout"还是"独立 `slab-rollout-store`"。
- **理由**：rollout 是事件溯源关键路径，独立后可单独压测与 fuzz。
- **风险**：中。涉及 slab-agent 的 port 边界与 app-core 的服务编排双向引用，需要先画依赖图。

### C3：`slab-memory` — 记忆系统

- **范围**：`infra/agent/memory.rs`（2,128 行）+ 相邻记忆检索逻辑。
- **理由**：记忆系统默认开启后是活跃演进面，独立 crate 让其 schema/检索策略演进不碰 app-core。
- **风险**：中。与 agent 会话生命周期耦合较深。

### C4（谨慎）：`infra/db` + `schemas/` 的外迁

- `infra/db`（5.0k）外迁受两件事约束：`migrations/` append-only 路径稳定性（build 脚本/离线
  查询校验），以及 repository ↔ domain models 的双向密切度。
- `schemas/`（6.4k，utoipa DTO）可先于 db 外迁成 `slab-api-schemas`，但 utoipa derive 与
  axum feature 的耦合需要设计（DTO crate 不应启用 axum，只出 utoipa 注解）。
- **风险**：高。本轮只记录，不建议先做。

### C5（观察项，非 app-core）：harness 会话状态机仍在 `bin/slab-server`

`bin/slab-server/src/api/v1/agent/harness/`（mod.rs 1,283 行 + body/session/host/transform）
约 2,400+ 行连接级状态机，是 HTTP 层里最重的业务逻辑。wire 契约已在 slab-proto，但会话语义
（thread 绑定、event fanout、establish 流程）是否下沉 app-core 值得单独专题——**注意方向**：
下沉会把有状态会话管理引入 app-core，与"HTTP-server-free 但也无连接态"的当前形态冲突，
需要先定 seam（例如抽 `SessionManager` port）再动。

## 4. 依赖方向规则（任何拆分必须遵守）

1. 新 crate **不得**依赖 slab-app-core（单向：app-core → 新 crate）。
2. 跨 crate 契约优先落 `slab-types` / `slab-proto`；协议翻译类（C1）除外——它自己就是契约面。
3. 每一步迁移后立即 `cargo check -p slab-server`（顶层消费者早编译，防窄扫描漏掉传递消费者）。
4. 每步一个 commit；SQLx migration 路径与查询校验流程不动。
5. 拆分不改变行为：靠既有测试（`openai_compat_tests` 等）整体搬移护航。

## 5. 路线图（建议顺序）

1. C1 `slab-agent-response`（1 个 PR，行为零变化，测试整体搬迁）。
2. C2 依赖图确认 → rollout 归拢（1 个设计文档 + 1-2 个 PR）。
3. C3 `slab-memory`。
4. C4/C5 各出独立专题后再定。

## 6. 附：本轮记录、未执行的三个收敛项

### 6.1 slab-hub 三 HF provider 收敛（未动）

`crates/slab-hub` feature-gate 三条 provider（`hf-hub`、`huggingface-hub`、`models-cat`），
`src/client.rs` 为三者各写 list/download/cached 三套方法；根 Cargo.toml 的
`huggingface-hub = "0.0.0"` 是占位版本。模型下载是关键路径，收敛需要真实 HF 网络 e2e 验证，
本轮不动代码。建议专项：先确认三条 provider 各自的真实消费场景（本地缓存探测 / 下载 / 目录聚合），
再决定是否砍到两条。

### 6.2 workspace lints 全员启用（未动）

现状：根 `[workspace.lints.rust]` 为空，仅 `slab-agent-rollout`、`slab-apply-patch` 挂了
`[lints]`。建议渐进启用：先在新裂出的 crate（C1 起）挂 `workspace = true` lints 并配
`clippy::all = "warn"`，观察 CI（validate.ts 链）对 warning 的态度后再全员铺开。一次性全员启用
有"存量 warning 爆炸 → 大家加 allow"的反效果风险。

### 6.3 slab-agent tokio features 收紧（未动）

根 Cargo.toml 的 tokio workspace 依赖带 `features = ["full"]`，"纯 crate" slab-agent 因此在依赖
面上启用了 fs/net/process。源码当前无违规（审计核实），但约束仅靠约定。建议：拆分专项之外单独
一个小 PR 把 tokio feature 声明下放到各成员（agent 只留 sync/rt/macros），编译矩阵全量验证。

## 7. 复核清单

- [ ] C1 立项时复核 `domain/services/agent/response/` 的真实依赖闭包
- [ ] C2 前画 slab-agent-rollout ↔ app-core rollout_store 依赖图
- [ ] C5 前定 harness 会话 seam（SessionManager port 形态）
