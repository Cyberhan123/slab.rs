# Slab 全链路 API Schema / DTO 审计报告

**审计范围**：`slab-proto` → `slab-app-core` → `slab-server` → `slab-runtime`  
**审计日期**：2026-06-08  
**审计方法**：四层并行深度扫描 + 交叉比对  
**审计工具**：4 个专用 Agent（proto-auditor, app-core-auditor, server-auditor, runtime-auditor）

---

## 一、架构总览与数据流拓扑

```
┌─────────────────────────────────────────────────────────────────────┐
│                      slab-proto (契约层)                            │
│  ┌──────────────────────────┐  ┌──────────────────────────────────┐ │
│  │ openai/ (OpenAI DTOs)    │  │ slab/ipc/v1/ (gRPC Protobuf)    │ │
│  │ 手写 Rust struct/enum    │  │ .proto → tonic 生成              │ │
│  │ ~120+ 类型               │  │ 7 proto 文件, ~40+ 消息          │ │
│  │ serde JSON 序列化        │  │ protobuf 二进制序列化             │ │
│  └────────────┬─────────────┘  └──────────────┬───────────────────┘ │
└───────────────┼────────────────────────────────┼────────────────────┘
                │                                │
                ▼                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   slab-app-core (上帝层)                            │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────┐  ┌─────────────┐  │
│  │ schemas/    │  │ domain/      │  │ infra/db │  │ infra/rpc   │  │
│  │ 80+ API DTO│  │ models/ 70+  │  │ entities │  │ gateway     │  │
│  │ From/Into  │  │ From/TryFrom │  │ 22 记录  │  │ GrpcGateway │  │
│  │ ×2 双向    │  │ ×100+ 转换   │  │ JSON字段 │  │ 3 后端通道  │  │
│  └──────┬──────┘  └──────┬───────┘  └──────────┘  └──────┬──────┘  │
└─────────┼────────────────┼───────────────────────────────┼─────────┘
          │                │                               │
          ▼                │                               │
┌──────────────────────────┼───────────────────────────────┼─────────┐
│     slab-server (薄透传层)                               │         │
│  schema.rs = pub use slab_app_core::schemas::*           │         │
│  handler.rs: req.into() → service → result.into()       │         │
│  无独立 schema, 无独立转换逻辑                            │         │
└──────────────────────────┼───────────────────────────────┼─────────┘
                           │                               │
                           │         gRPC (protobuf)       │
                           └───────────────┬───────────────┘
                                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     slab-runtime (执行层)                            │
│  ┌──────────────────┐  ┌────────────────────┐                       │
│  │ bin/slab-runtime │  │ slab-runtime-core  │                       │
│  │ DTOs (From proto)│  │ Payload/StreamChunk│                       │
│  │ Domain Models    │  │ BackendProtocol    │                       │
│  │ Application Svc  │  │ Scheduler/Worker   │                       │
│  └──────────────────┘  └────────────────────┘                       │
│  6 个 gRPC 服务, 27 个 RPC 方法                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 二、发现清单（按严重程度排序）

### 🔴 P0 — API 失真（数据在传递过程中发生非预期变形）

#### P0-1: slab-server 无独立 Schema 层 — 全部透传 app-core

**位置**：`bin/slab-server/src/api/v1/chat/schema.rs` 等 16 个 schema 文件  
**现象**：所有 `bin/slab-server/src/api/v1/*/schema.rs` 都是单行 `pub use slab_app_core::schemas::*`  
**影响**：
- slab-server 无法独立于 slab-app-core 演进 API 格式
- 无法做 API 版本隔离（v1/v2 无法分治）
- HTTP 层的特定需求（如分页、错误格式、字段过滤）被迫塞入 app-core 的 schemas

**判定**：**妥协设计**。减少了代码重复，但代价是两层耦合为"同一层"。

#### P0-2: ChatToolCall/FunctionTool 三重形状 — 跨三层存在三种不同表示

| 层 | 类型 | 字段 |
|---|---|---|
| **slab-proto** (openai) | `ChatCompletionMessageToolCall` | `id`, `type`(enum), `function`(Box\<ChatCompletionMessageToolCallFunction\>) |
| **slab-app-core schemas** | `ChatToolCall` | `id`, `function`(ChatToolFunction) — **缺少 `type` 字段** |
| **slab-app-core domain** | `ConversationToolCall` (slab-types) | `id`, `function`(ConversationToolFunction) |
| **slab-agent** | `ParsedToolCall` | `id`, `name`, `arguments` — **扁平化，丢失 function 包装** |

**转换链**：
```
proto::ChatCompletionMessageToolCall
  → (缺失 From) schemas::ChatToolCall  ← 丢失 type 字段
  → domain::ConversationToolCall
  → agent::ParsedToolCall              ← 扁平化
```

**判定**：**失真**。`type` 字段在 schema 层被静默丢弃。当 proto 支持 `custom` 类型工具调用时，app-core 无法区分。

#### P0-3: ChatMessageContent 在 Domain 和 Schema 层有不同变体数

**Schema 层** (`crates/slab-app-core/src/schemas/chat.rs`) 的 `ChatMessageContent`:
```rust
enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}
```

**Domain 层** (slab-types) 的 `ConversationMessageContent`:
```rust
enum ConversationMessageContent {
    Text(String),
    Parts(Vec<ConversationContentPart>),
    // 可能有更多变体（如 tool_result 等）
}
```

**判定**：**潜在失真**。如果 domain 层添加了 schema 层不认识的变体，`From` 实现会发生什么？需要确认是否有 exhaustive match。

#### P0-4: Agent Adapter 中 ToolSpec → FunctionTool 的 description 字段被双重 Option 包装

**位置**：`crates/slab-app-core/src/infra/agent_adapter.rs:186`

```rust
function_tool.description = Some(Some(tool.description.clone()));
// proto 中 FunctionTool 的 description: Option<Option<String>>
```

**判定**：**兼容妥协**。proto 中 `description` 是 `Option<Option<String>>`（double option），但 `ToolSpec.description` 是 `String`（非空才设置）。这意味着空字符串描述会变成 `Some(Some(""))` 而非 `None`。

#### P0-5: UnifiedModelRecord → UnifiedModel 转换中所有 JSON 解析失败被静默吞噬

**位置**：`crates/slab-app-core/src/infra/db/entities/model.rs:64`

```rust
// 所有 JSON 字段解析失败时:
// capabilities → 默认空 Vec
// spec → 默认空 ModelSpec
// runtime_presets → None
// materialized_artifacts → 默认空 BTreeMap
```

**影响**：如果数据库中存储了损坏的 JSON，用户不会收到任何错误提示，模型会以"空配置"状态呈现。

**判定**：**失真**。数据损坏被掩盖，无法被检测和修复。

---

### 🟠 P1 — 有意转换但存在风险

#### P1-1: 数据库层使用字符串存储枚举类型

**位置**：`crates/slab-app-core/src/infra/db/entities/task.rs`, `model.rs`

| 字段 | 存储类型 | 实际类型 | 默认值 |
|---|---|---|---|
| `UnifiedModelRecord.kind` | `String` | `UnifiedModelKind` enum | 解析失败 → 警告日志 |
| `UnifiedModelRecord.status` | `String` | `UnifiedModelStatus` enum | 解析失败 → `Error` |
| `TaskRecord.task_type` | `String` | enum | - |
| `ChatMessage.role` | `String` | `"user"/"assistant"/"system"` | - |

**判定**：**妥协设计**（便于 SQLite 兼容）。但缺少 CHECK 约束和迁移路径。

#### P1-2: DateTime 格式在跨越 DB → Domain → Schema 时发生转换

```
DB:    DateTime<Utc>  (chrono)
  ↓ From
Domain: String (RFC3339)  [SessionView.created_at]
  ↓ serde
Schema/API: String (RFC3339)  [SessionResponse]
```

**判定**：**有意转换**。统一为 RFC3339 字符串对外暴露是合理的，但 Domain 层不应持有 String 类型的时间——丢失了类型安全的时区信息。

#### P1-3: ChatStreamChunk 只有一个 Data 变体但被定义为 enum

**位置**：`bin/slab-server/src/api/v1/chat/handler.rs:137`

```rust
ChatStreamChunk::Data(data) => data,
```

这个 enum 只有 `Data(String)` 一个变体。如果未来添加新变体（如 Error、Done），当前代码不做 exhaustive match。

**判定**：**有意设计**（预留扩展），但当前是过度抽象。

#### P1-4: Runtime 层 GGML vs Candle 后端的请求参数数量差异巨大

| 请求类型 | 字段数 | 差异 |
|---|---|---|
| `GgmlLlamaChatRequest` | 15+ | 完整参数 |
| `CandleChatRequest` | 3 | prompt, max_tokens, session_key |
| `GgmlDiffusionGenerateImageRequest` | 15 | 完整参数 |
| `CandleDiffusionGenerateImageRequest` | 8 | 简化参数 |

**判定**：**有意设计**。Candle 是轻量后端，功能子集是合理的。但 DTO 层复用了 `LlamaChatResponse`，意味着 Candle 返回的 `reasoning_content`（deprecated）字段永远为空。

#### P1-5: 错误类型链跨 5 层转换，每层都在扩展

```
Engine Error → CoreError → RuntimeError → AppCoreError → ServerError → tonic::Status/OpenAiError
```

**判定**：**有意分层**。但 `CoreError` 的 `GGMLEngine(String)` / `CandleEngine(String)` 丢失了原始结构化错误信息。

---

### 🟡 P2 — 行为不符合标准

#### P2-1: `/v1/chat/models` 端点标记为 Deprecated 但仍可访问

**位置**：`bin/slab-server/src/api/v1/chat/handler.rs:69-85`

```rust
summary = "Deprecated chat model listing compatibility route",
description = "Compatibility wrapper over GET /v1/models filtered by capability=chat_generation.",
```

OpenAPI 文档标注 deprecated 但 HTTP 层面未返回 `Deprecation` header 或 `Sunset` header。

**判定**：**兼容妥协**。前端应迁移到 `GET /v1/models?capability=chat_generation`。

#### P2-2: `AgentResponsesClientMessage` 使用 `#[serde(untagged)]` 枚举

**位置**：`crates/slab-app-core/src/schemas/agent.rs`

untagged enum 的反序列化顺序敏感——如果两个变体有重叠字段，先匹配的胜出。

**判定**：**妥协设计**。这是 JSON-RPC 风格消息的常见模式，但需要严格测试变体优先级。

#### P2-3: proto 中 `ChatMetadata.stop` 是必需字段但 DTO 中是 Option

**proto** (`common.proto`):
```protobuf
message ChatMetadata {
  optional string reasoning_content = 1;
  ChatStopMetadata stop = 2;  // 非optional → 必需
  optional string extra_json = 3;
}
```

**Runtime DTO** (`bin/slab-runtime`):
```rust
pub struct ChatMetadata {
    pub reasoning_content: Option<String>,
    pub stop: Option<ChatStopMetadata>,  // Option ← 原为必需
    pub extra_json: Option<String>,
}
```

**判定**：**兼容妥协**。runtime 层防御性地将 proto 的必需字段降级为 Option，避免反序列化失败。但这掩盖了 proto 定义与实际使用的分歧。

#### P2-4: `SESSION_MESSAGE_STORAGE_VERSION = 2` 但无 v1→v2 迁移逻辑

**位置**：`crates/slab-app-core/src/infra/db/entities/chat.rs` 相关

版本常量已定义为 2，但未找到版本 1 数据的迁移代码。

**判定**：**遗留问题**。如果存在 v1 格式的 session 数据，加载后会静默失败或行为异常。

---

### 🔵 P3 — 有意的设计决策（记录在案）

#### P3-1: slab-server 作为薄代理层

handler 统一模式为 `req.into() → service → result.into()`，零业务逻辑。这让 slab-server 只关注 HTTP 传输层（路由、验证、序列化）。

**评价**：合理的分层选择。Tauri IPC 桥接和 HTTP API 共享同一套 schema。

#### P3-2: app-core 直接依赖 slab-types 和 slab-proto

- `domain/models/chat.rs` 直接 `use slab_proto::openai::FunctionTool`
- `domain/models/*.rs` 大量 re-export `slab_types::*`

**评价**：违反了严格的分层隔离，但避免了转换代码爆炸。在有 100+ 转换的前提下，这是务实的妥协。

#### P3-3: Runtime 层的 Payload 类型擦除

`slab-runtime-core` 使用 `Payload` enum（None/Bytes/F32/Text/Json/Typed）进行后端间通信，在编排器层面放弃类型信息。

**评价**：runtime-core 作为通用后端抽象层，类型擦除是必要的。代价是转换边界需要运行时类型检查。

---

## 三、全链路追踪矩阵（关键 API 端到端）

### 3.1 Chat Completions 完整链路

```
[Client] POST /v1/chat/completions
  │
  ▼ JSON deserialize
schemas::ChatCompletionRequest                   ← slab-app-core (re-exported by slab-server)
  │ From<ChatCompletionRequest>
  ▼
domain::ChatCompletionCommand                    ← slab-app-core/domain
  │ ChatService::create_chat_completion()
  ▼
┌─ Local 路径 ─────────────────┐  ┌─ Cloud 路径 ──────────────┐
│ GrpcGateway → gRPC call      │  │ HTTP → 外部 API           │
│ proto::GgmlLlamaChatRequest  │  │ proto::openai models      │
│   ↓ protobuf encode          │  │   ↓ JSON serialize        │
│ slab-runtime                  │  │ 远程 OpenAI API            │
│   ↓ DTO decode               │  │   ↓ JSON deserialize       │
│ runtime::GgmlLlamaChatReq    │  │ ChatCompletionResponse     │
│   ↓ From<dto>                │  │   ↓ From                   │
│ TextGenerationOptions         │  │ domain::ChatCompletionResult│
│   ↓ BackendRequest(Payload)  │  └───────────────────────────┘
│ BackendWorker (engine)        │
│   ↓ EngineOutput              │
│ BackendReply(Payload)         │
│   ↓ DTO encode                │
│ proto::GgmlLlamaChatResponse  │
└───────────────────────────────┘
  │ ChatCompletionResult
  ▼
domain::ChatCompletionResult                    ← slab-app-core/domain
  │ From<ChatCompletionResult>
  ▼
schemas::ChatCompletionResponse                 ← JSON serialize → [Client]
```

**转换节点统计**：
- Local 路径：7 次类型转换
- Cloud 路径：4 次类型转换
- 其中 **有失真的节点**：P0-2 (ToolCall type 丢失), P0-5 (DB JSON 吞噬)

### 3.2 Model Load 链路

```
[Client] POST /v1/models/load
  → schemas::LoadModelRequest
  → domain::ModelLoadCommand
  → ModelService::load_model()
  → GrpcGateway → proto::GgmlLlamaLoadRequest
  → runtime DTO → domain::LoadConfig
  → BackendRequest(Payload)
  → Engine load → proto::ModelStatusResponse
  → domain::ModelStatus
  → schemas::ModelStatusResponse
  → [Client]
```

**转换节点**：8 次，无失真。

### 3.3 Agent Responses 链路 (WebSocket/SSE)

```
[Client] WS/SSE /v1/agents/responses
  → AgentResponsesClientMessage (untagged enum)
  → handle_agent_command()
  → AgentService.spawn/send_input/approve_call/...
  → AgentEvent (broadcast channel)
  → AgentStreamEvent (serialization)
  → WebSocket Message::Text / SSE Event
  → [Client]
```

**特殊注意**：Agent 适配器中 `ToolSpec → FunctionTool` 存在 P0-4 的双重 Option 问题。

---

## 四、统计数据

| 指标 | 数量 |
|---|---|
| **slab-proto OpenAI DTOs** | ~120+ 类型 |
| **slab-proto gRPC 消息** | ~40+ message |
| **slab-app-core DB 实体** | 22 类型 |
| **slab-app-core Domain 模型** | 70+ 类型 |
| **slab-app-core API Schema** | 80+ 类型 |
| **slab-app-core From/Into/TryFrom** | 100+ 实现 |
| **slab-server schema 独立类型** | 0（全部 re-export） |
| **slab-runtime gRPC 服务** | 6 个服务 |
| **slab-runtime RPC 方法** | 27 个方法 |
| **HTTP API 端点** | 75+ 路由 |
| **WebSocket 端点** | 4 个 |
| **SSE 端点** | 2 个 |
| **全链路转换节点（Chat）** | 7 次（local）/ 4 次（cloud） |
| **JSON 序列化 DB 字段** | 10+ |
| **跨 crate proto 依赖点** | 5 处 |
| **已识别失真点** | 5 (P0) |
| **已识别风险转换** | 5 (P1) |
| **已识别非标准行为** | 4 (P2) |

---

## 五、修复建议

### 🔴 P0 修复

| 编号 | 修复方案 | 工作量 | 影响范围 |
|---|---|---|---|
| **P0-1** | 在 slab-server 中引入独立的 schema wrapper，至少对需要 diverge 的类型建立本地别名 | 小 | slab-server |
| **P0-2** | 给 `ChatToolCall` 添加 `tool_type: Option<ChatToolCallType>` 字段，在 From 转换中填充 | 小 | slab-app-core schemas |
| **P0-3** | 审查 `ChatMessageContent` ↔ `ConversationMessageContent` 的 From impl，确认 exhaustive 覆盖，添加 fallback 日志 | 小 | slab-app-core schemas |
| **P0-4** | 修复 `FunctionTool.description` 的赋值逻辑：空描述不应设置 `Some(Some(""))` | 极小 | agent_adapter.rs |
| **P0-5** | 将 `TryFrom<UnifiedModelRecord>` 中的 JSON 解析失败改为返回 `Err` 而非静默默认值，至少在 log 中记录 WARNING 并在 API 响应中添加 `warnings` 字段 | 中 | slab-app-core infra/db |

### 🟠 P1 修复

| 编号 | 修复方案 | 工作量 | 影响范围 |
|---|---|---|---|
| **P1-1** | 为 DB 字符串枚举字段添加 `CHECK` 约束（新 migration） | 中 | migrations |
| **P1-2** | Domain 层 `SessionView.created_at` 改为保留 `DateTime<Utc>` 类型，到 schema 层才转 RFC3339 | 小 | domain + schemas |
| **P1-4** | 为 Candle 后端创建独立的简化 Response 类型，而非复用 GGML 的 | 中 | slab-proto + runtime |
| **P1-5** | 将 `CoreError::GGMLEngine(String)` 改为结构化错误类型 | 大 | runtime-core |

### 🟡 P2 修复

| 编号 | 修复方案 | 工作量 |
|---|---|---|
| **P2-1** | 给 deprecated 端点添加 `Deprecation` + `Sunset` HTTP header | 极小 |
| **P2-3** | 统一 proto `ChatMetadata.stop` 的 optional/required 语义 | 小 |
| **P2-4** | 添加 v1→v2 session 消息格式迁移逻辑 | 中 |

---

## 六、架构建议（长期方向）

### 6.1 引入 Schema 独立层

```
当前:  slab-server/schemas = pub use slab_app_core::schemas::*
建议:  slab-server/schemas = 本地 wrapper + 有选择地 diverge
```

在 slab-server 中为需要版本化或需要 HTTP 特定行为的端点建立独立 schema，其余仍 re-export。这样可以在不破坏 app-core 的情况下演进 HTTP API。

### 6.2 将 FunctionTool 从 domain 层移除

当前 `domain/models/chat.rs` 直接依赖 `slab_proto::openai::FunctionTool`。建议：
- 在 domain 层定义自己的 `ToolDefinition` 类型
- 在 service 层或 adapter 层做 `ToolDefinition ↔ FunctionTool` 转换

### 6.3 建立 DTO 生命周期文档

建议建立明确的生命周期约定：
```
Request (schema) → Command (domain) → Model (domain) → View (domain) → Response (schema)
```
并标注每个阶段哪些字段可以丢失、哪些必须保留。

### 6.4 为 JSON 序列化的 DB 字段引入 Guard

建议使用 newtype wrapper（如 `JsonField<T>`）替代裸 `String`，在序列化/反序列化时统一处理错误，而非散落在各个 `TryFrom` 实现中。

---

## 七、补充发现（来自各层审计专家）

### 7.1 Server 层补充

- **AgentConfigInput 未暴露完整配置**：`max_depth` 和 `max_threads` 字段使用默认值，前端无法控制
- **ChatCompletionRequest.thinking 被拆解**：`thinking` 字段在转换过程中被拆解为 `reasoning_effort` + `verbosity`
- **认证覆盖不一致**：75+ 路由中仅 `/v1/backends/*` 和 `/v1/settings/*` 有认证中间件

### 7.2 Runtime 层补充

- **27 个 RPC 方法**：分布在 6 个 gRPC 服务中
- **deprecated 字段仍返回**：`GgmlLlamaChatResponse.reasoning_content` 已标记 DEPRECATED，应使用 `metadata.reasoning_content`
- **Candle 后端极简请求**：`CandleWhisperTranscribeRequest` 仅 1 个 optional 字段（path）

### 7.3 App-core 层补充

- **媒体任务缺少 From impl**：`ImageGenerationTaskRecord → View`、`VideoGenerationTaskRecord → View` 等无 From 实现，需手动构造
- **Plugin 转换缺失**：未找到 `PluginStateRecord → PluginView` 的显式转换
- **Session 消息版本无迁移**：`SESSION_MESSAGE_STORAGE_VERSION = 2` 已定义但无 v1→v2 迁移逻辑

---

## 八、审计结论

整体架构是务实的妥协设计，四层之间的转换逻辑基本正确。主要风险集中在：

1. **静默吞噬的 JSON 解析错误**（P0-5）— 最可能在生产中导致用户可感知问题
2. **ToolCall 类型在跨层传递中丢失 `type` 字段**（P0-2）— 影响 custom tool call 支持
3. **slab-server 完全没有独立 schema 导致无法做 API 版本化**（P0-1）— 长期架构债务

---

*报告生成时间：2026-06-08*  
*审计工具：4-Agent 并行审计团队（proto-auditor, app-core-auditor, server-auditor, runtime-auditor）*
