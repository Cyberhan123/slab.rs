# Slab.rs 项目重构计划（基于 2026-03-31 审计）

## 摘要
- 目标不是继续叠兼容层，而是先收拢真源，再收紧契约，最后退役历史补丁。
- 实施顺序固定为 7 个工作包，分 3 个阶段推进；每个工作包可单独合并，但必须按顺序落地。
- 每个阶段结束都同步更新 OpenAPI、前端 API 类型定义和相关开发文档。

## 关键改动
- WP1 `P0` 后端标识真源收口：以 `slab-types::RuntimeBackendId` 作为唯一跨层后端 ID；给它补 `ALL`/遍历能力；`slab-server` 的 validation、backend/model/setup 服务、auto-unload、RPC gateway/client 全部改用该类型；删除 `BackendId`、`canonical_backend_id()`、`BackendKind::parse()` 这类字符串解析点。`slab-runtime` 仅保留内部 `BackendKind` 作为服务分发枚举，不再承担外部 ID 解析。
- WP2 `P0` API 基址真源收口：桌面端新增唯一 `ApiEndpointConfig`，固定产出 `gateway_bind=127.0.0.1:3000`、`api_origin=http://127.0.0.1:3000` 和对应 `connect-src`；`sidecar` 启动、`get_api_url`、plugin runtime、plugin CSP、Tauri CSP、前端 `SERVER_BASE_URL` 全部从它派生；前端只保留 `VITE_API_BASE_URL` 一个环境变量，删除 `VITE_API_URL` 和页面级默认 URL 常量。
- WP3 `P0` Chat 契约收口：域层请求拆成 `CommonChatParams`、`LocalChatParams`、`CloudChatParams`；API 层继续接受现有字段，但在模型路由确定后做显式校验，本地拒绝 `reasoning_effort/verbosity`，云端拒绝原始 `grammar`，`response_format/json_schema` 统一映射为 structured output；所有“不生效参数”统一返回 `400 unsupported_chat_parameter`，不再静默忽略。
- WP4 `P0` Chat 能力显式化：`GET /v1/chat/models` 增加 `capabilities` 字段，至少包含 `raw_grammar`、`structured_output`、`reasoning_controls`；前端聊天页按能力开关禁用不支持的控制项，不再靠失败后回退判断。
- WP5 `P1` 任务系统类型化：引入 `TaskStatus` 枚举和 `TaskPayloadEnvelope { kind, version, data }`；数据库继续保留 `TEXT` 列，但只有 repository 负责字符串与 JSON 编解码；应用服务不再直接比较 `"pending"`、`"running"`、`"succeeded"`。`/v1/tasks/{id}` 和 `/v1/tasks/{id}/result` 对外 JSON 形状保持兼容，`status` 改为枚举驱动的 schema。
- WP6 `P1` 兼容链退役：`/v1/backends/reload` 和 gRPC `ReloadLibraryRequest` 升级为完整承载 `RuntimeModelReloadSpec`，采用 `lib_path + load` 的对称结构，不再伪造 `ModelLoadRequest`；会话消息写入统一切到 `StoredSessionMessage v2` JSON envelope，纯文本和旧 JSON 只保留读兼容；云模型选择统一使用 catalog `model.id`，`cloud/{provider}/{legacy_model_id}` 仅保留一个发布窗口的翻译 shim，并打 warning/metric，窗口结束后删除。
- WP7 `P1` 前端错误语义修复：删除 `normalizeChatErrorResponse()` 的 `200 OK` 改写；新增 `ChatTransportError { transport_status, code, message, request_id }` 适配层，把失败 JSON 转成 provider 可消费对象，但保留原始 HTTP status 给日志、重试和监控；UI 可以继续展示友好错误文案，但不能再篡改传输层语义。
- WP8 `P2` 流式完成语义下推到 runtime：runtime 终止 chunk 直接透传 `finish_reason` 和 `usage`；server 只在缺失时兜底估算，并在域层标记 `finish_reason_source` 与 `usage_source`；对外响应继续沿用现有字段，其中 `usage.estimated` 必须准确反映是否为估算值。

## 公共接口与类型变更
- `RuntimeBackendId` 扩展为可遍历的唯一后端 ID 类型，并替代 `slab-server` 内部平行 `BackendId`。
- `GET /v1/chat/models` 新增 `capabilities` 字段。
- `POST /v1/chat/completions` 和 completion 路由保持请求字段兼容，但新增路由专属参数校验，之前“接受但忽略”的字段改为明确 `400`。
- `POST /v1/backends/reload` 与对应 proto 扩展为完整 reload 载荷；旧调用方在缺省新增字段时继续可用一个兼容窗口。
- `TaskResponse.status` 改为枚举驱动的 schema；`TaskResultPayload` 保持现有字段集合。
- 会话消息存储内部格式升级到 `StoredSessionMessage v2`；这是存储协议变化，不是公开 HTTP 变更。

## 测试计划
- 后端 ID：补 `RuntimeBackendId` 解析、别名、遍历单测，覆盖 backend/model/setup/RPC 路径的回归测试，确保仓库内不再有重复 canonical 解析。
- API 基址：补 Tauri Rust 单测验证 `ApiEndpointConfig`、plugin CSP 生成和 host health check；前端 smoke test 覆盖 chat、image、video、hub 都走同一 base URL。
- Chat 契约：为本地/云模型分别覆盖“支持参数通过、不支持参数 400、structured output 正常映射”的矩阵测试，并更新 OpenAPI 快照。
- 任务系统：补 repository round-trip、legacy JSON/text fallback、取消/重启语义测试，确保 API 输出不变。
- 兼容链：补 `reload_library` proto round-trip、旧会话格式读兼容、legacy cloud ID deprecation tests，并在日志/指标里确认命中次数可观测。
- 前端错误：补 provider/XRequest 适配测试，验证网络面板仍显示 4xx/5xx，同时 UI 能展示友好错误。
- 流式语义：补 runtime 到 server 的流式集成测试，校验 terminal chunk、`finish_reason`、`usage` 和 `usage.estimated` 的透传与兜底行为。

## 假设与默认值
- 本轮重构继续使用固定桌面网关地址 `http://127.0.0.1:3000`；动态端口不是本计划范围，但新的 endpoint provider 必须保留单入口，方便后续扩展。
- 除 Chat 路由新增显式 `400` 校验外，其余 HTTP JSON 形状以兼容为优先，不做破坏性改名。
- 任务表不做破坏性 schema 迁移；类型化改造先落在 domain/repository codec 边界。
- 旧云模型 ID 兼容保留一个发布窗口；旧会话格式保持读兼容直到确认历史数据已自然迁移。
