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

### RP-1（高）`slab-core` 的 ImageEmbedding 能力在主链路中无上层调用，疑似“沉没能力”
- **位置**：`slab-core/src/api/pipeline.rs`（`run_image_embedding` / `submit_image_embedding`）
- **问题描述**：核心层已完整支持 `Capability::ImageEmbedding`，但运行时仅启用 `Llama/Whisper/Diffusion` 三类后端，server/app/proto 主链路没有对 image embedding 的 API 暴露或调用入口，当前更像“预留能力”而非实际业务链路。若长期不启用，会形成维护噪音和测试负担。**需与业务方确认**是否近期要上线向量检索/多模态 embedding。 
- **影响程度**：高（长期造成误判“支持已完成”，并稀释核心链路复杂度预算）。

### RP-2（高）跨层后端标识硬编码重复，耦合到字符串约定
- **位置**：`slab-server/src/infra/rpc/gateway.rs`、`slab-server/src/infra/rpc/client.rs`、`slab-runtime/src/main.rs`
- **问题描述**：`ggml.llama/whisper/diffusion` 及其别名解析在 server gateway、server client、runtime CLI 中多处重复实现。任何新增/重命名 backend 都要求跨 crate 同步修改，属于典型“字符串协议耦合”。
- **影响程度**：高（演进新后端时高概率漏改，导致运行时可用但网关不可路由）。

### RP-3（中）`reload_library` 的 proto 契约与 `load_model` 不对称，存在历史残留风险
- **位置**：`slab-proto/src/convert.rs`（`encode_reload_library_request` / `decode_reload_library_request`）
- **问题描述**：`reload_library` 仅携带 `model_path/num_workers/context_length`，扩展字段（diffusion 路径与开关）被丢失，并在 decode 时手工构造 `ModelLoadRequest`（其余字段填空值）。这意味着 API 形态与 `load_model` 的真实能力不一致，属于“历史兼容但语义收缩”的腐烂接口。
- **影响程度**：中（重载流程在扩展能力场景下不可预测）。

### RP-4（高）`slab-server` Chat 请求参数是“超集输入”，但本地链路并未完整消费
- **位置**：`slab-server/src/api/v1/chat/schema.rs`、`slab-server/src/domain/models/chat.rs`、`slab-server/src/domain/services/chat/local.rs`
- **问题描述**：API 层接收 `thinking/reasoning_effort/verbosity/response_format/json_schema/n/stop` 等大而全参数；但本地 llama 分支仅消费 subset（例如通过 grammar/grammar_json），其他字段多数只在云分支生效或做降级。接口契约虽然“兼容 OpenAI”，但对调用方语义不够清晰，容易形成“传了不一定生效”的万能参数泥潭。
- **影响程度**：高（产品层误用概率高，行为难以解释）。

### RP-5（中）前端将非 2xx 错误包装为 200，削弱传输层语义
- **位置**：`slab-app/src/pages/chat/chat-context.ts`（`normalizeChatErrorResponse`）
- **问题描述**：前端为兼容 provider，将标准错误响应重写成 `status: 200` 且 `success:false` JSON 体。这会让监控、代理层、统一错误中间件看不到真实 HTTP 失败率，属于“协议防腐缺位后的前端兜底补丁”。
- **影响程度**：中（可用性短期提升，但观测与治理能力下降）。

### RP-6（中）会话消息存储存在多格式回退链，兼容路径持续膨胀
- **位置**：`slab-server/src/domain/models/chat.rs`（`serialize_session_message` / `deserialize_session_message`）
- **问题描述**：当前同时兼容纯文本、`StoredSessionMessage`、裸 `ConversationMessage`、裸 `ConversationMessageContent`。这是典型“历史数据兼容层内嵌到域模型”，若不设淘汰窗口会长期滞留。
- **影响程度**：中（新增字段时反序列化路径更脆弱）。

### RP-7（低）流式结束语义由 server 二次推断，存在与 runtime 结果偏差
- **位置**：`slab-server/src/domain/services/chat/local.rs`、`slab-runtime/src/grpc/llama.rs`
- **问题描述**：runtime 发送 `done` chunk，但 server 在 SSE 层还基于 token 计数与预算推断 finish_reason，并可附加 usage 估算。这是必要兼容逻辑，但存在“真实 finish_reason 与推断不一致”的边缘风险。
- **影响程度**：低（主要影响可观测字段一致性）。

---

## 2. 防腐重构建议 (ACL Strategies)

### ACL-1 统一“后端标识与能力”防腐层（跨 crate 单一真源）
- **重构方案**：在 `slab-types` 增加 `BackendDescriptor`（id/aliases/capabilities），server/runtime 仅依赖该类型做 parse/canonicalize，移除重复 `match`。
- **伪代码示例**：

```rust
// Before: 每层各自解析字符串
match raw.trim().to_ascii_lowercase().as_str() {
  "ggml.llama" | "llama" => ...
  ...
}

// After: 统一 ACL
let backend = BackendDescriptor::parse(raw)?;
let canonical = backend.id();
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

### ACL-3 为 `reload_library` 建立独立契约对象，避免“伪 load request”
- **重构方案**：proto 新增 `ReloadLibraryRequestV2 { lib_path, load_spec }`（嵌套完整 load spec）；保留旧字段兼容解析，优先新字段。
- **伪代码示例**：

```rust
// Before: decode 时手工构造 ModelLoadRequest
let load = decode_model_load_request(&pb::ModelLoadRequest { ...empty diffusion... })?;

// After: 直接 decode nested load_spec
let load = decode_model_load_spec(request.load_spec.as_ref().ok_or(...)? )?;
```

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
- **重构方案**：`SessionMessageCodecRegistry` 显式支持 v1/v2/plaintext，新增格式仅增量注册；同时为旧格式加埋点并设置下线阈值。
- **伪代码示例**：

```rust
match detect_format(raw) {
  Format::V2 => decode_v2(raw),
  Format::V1 => decode_v1(raw),
  Format::Plain => decode_plain(raw),
}
```

---

## 3. 移除策略 (Cleanup Roadmap)

### Roadmap-A：ImageEmbedding 沉没能力治理
- **第一步**：加观测
  - 在 runtime/server 增加 `capability=image_embedding` 调用计数埋点；
  - 连续 2~4 周确认生产调用为 0（或仅测试环境）。
- **第二步**：渐进清理
  - 先标记 `slab-core` embedding API 为 `#[deprecated(note = "unused in runtime chain")]`；
  - 下一里程碑移除上层不可达路径，保留独立 feature flag（便于未来恢复）。

### Roadmap-B：后端 ID 重复硬编码治理
- **第一步**：引入共享 descriptor，不改行为
  - 在 `slab-types` 增加 parse/canonicalize，老逻辑旁路比对并打 warning（双写期）。
- **第二步**：删除重复 match
  - gateway/client/runtime 全部切到 descriptor，移除本地 `match` 分支。

### Roadmap-C：`reload_library` 契约整治
- **第一步**：新增 V2 字段并灰度
  - server/runtime 同时支持旧版与新版 protobuf 字段；
  - 统计旧字段调用比例，若 >0 需与业务方确认迁移窗口。
- **第二步**：冻结旧路径
  - 文档标注旧字段 deprecated；
  - 2 个发布周期后删除旧 decode 兼容逻辑。

### Roadmap-D：聊天万能参数收敛
- **第一步**：参数生效矩阵落地
  - 在 API 文档和响应 header 中返回 `x-slab-applied-params`（或 debug 字段）明确哪些参数生效；
  - 对“传入但未生效”打 warning 日志。
- **第二步**：契约分层
  - 把本地/云专属参数放入嵌套对象，逐步弃用顶层混合字段。

### Roadmap-E：前端错误语义修复
- **第一步**：观测改造
  - 保留 UI 兼容的同时，上报真实 transport status；
  - 看板新增“provider 视角成功率 vs HTTP 真实成功率”。
- **第二步**：回收 200 包装
  - 当 provider 支持非 2xx 透传后，移除 `status:200` 重写逻辑。

---

## 结论（执行优先级）

1. **P0**：后端 ID 防腐层（ACL-1）+ 聊天参数收敛（ACL-2）。
2. **P1**：`reload_library` 契约修正（ACL-3）+ 前端错误语义修复（ACL-4）。
3. **P2**：会话格式解码器注册化（ACL-5）+ ImageEmbedding 沉没能力去留决策（需与业务方确认）。

整体建议采用“绞杀者模式”：先并行引入 ACL，保持行为等价；再按指标触发旧逻辑退场，避免一次性破坏式重构。
