# slab.rs 核心数据传输链路深度审计（2026-03-31）

> 审计范围：`slab-core -> slab-runtime -> slab-proto -> slab-server -> slab-app` 的推理/会话主链路（重点 Chat 文本推理）。

## 链路总览（事实基线）

1. `slab-core::Pipeline` 负责 capability 检查、模型加载、编码请求并提交任务（`InvocationPlan`）到调度器；并区分 unary/streaming 提交路径。  
2. `slab-runtime` 的 gRPC 实现（如 `llama.rs`）从 protobuf 请求解码为 `TextGenerationRequest`，再调用 `Pipeline` 执行，并把结果重新编码为 protobuf。  
3. `slab-proto::convert` 在 runtime/server 之间承担了主要 DTO 转换，包含大量“空字符串=未设置”的哨兵规则。  
4. `slab-server` 的 `ChatService` 负责 API 兼容层（OpenAI 风格）、模型路由（本地/云）、SSE 拼接、stop 截断、会话持久化。  
5. `slab-app` Chat 页面并未统一走 OpenAPI 生成客户端，而是通过 `XRequest` 直接请求 `/v1/chat/completions`，并对错误响应做“200 包装”适配。  

---

## 1. 风险点识别 (Risk Points)

### RP-1（高）跨层后端标识硬编码重复，耦合到字符串约定
**位置（例如）**：`slab-server/src/infra/rpc/gateway.rs`、`slab-server/src/infra/rpc/client.rs`、`slab-runtime/src/main.rs`、`slab-server/src/domain/models/backend.rs`（`BackendId` 的 strum 解析）、`slab-types/src/backend.rs`（`RuntimeBackendId::from_str` / `canonical_id`）

### RP-2（中）`reload_library` 的 proto 契约与 `load_model` 不对称，存在历史残留风险
- **位置**：`slab-proto/src/convert.rs`（`encode_reload_library_request` / `decode_reload_library_request`）
- **问题描述**：`reload_library` 仅携带 `model_path/num_workers/context_length`，扩展字段（diffusion 路径与开关）被丢失，并在 decode 时手工构造 `ModelLoadRequest`（其余字段填空值）。这意味着 API 形态与 `load_model` 的真实能力不一致，属于“历史兼容但语义收缩”的腐烂接口。
- **影响程度**：中（重载流程在扩展能力场景下不可预测）。

### RP-3（高）`slab-server` Chat 请求参数是“超集输入”，但本地链路并未完整消费
- **位置**：`slab-server/src/api/v1/chat/schema.rs`、`slab-server/src/domain/models/chat.rs`、`slab-server/src/domain/services/chat/local.rs`
- **问题描述**：API 层接收 `thinking/reasoning_effort/verbosity/response_format/json_schema/n/stop` 等大而全参数；但本地 llama 分支仅消费 subset（例如通过 grammar/grammar_json），其他字段多数只在云分支生效或做降级。接口契约虽然“兼容 OpenAI”，但对调用方语义不够清晰，容易形成“传了不一定生效”的万能参数泥潭。
- **影响程度**：高（产品层误用概率高，行为难以解释）。

### RP-5（中）前端将非 2xx 错误包装为 200，削弱传输层语义
- **位置**：`slab-app/src/pages/chat/chat-context.ts`（`normalizeChatErrorResponse`）
- **问题描述**：前端为兼容 provider，将标准错误响应重写成 `status: 200` 且 `success:false` JSON 体。这会让监控、代理层、统一错误中间件看不到真实 HTTP 失败率，属于“协议防腐缺位后的前端兜底补丁”。
- **影响程度**：中（可用性短期提升，但观测与治理能力下降）

### RP-5（中）会话消息存储存在多格式回退链，兼容路径持续膨胀
- **位置**：`slab-server/src/domain/models/chat.rs`（`serialize_session_message` / `deserialize_session_message`）
- **问题描述**：当前同时兼容纯文本、`StoredSessionMessage`、裸 `ConversationMessage`、裸 `ConversationMessageContent`。这是典型“历史数据兼容层内嵌到域模型”，若不设淘汰窗口会长期滞留。
- **影响程度**：中（新增字段时反序列化路径更脆弱）。

### RP-（低）流式结束语义由 server 二次推断，存在与 runtime 结果偏差
- **位置**：`slab-server/src/domain/services/chat/local.rs`、`slab-runtime/src/grpc/llama.rs`
- **问题描述**：runtime 发送 `done` chunk，但 server 在 SSE 层还基于 token 计数与预算推断 finish_reason，并可附加 usage 估算。这是必要兼容逻辑，但存在“真实 finish_reason 与推断不一致”的边缘风险。
- **影响程度**：低（主要影响可观测字段一致性）。

---

## 2. 防腐重构建议 (ACL Strategies)

### ACL-1 统一“后端标识与能力”防腐层（跨 crate 单一真源）
- **重构方案**：复用 `slab-types::backend::RuntimeBackendId` 作为“后端标识”唯一真源（`FromStr` + `canonical_id()` 已提供 alias 解析与标准化），在 server/runtime 层统一依赖该类型做 parse/canonicalize，移除重复字符串 `match`；若需要能力矩阵，再在 `RuntimeBackendId` 之上增加轻量 `BackendCapabilities` / descriptor 映射，而不是并行造一个新的 id 类型。
- **伪代码示例**（示意如何在上层复用 `RuntimeBackendId`，并可选挂载能力信息）：
```rust
use slab_types::backend::RuntimeBackendId;
use std::str::FromStr;
// Before: 每层各自解析字符串
match raw.trim().to_ascii_lowercase().as_str() {
  "ggml.llama" | "llama" => { /* llama */ }
  "whisper" => { /* whisper */ }
  // ...
  other => return Err(BackendError::Unknown(other.to_owned())),
}
// After: 统一复用 RuntimeBackendId
let backend = RuntimeBackendId::from_str(raw)?; // 或 `raw.parse::<RuntimeBackendId>()`?
let canonical = backend.canonical_id();
// 如需能力矩阵，可以在 types 层单独定义映射，而不是重新发明 id：
fn backend_capabilities(backend: RuntimeBackendId) -> BackendCapabilities {
    match backend {
        RuntimeBackendId::Llama => BackendCapabilities::TEXT | BackendCapabilities::CHAT,
        RuntimeBackendId::Whisper => BackendCapabilities::AUDIO,
        RuntimeBackendId::Diffusion => BackendCapabilities::IMAGE,
        // ...
    }
}
if backend.supports(Capability::TextGeneration) { ... }
```

### ACL-2 将 Chat 请求拆分为“公共参数 + 路由专属参数”
- **重构方案**：`ChatCompletionCommand` 拆成：
  - `ChatCommonParams`（model/messages/max_tokens/temperature/top_p/stream/n/stop）
  - `LocalLlamaParams`（grammar/grammar_json）
  - `CloudReasoningParams`（reasoning_effort/verbosity/structured_output）
  由 `RoutePlan` 在服务层组装，明确“某参数在哪条路由生效”。
- **伪代码示例**：

```rust
enum RoutePlan {
  Local { common: ChatCommonParams, local: LocalLlamaParams },
  Cloud { common: ChatCommonParams, cloud: CloudReasoningParams },
}

let plan = route_planner.plan(req)?; // 在这里做参数生效校验与告警
executor.execute(plan).await
```

### ACL-3 完全移除陈旧的 `reload_library` ，避免“伪 load request”
- **重构方案**：proto 完全移除陈旧的 reload_library 彻底清除，并深度检查。

### ACL-4 把前端错误 200 包装迁移到“Provider Adapter 层”并保留原始状态
- **重构方案**：在 adapter 返回对象里并行保留 `transport_status` 与 `provider_status`，UI 继续兼容 provider，但日志/监控按真实 HTTP status 上报。
- **伪代码示例**：

```ts
return {
  providerStatus: 200,
  transportStatus: response.status,
  body: normalizedError,
};
```

### ACL-5 为会话存储建立版本化解码器注册表
- **重构方案**：`SessionMessageCodecRegistry` 显式只只支持新格式；同时为旧格式弃用，但为防止未来出现新格式从而产生兼容问题所以建议增加版本局
- **伪代码示例**：

```rust
match detect_format(raw) {
  Format::V2 => decode_v2(raw),
  Format::V1 => decode_v1(raw),
}
```
---


## 结论（执行优先级）

1. **P0**：后端 ID 防腐层（ACL-1）+ 聊天参数收敛（ACL-2）。
2. **P1**：`reload_library` 契约修正（ACL-3）+ 前端错误语义修复（ACL-4）。
3. **P2**：会话格式解码器注册化（ACL-5）