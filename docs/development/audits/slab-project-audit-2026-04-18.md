# Slab.rs 项目审计报告（2026-04-18，核实修订版）

**日期**: 2026-04-18
**状态**: 已按当前仓库代码逐项核实并重写
**范围**: Rust 后端、Tauri 宿主、TypeScript 前端、OpenAPI 生成链路、跨层契约

---

## 执行摘要

当前仓库已经完成了一批实质性改进，尤其是在 Chat 路由参数校验、任务状态类型化、会话消息存储结构化、`slab-app-core` 统一错误类型等方面，原始报告在这些点上的积极判断大体成立。

但原始报告的几个关键结论需要修正：

- “前端把非 2xx 错误包装成 200” 的说法不准确。当前实现会保留真实的非 2xx HTTP 状态码；只有流式 SSE 中的“带错误负载但 HTTP 传输成功”的场景，才会人为标记为 `transport_status: 200`。
- “DELETE 端点 OpenAPI 类型生成普遍丢失路径参数” 的说法过宽。当前确认丢失 `path` 类型的是 `/v1/sessions/{id}` 的 `DELETE` 和 `/v1/sessions/{id}/messages` 的 `GET`；前端其他 `as unknown as` 还来自请求体、返回体和 `FormData` 集成的额外类型缺口。
- “`slab-app-core` 没有统一错误类型、全量使用 `anyhow::Result`” 不成立。当前仓库已有 `AppCoreError` 作为核心错误类型。
- “生产路径中的 panic/expect” 例子部分引用错误。原文点名的 `crates/slab-app-core/src/domain/models/model.rs` 与 `crates/slab-app-core/src/infra/runtime/process.rs` 相关 `expect`/`unwrap` 位于测试代码中，不能作为生产风险证据。

在保留已证实结论后，本次核实认为当前最重要的真实问题是：

1. Tauri CSP 仍包含 `unsafe-eval`
2. CORS 默认策略仍然过宽
3. API 基地址与端口默认值仍然分散、且 `localhost` / `127.0.0.1` 存在不一致
4. Proto 转换层仍然存在空值哨兵与静默替换问题
5. OpenAPI 到前端客户端的类型链路仍存在多个需要本地断言绕过的缺口

---

## 已核实的进展

以下判断可以保留，且已被当前代码证实：

| 项目 | 结论 | 证据 |
|---|---|---|
| Chat 路由参数校验 | 已修复 | `crates/slab-app-core/src/domain/services/chat/mod.rs` 中存在 `validate_chat_route_params()`、`validate_text_route_params()`，并分别接入聊天与文本生成流程 |
| 任务状态类型化 | 已修复 | `crates/slab-app-core/src/domain/models/task.rs` 定义了 `TaskStatus` 枚举及 `FromStr`；`crates/slab-app-core/src/schemas/tasks.rs` 也使用结构化 `TaskResultPayload` |
| 会话消息结构化存储 | 已实现且保留兼容层 | `crates/slab-app-core/src/domain/models/chat.rs` 当前序列化为 `StoredSessionMessageV2`，反序列化仍兼容 `V2`、`V1`、`ConversationMessage` 直存和纯文本回退 |
| `slab-app-core` 统一错误类型 | 已存在 | `crates/slab-app-core/src/error.rs` 定义 `AppCoreError`，并承担 `Runtime`、`Database`、`BadRequest`、`Internal` 等分类 |
| `slab-agent` 仍保持基础设施纯净 | 成立 | `crates/slab-agent/Cargo.toml` 未直接引入 `axum`、`sqlx`、`tonic` |

---

## 原报告需要修正或删除的结论

| 原结论 | 核实结果 | 修订说明 |
|---|---|---|
| F6: 前端将非 2xx 错误改写为 200 | 不成立，需删除 | `packages/slab-desktop/src/pages/chat/lib/chat-request-errors.ts` 中 `adaptChatTransportResponse()` 对非 `response.ok` 直接抛出，`transport_status` 保留真实 `response.status` |
| F6 的佐证位于 `chat-provider.ts` 中 `transport_status: 200` | 结论外推过度 | 该分支只处理 SSE 流中“HTTP 200 但 chunk 内含错误”的场景，不能推导出“非 2xx 被包装成 200” |
| NF-4: DELETE 端点 OpenAPI 类型生成普遍丢失路径参数 | 不准确，需改写 | `packages/slab-desktop/src/lib/api/v1.d.ts` 中确认缺失 `path` 类型的是 `delete_session` 与 `list_session_messages`；`delete_model`、`update_model_config_selection`、`delete_ui_state` 等操作的 `path` 类型是存在的 |
| NF-6: `slab-app-core` 全量使用 `anyhow::Result`，没有统一错误类型 | 不成立，需删除 | `crates/slab-app-core/src/error.rs` 已定义 `AppCoreError`，并被大量业务代码直接使用 |
| NF-7: `crates/slab-app-core/src/domain/models/model.rs` 的 `expect("deserialize legacy config")` 属于生产 panic 风险 | 不成立，需删除该证据 | 该调用位于 `#[cfg(test)]` 测试模块中 |
| NF-7: `crates/slab-app-core/src/infra/runtime/process.rs` 的 `expect("failed to start process")` 属于生产 panic 风险 | 不成立，需删除该证据 | 当前该文件中相关 `unwrap()`/`spawn().unwrap()` 位于测试代码，而非生产路径 |
| F5: `reload_library` proto / legacy cloud ID 兼容层仍滞留 | 未找到证据，需删除 | 当前仓库中未检索到 `reload_library` 相关符号；原结论缺少对应代码依据 |
| F5: 兼容层问题全部来自会话消息反序列化 | 需要缩窄表述 | 当前能确认的兼容层只包括 `deserialize_session_message()` 对旧格式与纯文本回退的兼容 |
| 正向发现: `slab-types` 已完全成为跨 crate 类型唯一真源 | 表述过满 | `RuntimeBackendId` 在 `slab-app-core` 等处已广泛使用，但 `bin/slab-runtime/src/domain/models/backend.rs` 的 `ResolvedBackend.backend_id` 仍是 `String` |
| NF-8 中把 `tokio` 视为多版本重复依赖 | 证据不充分 | 本次 `cargo tree -d` 明确能确认的多版本重复包括 `base64`、`reqwest`、`thiserror`、`windows`、`toml`、`zip` 等；不再单独点名 `tokio` |

---

## 修订后的真实发现

### P0-1. Tauri CSP 仍包含 `unsafe-eval`

**位置**

- `bin/slab-app/src-tauri/tauri.conf.json`

**现状**

当前 `script-src` 仍包含 `'unsafe-eval'`。这是原报告中最明确、也最应该保留的高优先级问题之一。

**影响**

- 扩大前端脚本执行面
- 与 Tauri 宿主的最小权限策略目标不一致

**建议**

- 梳理必须依赖 `unsafe-eval` 的前端依赖链
- 优先移除开发期残留配置，再回归验证插件 WebView、Markdown、路由和构建产物

---

### P0-2. CORS 默认策略过宽，空配置时会退化为 `Any`

**位置**

- `bin/slab-server/src/api/middleware/cors.rs`

**现状**

当 `SLAB_CORS_ORIGINS` 未设置时，服务端使用 `allow_origin(Any)`；即使设置了该环境变量，但解析后为空列表，也同样回退到 `Any`。

**影响**

- 本地 sidecar HTTP 服务可被任意来源跨域访问
- 与桌面宿主默认本地 API 场景不匹配

**建议**

- 默认收紧为 `http://127.0.0.1:3000` 与 `http://localhost:3000`
- 仅在显式配置时放宽
- 将“空字符串配置”视为配置错误，而不是回退为宽松策略

---

### P0-3. API 基地址与端口默认值仍然分散，且存在 `localhost` / `127.0.0.1` 不一致

**位置**

- `packages/slab-desktop/src/lib/config.ts`
- `packages/slab-desktop/package.json`
- `bin/slab-app/src-tauri/tauri.conf.json`
- `bin/slab-app/src-tauri/src/setup/api_endpoint.rs`
- `crates/slab-app-core/src/config.rs`
- `crates/slab-types/src/settings/launch.rs`
- `crates/slab-types/src/settings/v2.rs`

**现状**

本次核实发现，这不是“仅剩两三处硬编码”的问题，而是多层默认值同时存在：

- 前端默认 API 基地址是 `http://127.0.0.1:3000`
- Tauri 宿主的 `ApiEndpointConfig::desktop()` 也硬编码了 `127.0.0.1:3000`
- Tauri CSP `connect-src` 写死了 `http://127.0.0.1:3000`
- OpenAPI 类型生成脚本也依赖 `http://127.0.0.1:3000/api-docs/openapi.json`
- 但 `slab-app-core` 的 `Config::from_env()` 默认 `bind_address` 却是 `localhost:3000`

**影响**

- 端口与主机名来源分裂
- 前端、宿主、服务端与类型生成脚本之间缺少单一真源
- `localhost` 与 `127.0.0.1` 混用会增加配置漂移与排错成本

**建议**

- 统一一个主机与端口定义来源
- 让 Tauri 宿主、前端 base URL、CSP、OpenAPI 生成脚本共同消费该来源
- 避免在不同 crate / package 中重复写默认值

---

### P1-1. 运行时后端 ID 仍未完全类型化

**位置**

- `bin/slab-runtime/src/domain/models/backend.rs`

**现状**

`slab-types::RuntimeBackendId` 已在 `slab-app-core`、schema 校验、runtime supervisor 中广泛采用，但 `bin/slab-runtime` 内部的 `ResolvedBackend` 仍以 `String` 持有 `backend_id`。

**影响**

- 运行时侧仍保留字符串约定
- 类型系统无法在 runtime 侧完全约束 backend 标识

**建议**

- 继续把 runtime 侧 catalog / binding 逻辑收敛到 `RuntimeBackendId`
- 如确需保留字符串输入，至少在边界层完成一次规范化

---

### P1-2. Proto 转换层仍混用“空值过滤”和“空值报错”两种语义

**位置**

- `crates/slab-proto/src/convert.rs`

**现状**

当前同一转换文件中同时存在两种策略：

- `non_empty_string()` 会把空字符串过滤成 `None`
- `ensure_non_empty()` 会把空字符串视为错误

同时，多个数值字段仍使用 “`0` 表示未设置” 的哨兵语义，例如 `context_length`、`n_threads`、`intra_op_num_threads` 等字段在 encode/decode 时会过滤 `0`。

**影响**

- 往返转换语义不完全对称
- 调用方可能无法区分“明确传了空值/零值”和“根本未设置”

**建议**

- 为需要“可缺省”的字段显式使用 `Option`
- 为必须非空的字段统一走校验错误路径
- 减少哨兵值编码策略

---

### P1-3. Diffusion 图像响应会静默覆盖调用方传入的宽高/通道

**位置**

- `crates/slab-proto/src/convert.rs`

**现状**

`decode_diffusion_image_response()` 会解析返回图像字节中的真实尺寸与通道数，并使用：

- `width.max(metadata.width)`
- `height.max(metadata.height)`
- `max_channels(image.channels, metadata.channels)`

来决定最终值。

这意味着调用方传入的 `width` / `height` / `channels` 如果偏小，会被静默放大；如果传错，也不会得到明确错误。

**影响**

- 参数“被接受但不严格按原值生效”
- 调试和跨进程协议排查成本上升

**建议**

- 明确规定返回值以解码后的真实元数据为准，并在协议层写清楚
- 或对不一致输入返回错误，而不是静默修正

---

### P1-4. OpenAPI 到前端客户端的类型链路仍存在多个局部断言绕过

**位置**

- `packages/slab-desktop/src/pages/chat/hooks/use-chat-sessions.ts`
- `packages/slab-desktop/src/pages/chat/lib/chat-history.ts`
- `packages/slab-desktop/src/pages/audio/hooks/use-transcribe.tsx`
- `packages/slab-desktop/src/pages/image/hooks/use-image-generation.ts`
- `packages/slab-desktop/src/pages/video/hooks/use-video-generation.ts`
- `packages/slab-desktop/src/pages/hub/hooks/use-hub-model-catalog.ts`
- `packages/slab-desktop/src/lib/model-config.ts`
- `packages/slab-desktop/src/store/ui-state-storage.ts`
- `packages/slab-desktop/src/lib/api/v1.d.ts`

**现状**

原报告把这个问题归因为“DELETE 端点路径参数缺失”，但本次核实确认实际情况更复杂：

- `v1.d.ts` 中 `delete_session` 和 `list_session_messages` 的 `operations` 定义确实丢失了 `path` 参数
- 但其他 `as unknown as` 还来自不同来源：
  - 某些 `useMutation()` 返回值需要手工补全 `mutateAsync` 入参与返回体
  - `multipart/form-data` 的 `FormData` 使用仍有本地类型垫片
  - UI state 与 session history 为了获得稳定的调用面，定义了本地专用客户端类型

**影响**

- 前端多处依赖类型断言绕过
- 一旦 OpenAPI 生成或客户端库升级，局部断言容易静默失效

**建议**

- 先修复 `/v1/sessions/{id}` 与 `/v1/sessions/{id}/messages` 的 `path` 类型丢失问题
- 再单独梳理 `openapi-fetch` / `openapi-react-query` 在 202、multipart、hook 推导上的集成缺口
- 把“生成器问题”和“客户端 hook 类型推导问题”分开追踪

---

### P2-1. 会话消息兼容层仍然较宽

**位置**

- `crates/slab-app-core/src/domain/models/chat.rs`

**现状**

`deserialize_session_message()` 当前同时兼容：

- `StoredSessionMessageV2`
- `StoredSessionMessageV1`
- `ConversationMessage` JSON
- `ConversationMessageContent` JSON
- 纯文本内容回退

这证明兼容层确实还在，但问题范围应被限定为“会话消息存储兼容”，而不是原报告中未找到证据的 `reload_library` / legacy cloud ID 相关项。

**影响**

- 兼容窗口持续拉长
- 未来数据清理与格式退役成本会上升

**建议**

- 为 V1 / 直存 JSON / 纯文本回退设定退役窗口
- 增加迁移或读后写回机制，逐步收敛到 V2

---

### P2-2. 生产路径中仍有少量已确认的 panic 面，但原报告举例过多且不准确

**位置**

- `crates/slab-hub/src/client.rs`
- `crates/slab-app-core/src/context/worker_state.rs`

**现状**

本次确认的生产相关点包括：

- `crates/slab-hub/src/client.rs` 对缓存 provider 锁使用 `expect("cached provider lock")`
- `crates/slab-app-core/src/context/worker_state.rs` 的 `Debug` 实现中使用 `unwrap_or(0)` 作为 poisoned 锁降级

但原报告引用的多个例子并不成立：

- `crates/slab-app-core/src/domain/models/model.rs` 对应 `expect` 位于测试模块
- `crates/slab-app-core/src/infra/runtime/process.rs` 中点名的 `unwrap()` 也位于测试代码

**建议**

- 保留“继续压缩生产 panic 面”的方向
- 但后续审计应明确区分测试代码与生产代码，不再混引

---

### P2-3. Rust 依赖重复版本确实存在，但应以 `cargo tree -d` 实际结果为准

**现状**

本次用 `cargo tree -d` 确认当前存在多版本重复的依赖至少包括：

- `base64`
- `reqwest`
- `thiserror`
- `windows`
- `toml`
- `zip`

原报告中把 `tokio` 一并列为多版本重复项，证据不够明确，本修订版不再保留该说法。

**建议**

- 先从工作区直接依赖入手，优先减少 `reqwest`、`thiserror` 等高频基础库分叉
- 对 `hf-hub`、Tauri 生态、Windows 生态带来的间接分叉单独评估

---

## 修订后的优先级建议

### 第一阶段

1. 去掉 Tauri CSP 中的 `unsafe-eval`
2. 收紧 `bin/slab-server` 的默认 CORS 策略
3. 统一 API 基地址、端口和主机名默认值，消除 `localhost` / `127.0.0.1` 分裂

### 第二阶段

1. 修复 `v1.d.ts` 中 `/v1/sessions/{id}` 与 `/v1/sessions/{id}/messages` 的 `path` 类型缺失
2. 梳理前端 `as unknown as` 的其余来源，区分生成器问题与客户端 hook 推导问题
3. 统一 proto 空值与零值语义

### 第三阶段

1. 为会话消息兼容层设定退役窗口
2. 继续压缩真实生产路径中的 panic 面
3. 基于 `cargo tree -d` 输出收敛重复依赖版本
4. 继续推进 runtime 侧 backend ID 类型化

---

## 附录：本次核实中明确成立的反向结论

以下几点是对原报告的重要纠偏：

- `slab-app-core` 不是“没有统一错误类型”的状态，`AppCoreError` 已是当前核心错误面
- Chat 传输层不是“把所有错误都写成 200”，真实非 2xx 状态会被保留
- OpenAPI 类型问题不是“所有 DELETE 路由都坏了”，而是少数操作的 `path` 类型丢失叠加多种 hook 类型缺口
- 原报告列举的部分 `expect` / `unwrap` 风险引用到了测试代码，不应作为生产风险证据

---

*基于 2026-04-18 当前工作区代码、配置与生成类型文件核实。*
