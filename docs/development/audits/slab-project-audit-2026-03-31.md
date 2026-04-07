# slab.rs 项目统一审计报告（2026-03-31）

> 来源报告：
> - `./slab-core-dataflow-audit-2026-03-31.md`
> - `./slab-server-audit-2026-03-31.md`
>
> 复核方式：基于当前仓库实现抽样核对关键代码路径，按“重复根因合并、同类问题归并、保留当前仍存在的问题”重写。

## 执行摘要

当前项目的主要问题不是单点缺陷，而是跨层契约、配置真源和兼容层边界在持续扩散。两份旧报告没有明显冲突，更多是从不同层面观察到了同一批问题：

- 一类是“单一真源缺失”，同一个概念在多个 crate、宿主层、前端层、插件层重复定义。
- 一类是“兼容补丁常态化”，历史兼容逻辑没有退出机制，逐步侵入主路径。
- 一类是“接口超集化”，API 看起来统一，但不同路由真实生效参数不同，容易形成语义错觉。

从当前代码状态看，最优先应处理的仍然是：

1. 后端标识与 API 基址的单一真源问题。
2. Chat 参数契约按路由收敛的问题。
3. 任务系统和会话/兼容链路的类型化与退役问题。

---

## 统一结论

### F1（P0）后端标识缺少单一真源，跨层仍存在重复定义与重复解析

**现状**

- `slab-types/src/backend.rs` 已定义 `RuntimeBackendId`，并提供 `FromStr` 与 `canonical_id()`。
- `slab-server/src/domain/models/backend.rs` 仍单独维护 `BackendId`。
- `slab-server/src/infra/rpc/gateway.rs` 继续手写 `canonical_backend_id()`。
- `slab-server/src/infra/rpc/client.rs` 继续手写 `BackendKind::parse()`。
- `slab-runtime/src/grpc/mod.rs` 又维护一套 `BackendKind -> RuntimeBackendId` 映射。

**问题本质**

同一语义对象已经在 `types / server / rpc / runtime` 多层重复存在。`slab-types` 虽然已经提供了更合适的真源，但目前并没有真正收拢上层调用。

**风险**

- 新增后端或调整 alias 时容易出现“某层支持、某层不支持”的分叉。
- 观测、路由、能力判断会继续依赖字符串约定。
- 任何跨层重构都需要同步修改多个位置，维护成本持续升高。

**建议**

- 以 `slab-types::RuntimeBackendId` 作为跨层唯一真源。
- `slab-server` 的领域模型、RPC gateway/client、runtime backend 路由统一复用该类型。
- 如需能力矩阵，在该类型之上增加 descriptor/capability 映射，避免再引入新的平行 ID 类型。

### F2（P0）API 基址与本地端口配置分裂，宿主、前端、插件、CSP 都在硬编码

**现状**

- `slab-app/src-tauri/src/setup/sidecar.rs` 固定 sidecar 为 `127.0.0.1:3000`。
- `slab-app/src-tauri/src/lib.rs` 使用 `SLAB_API_URL`，默认 `http://localhost:3000/`。
- `slab-app/src/lib/config.ts` 使用 `VITE_API_BASE_URL`。
- `slab-app/src/lib/tauri-api.ts` 使用 `VITE_API_URL`，并内置多个 `http://localhost:3000/` 回退。
- `slab-app/src-tauri/src/plugins/runtime.rs` 固定 `DEFAULT_API_BASE_URL = http://127.0.0.1:3000`。
- `slab-app/src-tauri/src/plugins/protocol.rs` 和 `slab-app/src-tauri/tauri.conf.json` 也把 `localhost:3000` / `127.0.0.1:3000` 写进 CSP。

**问题本质**

“API 地址”这个概念没有单一配置入口，而是在 Tauri sidecar、前端运行时、插件桥接、CSP 白名单里以不同变量名和不同默认值存在。

**风险**

- 端口调整、并行实例、测试环境切换时极易错连。
- 插件网络通道与宿主 API 实际入口可能失配。
- CSP、插件代理和应用真实路由可能逐步偏离，形成隐性故障。

**建议**

- 建立统一的 API endpoint 配置真源，由 sidecar 启动结果或宿主层统一下发。
- 前端、插件 runtime、插件 CSP 都从同一配置派生，禁止再内嵌默认端口。
- 把“宿主可访问的本地 API 地址”与“插件允许访问的 connect-src”改成同源生成。

### F3（P0）Chat 接口是“超集输入”，但本地/云路由的真实消费面并不一致

**现状**

- `slab-server/src/api/v1/chat/schema.rs` 接收 `thinking / reasoning_effort / verbosity / response_format / json_schema / n / stop / grammar` 等一整套参数。
- `slab-server/src/domain/services/chat/mod.rs` 在路由时把 `reasoning_effort / verbosity` 只传给云分支，把 `grammar / grammar_json` 只传给本地分支。
- `slab-server/src/domain/services/chat/local.rs` 本地链路最终主要消费 `grammar / grammar_json / max_tokens / temperature / top_p`。
- `slab-server/src/domain/services/chat/cloud.rs` 云链路则承担 `reasoning_effort / verbosity / structured_output` 等更多语义。

**问题本质**

API 层暴露的是一个“大而全”的统一入口，但不同路由真实生效参数不同，导致“参数被接受”并不等于“参数生效”。

**风险**

- 调用方很难判断哪些参数在当前模型/路由下有效。
- 产品层容易形成“看似兼容 OpenAI，实际行为部分降级”的误解。
- 后续新增模型类型时，参数矩阵会继续膨胀。

**建议**

- 将 Chat 请求整理为“公共参数 + 路由专属参数”。
- 在路由规划阶段显式校验参数生效性，对无效参数返回告警或错误，而不是静默忽略。
- 对外文档明确区分本地模型与云模型支持矩阵。

### F4（P1）任务系统仍依赖字符串状态与字符串化 JSON 载荷，领域契约偏弱

**现状**

- `slab-server/src/domain/services/task.rs` 使用字符串判断任务状态，如 `"succeeded" / "pending" / "running"`。
- 任务结果从 `result_data: Option<String>` 动态反序列化，失败时回退为纯文本。
- `slab-server/src/infra/db/entities/task.rs` 的 `input_data`、`result_data` 均为 `Option<String>`。
- 其他任务服务已经普遍把结构化输入先序列化为 JSON 字符串再回读解析。

**问题本质**

任务子系统目前更像“约定驱动的字符串容器”，而不是显式建模的任务领域对象。

**风险**

- 无法静态约束任务输入/输出结构。
- 任务结果可能“表面成功、语义错位”。
- 新任务类型越多，字符串状态与 JSON 形状兼容成本越高。

**建议**

- 引入类型化的任务状态枚举与任务结果 envelope。
- 为常见任务类型建立结构化 payload codec，而不是把 JSON 文本散落在服务层。
- 数据库存储层可以仍保留文本列，但领域层不应继续直接依赖裸字符串语义。

### F5（P1）历史兼容层持续滞留，已经形成多条腐烂接口与回退链

**本项合并以下旧问题**

- `reload_library` proto 契约与 `load_model` 不对称。
- 会话消息存储存在多格式回退链。
- 云模型 legacy ID 兼容仍在主路径内。

**现状**

- `slab-proto/src/convert.rs` 中 `reload_library` 仍通过手工拼装 `ModelLoadRequest` 复用解码逻辑，扩展字段无法完整表达。
- `slab-server/src/domain/models/chat.rs` 的 `deserialize_session_message()` 同时兼容 `StoredSessionMessage`、`ConversationMessage`、`ConversationMessageContent` 和纯文本。
- `slab-server/src/domain/services/chat/cloud.rs` 仍在主路径中保留 `parse_legacy_cloud_option_id()` 及扫描匹配逻辑。

**问题本质**

历史兼容逻辑没有明确退役窗口，已经从边缘兼容层进入主路径，开始影响当前设计。

**风险**

- 新字段演进会反复被旧格式拖住。
- 调试时很难判断当前到底命中了哪一条兼容分支。
- 接口表面可用，但真实语义持续收缩或漂移。

**建议**

- 为每条兼容逻辑建立明确的退役策略和时间窗。
- `reload_library` 要么升级为完整契约，要么直接退役，避免“伪 load request”长期存在。
- 会话消息格式改为版本化 codec，并限制新写入只使用单一版本。
- legacy cloud ID 若确认客户端已迁移，应尽快移出主路径。

### F6（P1）前端把非 2xx 错误改写为 200，削弱了传输层语义

**现状**

- `slab-app/src/pages/chat/chat-context.ts` 的 `normalizeChatErrorResponse()` 会把标准错误响应改写为 `status: 200`、`success: false` 的 JSON 响应。

**问题本质**

这是为兼容上层 provider 所做的前端补丁，但补丁直接覆盖了 HTTP 语义。

**风险**

- 监控、代理、网关和统一错误处理中间件无法看到真实失败率。
- 网络层错误与业务层错误被揉在一起。
- 后续接入新的 provider/adapter 时会继续放大协议歧义。

**建议**

- 把兼容逻辑下沉到 provider adapter 层。
- UI 层如需 `success: false` 包装，可以保留，但必须同时保留真实 transport status。
- 监控、日志、重试决策统一使用原始 HTTP 状态码。

### F7（P2）流式结束语义仍由 server 二次推断，`finish_reason` 与 `usage` 存在漂移空间

**现状**

- `slab-runtime/src/grpc/llama.rs` 流式过程中遇到 `done` chunk 即结束。
- `slab-server/src/domain/services/chat/local.rs` 会基于 token 计数与 `max_tokens` 再推断 `finish_reason`，并在需要时估算 `usage`。

**问题本质**

server 当前承担了 OpenAI 兼容层职责，所以需要补齐 SSE 结束块，但真实结束原因并非完全由 runtime 原样透传。

**风险**

- `finish_reason` 在边缘场景下可能与 runtime 真值不一致。
- 观测字段和计费/分析字段可能出现轻微漂移。

**建议**

- 优先让 runtime 明确透传结束原因与 usage。
- server 只在缺失时兜底推断，并把“推断值”与“原始值”区分开。

---

## 问题归并说明

两份旧报告可以归并为以下 4 个统一问题域：

1. **单一真源缺失**
   后端 ID、API 地址、插件 API 通道、CSP 白名单都存在重复定义。
2. **接口超集化**
   Chat API 对外暴露超集参数，但内部路由按不同能力子集消费。
3. **兼容层长期滞留**
   `reload_library`、legacy cloud ID、会话消息多格式回退都属于同类问题。
4. **语义补丁侵入主路径**
   前端 200 包装、server 侧 finish_reason/usage 推断，本质上都是兼容层承担了协议修补责任。

---

## 整改优先级

### 第一阶段（P0）

1. 收拢后端标识单一真源，移除重复解析。
2. 收拢 API 基址与 sidecar/插件/CSP 配置来源。
3. 将 Chat 参数改造成“公共参数 + 路由专属参数 + 明确生效校验”。

### 第二阶段（P1）

1. 任务系统建立类型化状态与结构化 payload codec。
2. 清理历史兼容链：`reload_library`、legacy cloud ID、会话消息多格式回退。
3. 修复前端错误包装策略，保留真实 HTTP 传输语义。

### 第三阶段（P2）

1. 让 runtime 更完整地上报流式完成语义，减少 server 二次推断。

---

## 旧报告映射表

| 旧报告条目 | 统一报告条目 |
| --- | --- |
| 核心链路审计 RP-1 | F1 |
| 核心链路审计 RP-2 | F5 |
| 核心链路审计 RP-3 | F3 |
| 核心链路审计 前端错误 200 包装 | F6 |
| 核心链路审计 会话多格式存储 | F5 |
| 核心链路审计 流式 finish_reason 推断 | F7 |
| server 审计 P1-1 任务万能字符串/JSON | F4 |
| server 审计 P1-2 API 基址多源硬编码 | F2 |
| server 审计 P2-1 插件 API/CSP 本地端口硬编码 | F2 |
| server 审计 P3-1 云模型 legacy ID 兼容残留 | F5 |

---

## 最终判断

当前项目的核心风险已经比较清晰：真正需要治理的不是更多兼容补丁，而是把跨层真源、路由边界和历史兼容策略重新收紧。只要继续沿着当前方式叠加功能，问题会主要表现为“配置漂移、行为不透明、兼容链膨胀”，而不是单一模块的明显崩坏。

这意味着后续整改应优先做“收口”，而不是继续“加层”。
