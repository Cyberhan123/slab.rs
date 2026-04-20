## Proto 重组与 runtime 服务止血方案 v4

### Summary
- 本次止血只重构 `slab-proto` 和 `bin/slab-runtime` 的主链路，不动 `slab-app-core`，不深切 `slab-runtime-core`。
- `slab-proto` 的文件组织改为实现边界优先：
  - `ggml/llama.proto`
  - `ggml/whisper.proto`
  - `ggml/diffusion.proto`
  - `candle/transformers.proto`
  - `candle/diffusion.proto`
  - `onnx.proto`
  - 共享原子结构继续留在 `common.proto`
- `domain/services` 先按精确 driver capability 拆开，再由 `application/services` 做受控聚合。
- handler 中移除的功能必须明确迁移归属：兼容/聚合去后续 `slab-app-core/server`，driver 原生执行所需的机械装配去对应 domain service，不能留在 handler，也不能丢数据。

### Key Changes
- `slab-proto`
  - 取消当前以 `llama.proto` / `whisper.proto` / `diffusion.proto` 为顶层 family 文件的布局，改为上面的新目录结构。
  - `ggml/llama.proto`、`ggml/whisper.proto`、`ggml/diffusion.proto` 各自只描述 GGML 的 RPC 与 DTO，不混入 Candle/ONNX 参数。
  - `candle/transformers.proto` 定义 `CandleTransformersService`，同时覆盖 Candle 文本生成、流式文本生成、语音转写，以及对应 load/unload DTO。
  - `candle/diffusion.proto` 定义 `CandleDiffusionService`，只承载 Candle diffusion RPC 与 DTO。
  - `onnx.proto` 定义 `OnnxService`，统一承载 ONNX text 与 embedding RPC；runtime 内部固定 `backend_id = onnx`。
  - `common.proto` 只保留共享原子结构，如 usage、raw image、tensor、binary payload 等，不承载跨实现的半语义请求对象。
  - `convert.rs` 改成纯 `pb <-> dto` 薄映射层，不依赖 `slab_types` 语义对象，不做默认值注入、legacy fallback、sentinel 解释。

- `bin/slab-runtime/src/domain/services`
  - 将现有统一 `BackendSession + codec` 主链路拆成 8 个底层服务：
    - `GgmlLlamaService`
    - `GgmlWhisperService`
    - `GgmlDiffusionService`
    - `CandleLlamaService`
    - `CandleWhisperService`
    - `CandleDiffusionService`
    - `OnnxTextService`
    - `OnnxEmbeddingService`
  - 每个服务只接受自己的精确 DTO，构造自己的精确 payload，调用现有 scheduler/orchestrator/pipeline，并解析自己的精确结果。
  - driver 原生执行所需的机械处理放在这里，例如：
    - proto DTO 到 backend typed payload 的无损装配
    - 本地文件到 PCM 的执行前准备
    - ONNX tensor DTO 到 ONNX worker input 的机械装配
    - backend 原生返回值到 proto result DTO 的无损搬运
  - `codec.rs` 不再作为 family 级中心。可以拆散为各服务私有 helper，或保留为非常薄的机械 helper；禁止继续承载 legacy builder、family request/response 编解码、宽松 fallback。
  - 删除 runtime 主链路中的 `slab_types` 语义转换和结果重写逻辑。

- `bin/slab-runtime/src/application/services`
  - 调整为受控聚合层：
    - `GgmlLlamaService`
    - `GgmlWhisperService`
    - `GgmlDiffusionService`
    - `CandleService`
    - `OnnxService`
  - `CandleService` 内部聚合 `CandleLlamaService`、`CandleWhisperService`、`CandleDiffusionService`。
  - `OnnxService` 内部聚合 `OnnxTextService` 与 `OnnxEmbeddingService`。
  - application 层只负责路由、状态编排和向上暴露，不承担 family 语义转换。

- `bin/slab-runtime/src/api/handlers`
  - handler 与新 proto service 对齐：
    - GGML 三个 handler：llama / whisper / diffusion
    - Candle 两个 handler：transformers / diffusion
    - ONNX 一个 handler
  - handler 只做：
    - `pb -> dto`
    - `dto -> application service`
    - `result dto -> pb`
  - handler 中移除的功能归属如下：
    - `<think>` / reasoning 提取：移出 runtime；后续由 `slab-app-core/server` 的 OpenAI-compatible 聚合层处理。runtime 只返回模型原始文本/原始流片段或 driver 原生 reasoning 字段，不解析标签。
    - usage 估算：移出 runtime；后续由 `slab-app-core/server` 在需要兼容 API 返回 usage 时估算。runtime 只透传 driver 原生 usage；没有原生 usage 就保持 unset。
    - stop 修剪：移出 runtime；后续由 `slab-app-core/server` 的请求兼容层处理。runtime 只执行 driver 原生 stop 参数；不会二次修剪文本。
    - whisper 文本拼装：移出 handler 和通用 codec；`GgmlWhisperService` / `CandleWhisperService` 返回结构化转写 DTO。需要兼容纯文本时，后续由 `slab-app-core/server` 聚合层拼接。
    - OpenAI/SSE chunk 包装：移出 runtime；后续由 `slab-app-core/server` 负责。runtime 流式返回 driver 原生 chunk DTO。
    - `RuntimeBackendLoadSpec` / `ModelSpec/load_options` 组装：删除，不迁移。新的 load 参数直接来自 proto DTO，并由对应 domain driver service 机械装配到 backend payload。
    - `slab_types` family request/response 构造：删除，不迁移。后续如果外部 API 仍需这些语义类型，由 `slab-app-core/server` 自己承担。
  - handler 不允许继续做任何兼容兜底、默认值推断或结果润色。

- `slab-runtime-core`
  - 明确冻结，不做结构迁移。
  - 不修改：
    - `backend::*`
    - `scheduler::*`
    - `scheduler / orchestrator / pipeline`
    - `slab-runtime-macros`
    - 当前错误面
  - 若实现过程中必须触及，只允许最小编译适配，不允许新增新的 family/compat 层。

### Public Interfaces
- 新 proto/service 版图固定为：
  - `ggml/llama.proto` -> `GgmlLlamaService`
  - `ggml/whisper.proto` -> `GgmlWhisperService`
  - `ggml/diffusion.proto` -> `GgmlDiffusionService`
  - `candle/transformers.proto` -> `CandleTransformersService`
  - `candle/diffusion.proto` -> `CandleDiffusionService`
  - `onnx.proto` -> `OnnxService`
- `CandleTransformersService` 同时覆盖 candle llama 与 candle whisper。
- `OnnxService` 对外统一，但内部固定 `backend_id = onnx`，由 DTO 决定走 text 或 embedding 分支。
- runtime 输出 DTO 必须保留原始/结构化数据，不能为了兼容旧 API 提前压扁成 `text`、弱 JSON 或估算字段。

### Test Plan
- 编译
  - `cargo check -p slab-proto`
  - `cargo check -p slab-runtime`
  - `cargo check -p slab-runtime-core`
- Proto/DTO
  - 验证新 proto 文件布局和 import 关系能正常生成代码，`build.rs` 正确枚举新路径。
  - 验证 `pb <-> dto` round-trip 无损，`0`、`0.0`、`false`、空字符串与 unset 可区分。
  - 验证 `CandleTransformersService` 能分别覆盖 llama 和 whisper DTO。
  - 验证 `OnnxService` 能区分 text 与 embedding，并始终固定到 `backend_id = onnx`。
- Runtime
  - 验证 domain/application 层不再通过统一 family `BackendSession` 主路径处理所有请求。
  - 验证 handler 中不再保留 reasoning、usage、stop、whisper 文本拼装、OpenAI/SSE 包装等输出兼容逻辑。
  - 验证 runtime 没有在无原生 usage 时自行估算 usage。
  - 验证 whisper 返回结构化 DTO，纯文本兼容不在 runtime 内完成。
- 搜索止血
  - 搜索确认 runtime 主链路不再依赖 `slab_types::TextGenerationRequest/Response`、`AudioTranscriptionRequest/Response`、`Diffusion*Request/Response`、`RuntimeBackendLoadSpec`。
  - 搜索确认 touched 代码中不再调用 `build_*_from_legacy`、`to_legacy_spec`、worker 内 legacy fallback 入口。

### Assumptions
- `common.proto` 继续保留为共享 import 文件，虽然不在重组清单里，但本次默认它继续存在。
- `CandleTransformersService` 是 candle 的单一“非 diffusion”对外面，内部再分 llama 与 whisper。
- `OnnxService` 在 proto 文件和 handler 上保持独立，但不进一步拆成多个 proto 文件。
- 兼容 API 行为不会在本阶段迁入 runtime；它们的最终归属是后续 `slab-app-core/server` 边界整改。
- 本阶段允许 `slab-app-core` 与整仓其余部分暂时失配；只要求 touched crates 内自洽。
- deeper cleanup of `slab-runtime-core`、错误面、macros 依赖关系留到下一阶段。
