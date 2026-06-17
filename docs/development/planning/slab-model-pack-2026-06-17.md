# model_pack v3 增强型模型配置包专项设计 (2026-06-17)

> **文档定位**：本规划书基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §3.1 的 11 项 `model_pack` 字段缺陷（F1–F11）与 §2.3 的跨边界缺陷（F-Stack-1/3/4），在现有 `crates/slab-model-pack` 配置体系上演进出一套**字段无歧义、多引擎可回滚、多源可路由、面向未来模态可扩展**的 v3 规范。
>
> **方法**：首席架构师主导 + Agent Team 三路并行深度设计（A：本地多引擎/变体 Schema；B：云端模型 + 多源下载路由；C：引擎切换/回滚机制 + 实施路线图），三路结论经主架构师直接读源码核实后整合。所有 `path:line` 证据已对齐 2026-06-17 工作树。
>
> **读者**：实现该规范的工程师与审计员。本文为**契约级设计**，非概念稿。

---

## 1. 背景与目标

### 1.1 现状与痛点

`slab.rs` 的模型配置以 `model_pack`（一个 `.slab` zip，内含 `manifest.json` + 一组被 `ref://` 引用的子文档）为载体，经 `crates/slab-model-pack` 解析、`crates/slab-app-core` 编目入库、`runtime_bridge` 编译成 `RuntimeBackendLoadSpec`，最终由 `slab-runtime` 执行。该链路工程完成度高，但审计暴露三类系统性短板（详见 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §1.1）：

1. **字段语义结构性模糊（§3.1 F1–F11）**——这是本规范要解决的核心：
   - **引用字段双词汇表**：manifest 入口用 `$config`（[manifest.rs:102](../../../crates/slab-model-pack/src/manifest.rs#L102)），子文档内用 `$load_config`/`$inference_config`（[manifest.rs:450](../../../crates/slab-model-pack/src/manifest.rs#L450)），同名族却意义不同。
   - **`variant.id` 可重复且静默 last-wins**：[resolve.rs:147](../../../crates/slab-model-pack/src/resolve.rs#L147) `BTreeMap::insert` 覆盖，已发布的 [Qwen3.5-9B/manifest.json:71-80](../../../models/llama/Qwen3.5-9B/manifest.json#L71-L80) 与 Qwen2.5-0.5B 各带一个重复 `Q4_K_M`，schema 不校验、解析器不报错。
   - **懒校验**：`chat_template` 形态只在运行期 `compile_default_runtime_bridge()` 才校验（[pack.rs:197-258](../../../crates/slab-model-pack/src/pack.rs#L197-L258) 只查 kind/scope/id-match，不查 payload 形态），裸字符串 `"chatml"` 能入库展示直到加载才崩。
   - **死重量 / 误用字段**：`BackendConfigDocument.id` 必填但无消费者（[manifest.rs:523](../../../crates/slab-model-pack/src/manifest.rs#L523)）；`status:"ready"` 把运行期状态写进静态属性（[justinpinkney_miniSD/manifest.json:6](../../../models/diffusion/justinpinkney_miniSD/manifest.json#L6)）；`manifest.version` 全仓库无消费者（所有 pack 写死 `"version":2`）。

2. **多引擎能力缺失**——核心引擎基于 GGML/GGUF，需支持回退（Rollback）至 Candle。但**当前"引擎"仅仅是 `backend_hints.prefer_drivers[0]` 这一个值**（[runtime_bridge.rs:227-235](../../../crates/slab-model-pack/src/runtime_bridge.rs#L227-L235) `resolve_runtime_backend` 取 `prefer_drivers` 第一项，`:214-225` 丢弃其余）。没有回退链、没有"GGML 加载失败→换 Candle"的机制。变体只携带 GGUF 量化文件，引擎与产物格式（GGUF vs safetensors）完全解耦——若误选 Candle，会把 `.gguf` 路径塞给 Candle 在加载时崩溃（F2 类懒校验）。

3. **多源下载表达力不足**——`slab-hub` 已支持 HF / ModelScope / HF-rust 三 Provider 的 probe+cache+fallback（[client.rs:132-183](../../../crates/slab-hub/src/client.rs#L132-L183)），但 manifest 层缺乏"同元数据、不同仓库 URL / 不同下载策略"的原生声明，镜像源（自定义 base endpoint）无处表达，凭证无法以"引用"方式安全挂载。

### 1.2 目标

| 目标 | 衡量标准 |
|------|----------|
| **G1 字段无歧义** | 每个字段有且仅有一个语义、守护一个不变式；消灭 `is_local`/`engine_type`/`$config` vs `$load_config` 类双字段共控。对齐审计 F1/F2/F4/F5/F7/F8/F10。 |
| **G2 多引擎可回滚** | 本地 pack 声明**有序引擎链**，GGML 主用、Candle 兜底；加载失败可观测地切换下一个引擎，不静默重试。 |
| **G3 多源可路由** | 变体/manifest 原生声明 HF + ModelScope + 镜像的多源候选，`slab-hub` 按 priority 解析消费；凭证按引用、永不落盘明文。 |
| **G4 面向未来可扩展** | 新架构（SSM/Mamba、RWKV）与新模态纳入时**零 Schema 改动**，仅作 `#[non_exhaustive]` 枚举的增量 Rust PR。 |
| **G5 可批量生成、人类可读** | 规范是纯 JSON 字面量，`bun run gen:model-packs` 可从 HF 仓库元数据批量产出；自上而下可读。 |

### 1.3 非目标

- 不重做 `ref://` 寻址（[refs.rs](../../../crates/slab-model-pack/src/refs.rs)）与子文档 kind 体系（variant/preset/backend_config/component/adapter）；**演进，不重造**。
- 不改动 `slab-runtime` 内部各 backend 的加载实现，只改"如何选定并回退 backend"。
- 不在本文落地 PMID 热重载拓宽（属 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §3.2 / F-Stack-1 的独立治理项），但 §4 会明确引擎回退**不消费**热重载路径。

---

## 2. 架构设计原则

### 2.1 单一职责与高清晰度（严禁模糊控制）

> 审计 §1.1 第一短板即"字段双词汇表 / 多字段共控"。本规范的首要原则。

**P1. 一个决策，一个字段。** 每个字段守护且仅守护一个不变式。禁止两个字段协同控制同一决策。下表是 v3 的"字段—不变式"对照（完整版见 §3.3）：

| 决策 | v2（歧义） | v3（单源） |
|------|-----------|-----------|
| 本地 vs 云端路由 | 由 `PackSource::Cloud` 反推（[manifest.rs:142](../../../crates/slab-model-pack/src/manifest.rs#L142)），无显式标记 | `deployment: "local"\|"cloud"` —— **唯一判别式** |
| 用哪个推理引擎 | `backend_hints.prefer_drivers[0]`（[runtime_bridge.rs:227](../../../crates/slab-model-pack/src/runtime_bridge.rs#L227)） | `engines[]` 有序链首项为主用，余为回退 |
| 变体产物格式 | 隐式约定 `file.id == variant.id`（[resolve.rs:247-263](../../../crates/slab-model-pack/src/resolve.rs#L247-L263)） | `variant.format` 显式声明，与引擎 `format` 精确连接 |
| 指向子文档 | `$config`（入口）/ `$load_config`（变体内）双词汇 | 入口统一 `$ref`；`$load_config`/`$inference_config` 仅指代"带 scope 的 backend_config"，两者语义正交、不撞名 |
| **load 参数归属** | variant 与 preset **双载** `$load_config`，`preset ?? variant` 覆盖（[resolve.rs:338-349](../../../crates/slab-model-pack/src/resolve.rs#L338-L349)） | `$load_config` **只**在 variant（产物/引擎相关） | **F4/F5** |
| **inference 参数归属** | variant 与 preset **双载** `$inference_config`（如 [Q4_K_M.json:5](../../../models/llama/Qwen3.5-9B/variants/Q4_K_M.json#L5) + [default.json:7](../../../models/llama/Qwen3.5-9B/presets/default.json#L7)） | `$inference_config`（= generation_config）**只**在 preset（用例/采样相关） | **F4/F5** |
| 生成配置来源 | 单设 `generation_config` 声明块（本规范草案曾提议） | **不引入**；inference payload 即生成配置，HF 字段在代码转换、未映射 `warn!` | G1 |

**P2. 校验前移，拒绝懒校验（对齐 F2）。** 任何可在导入期判定的非法形态（重复 id、payload 非对象、asset-ref 形态错误、变体↔引擎不兼容），必须在 `ModelPack::from_bytes`（[pack.rs:42](../../../crates/slab-model-pack/src/pack.rs#L42)）即报错，**不得拖延到 `compile_default_runtime_bridge()` 运行期**。

**P3. 三轴正交。** `engine`（用哪个 backend）≠ `format`（产物在磁盘上的格式）≠ `scope`（load 还是 inference 参数）。三者各管一件事，组合后唯一确定一条加载路径，无交叉。

### 2.2 高扩展性（Extensibility）

**P4. 开放枚举 + 三元组表达新模型。** 一个新模型/模态完全由 `(family, capability, engine)` 三元组 + 一个 `artifact.format` 表达。经核实，`ModelFamily`（[runtime.rs:12-22](../../../crates/slab-types/src/runtime.rs#L12-L22)）、`Capability`（[runtime.rs:24](../../../crates/slab-types/src/runtime.rs#L24)）、`RuntimeBackendId`（[backend.rs:8](../../../crates/slab-types/src/backend.rs#L8)）**均已是 `#[non_exhaustive]`**；新增 Mamba/RWKV 是增量 Rust PR，**零 Schema 改动**。本规范需补齐的是 `slab-model-pack` 侧三个**未**标注 `#[non_exhaustive]` 的枚举：`PackDocumentKind`、`BackendConfigScope`、`PackSource`（见 §3.4）。

**P5. payload 是逃生舱。** 引擎专属的新参数 rides inside `BackendConfigDocument.payload: Value`（[manifest.rs:529](../../../crates/slab-model-pack/src/manifest.rs#L529)，已是任意 JSON 对象），新模态无需改 Schema。

### 2.3 多源原生（Multi-Provider）

**P6. 元数据同构、仓库异构。** 多源候选共享同一逻辑文件清单，仅 `repo_id`/`endpoint`/`revision`/下载策略不同。`sources[]` 是有序 `PackSourceCandidate`，`priority` 显式（小者优先，并列按声明序，复用 [resolve.rs:265-274](../../../crates/slab-model-pack/src/resolve.rs#L265-L274) 的稳定排序）。

**P7. 凭证按引用、永不按值。** pack 文件不含任何明文密钥；云端凭证通过 `provider_id` 引用操作员的 `chat.providers[]` 注册项（已由 [pmid_service.rs:971](../../../crates/slab-config/src/pmid_service.rs#L971) `secret()` 脱敏），下载凭证通过 `secret_ref: pmid://...` 句柄引用。

---

## 3. Model Pack 核心 Schema 设计

### 3.1 顶层结构（discriminated by `deployment`）

v3 的 `manifest.json` 顶层以 `deployment` 为**唯一**判别式：

```
manifest.json (v3)
├── schema_version: 3                 # F10：接线、受版本门控（原 version 无消费者）
├── deployment: "local" | "cloud"     # G1：唯一路由判别式
├── id / label / family / capabilities / context_window / pricing / metadata / footprint
│
├── [deployment="local"]
│   ├── engines[]        # G2：有序引擎链（主用 + 回退）；每项 { id, format }
│   ├── sources[]        # G3：多源下载候选
│   ├── variants[]       # 产物变体：拥有 format + $load_config（加载参数单一归属）
│   ├── presets[]        # 用例预设：拥有 variant_id + $inference_config（采样参数单一归属）
│   ├── components[] / adapters[]   # 可选：多文件复合产物 / LoRA 等
│   └── default_preset
│
│   严格职责分离（消除 v2 variant 与 preset 双载 $load/$inference 的冲突）：
│     • variant  = 产物（format）+ 加载参数（$load_config）        ← load 唯一归属
│     • preset   = 用例（variant_id）+ 采样参数（$inference_config）← inference 唯一归属
│     • 两类文档各管正交内容，$ref 不再冲突
│
└── [deployment="cloud"]
    └── cloud { provider_id, remote_model_id, preferred_api_base?, credentials? }
                                        # 云端只管端点 + 凭证，无 engines / 无 runtime
```

> **为什么用 `deployment` 而不是 `family`/`is_local`**：审计 D8 指出 `models.kind`（仅 `local|cloud`）与诸多 "kind" 撞名；v3 把这个语义提升为顶层显式枚举字段 `deployment`，且**云端 pack 不允许出现 `engines`**（validator 强制），从结构上消灭"半本地半云端"的歧义形态。

### 3.2 完整配置示例

#### 3.2.1 本地模型：多量化 + 多引擎/变体（Qwen3.5-9B，GGML 主用 + Candle 兜底）

> 注释（`// …`）仅为说明，真实产物无注释。

```jsonc
{
  "$schema": "https://slab.reorgix.com/manifests/v3/slab-manifest.schema.json",
  "schema_version": 3,
  "deployment": "local",
  "id": "Qwen3.5-9B",
  "label": "Qwen3.5-9B",
  "family": "llama",
  "capabilities": ["chat_generation", "text_generation"],
  "context_window": 32768,                    // 全局上下文上限 = 训练 ctx（缺省由 gen:model-packs 从 HF config.json max_position_embeddings 提取）；运行期受 GPU 显存钳制，不进 load_config

  "engines": [
    { "id": "ggml.llama",   "format": "gguf" },        // 主用：GGUF 量化，CUDA/CPU
    { "id": "candle.llama", "format": "safetensors" }  // 回退：safetensors 原始权重
  ],

  "sources": [
    {
      "kind": "hugging_face",
      "repo_id": "unsloth/Qwen3.5-9B-GGUF",
      "files": [
        { "id": "model",  "path": "Qwen3.5-9B-BF16.gguf" },
        { "id": "Q4_K_M", "path": "Qwen3.5-9B-Q4_K_M.gguf" },
        { "id": "Q5_K_M", "path": "Qwen3.5-9B-Q5_K_M.gguf" },
        { "id": "Q8_0",   "path": "Qwen3.5-9B-Q8_0.gguf" }
      ],
      "priority": 0
    },
    {
      "kind": "model_scope",
      "repo_id": "unsloth/Qwen3.5-9B-GGUF",
      "files": [
        { "id": "model",  "path": "Qwen3.5-9B-BF16.gguf" },
        { "id": "Q4_K_M", "path": "Qwen3.5-9B-Q4_K_M.gguf" },
        { "id": "Q5_K_M", "path": "Qwen3.5-9B-Q5_K_M.gguf" },
        { "id": "Q8_0",   "path": "Qwen3.5-9B-Q8_0.gguf" }
      ],
      "priority": 1
    }
  ],

  "variants": [
    { "id": "Q4_K_M", "label": "Qwen3.5-9B-Q4_K_M", "format": "gguf",        "$ref": "ref://variants/Q4_K_M.json" },
    { "id": "Q5_K_M", "label": "Qwen3.5-9B-Q5_K_M", "format": "gguf",        "$ref": "ref://variants/Q5_K_M.json" },
    { "id": "Q8_0",   "label": "Qwen3.5-9B-Q8_0",   "format": "gguf",        "$ref": "ref://variants/Q8_0.json" },
    { "id": "bf16",   "label": "BF16 (safetensors)","format": "safetensors", "$ref": "ref://variants/bf16_safetensors.json" }
  ],

  "presets": [
    { "id": "default",       "label": "Default (GGML)",         "variant_id": "Q4_K_M", "$ref": "ref://presets/default.json" },
    { "id": "precise",       "label": "Precise (T=0.3)",        "variant_id": "Q5_K_M", "$ref": "ref://presets/precise.json" },
    { "id": "candle-fallback","label": "Candle (safetensors)",  "variant_id": "bf16",   "$ref": "ref://presets/candle.json" }
  ],
  "default_preset": "default",

  "metadata": { "author": "QwenTeam" },
  "footprint": { "ram_mb": 12288, "vram_mb": 8192 }
}
```

> **preset 与 variant 的绑定 = 引擎选择**：`default` 选 `Q4_K_M`（gguf）→ 主引擎 `ggml.llama`；`candle-fallback` 选 `bf16`（safetensors）→ 主引擎 `candle.llama`。同一份 sampler 配置（inference）可被任意 preset 复用——safetensors 变体由此被正确表达为"一个可选用的 preset + 变体"，而非孤立声明。

> **generation_config 即 inference**：不再单设 `generation_config` 声明块。inference backend_config 的 payload **就是**生成配置；HF `generation_config.json` 的字段名由 `gen:model-packs` 在**代码中转换**写入 inference payload（映射表见下），无法映射的字段一律 `warn!`（不静默丢弃，对齐审计 G4）。

> **`context_length` 是全局运行期量，不进 load_config**：它受 GPU 显存钳制，本质是"本次能开多大窗口"的运行期决定，而非 variant 的静态加载参数。模型自有**训练 ctx**（HF `config.json` 的 `max_position_embeddings`）。因此：
> - **上限**：manifest 顶层 `context_window`（与云端同字段同语义）——作者声明，或缺省取训练 ctx（`gen:model-packs` 从 `config.json` 提取）。
> - **生效值**：运行期 = min(用户请求, `context_window`)，再由可用 VRAM 钳制（ggml/candle 据此分配 KV cache）。
> - **不在** `load_config.payload`：`load_config` 只保留"如何加载"（`num_workers`/`chat_template`/`gbnf`/`flash_attn`/`device`）。这同时消解了"long-context preset 靠 override context_length"的旧诉求——上下文是运行期滑块（钳于训练 ctx × 显存），不是 pack/preset/variant 字段。

子文档示例（**variant 只载 load，preset 只载 inference**——两类文档内容正交，`$ref` 不冲突）：

```jsonc
// variants/Q4_K_M.json —— 量化变体（GGUF → ggml.llama）。拥有 format + 加载参数，不含 inference
{
  "kind": "variant",
  "id": "Q4_K_M",
  "label": "Qwen3.5-9B-Q4_K_M",
  "format": "gguf",                       // 与 manifest.engines[*].format 精确连接，决定可用引擎
  "$load_config": "ref://configs/load_ggml.json"
}

// variants/bf16_safetensors.json —— 原始权重变体（safetensors → candle.llama）
{
  "kind": "variant",
  "id": "bf16",
  "label": "BF16 (safetensors)",
  "format": "safetensors",
  "$load_config": "ref://configs/load_candle.json"
}

// presets/default.json —— 用例预设。绑定 variant + 拥有 inference（采样参数），不含 load
{
  "kind": "preset",
  "id": "default",
  "label": "Default (GGML)",
  "variant_id": "Q4_K_M",
  "$inference_config": "ref://configs/inference.json",
  "footprint": { "vram_dynamic_mb": 6144 }
}

// presets/candle.json —— 同一份 inference，仅换绑到 safetensors 变体 → 走 candle.llama
{
  "kind": "preset",
  "id": "candle-fallback",
  "label": "Candle (safetensors)",
  "variant_id": "bf16",
  "$inference_config": "ref://configs/inference.json"
}

// configs/load_ggml.json —— load scope backend_config（加载参数；归 variant）
//   注意：context_length 不在此处。它是全局运行期量（见 manifest.context_window）
{
  "kind": "backend_config",
  "scope": "load",
  "label": "GGML llama load defaults",
  "payload": {
    "num_workers": 4,
    "chat_template": {
      "id": "qwen3.5_chat_template",
      "name": "qwen3.5_chat_template",
      "$path": "ref://configs/chat_template.jinja"
    }
  }
}

// configs/inference.json —— inference scope（采样参数 = 生成配置；归 preset）
//   payload 由 gen:model-packs 从 HF generation_config.json 代码转换写入，未映射字段 warn!
{
  "kind": "backend_config",
  "scope": "inference",
  "label": "Default inference (HF generation_config)",
  "payload": {
    "max_tokens": 81920,
    "temperature": 0.6,
    "top_p": 0.95,
    "top_k": 20,
    "min_p": 0.0,
    "presence_penalty": 0.0,
    "repetition_penalty": 1.0
  }
}
```

**HF `generation_config.json` → inference payload 转换映射表**（`gen:model-packs` 在代码中实现；**未覆盖的字段一律 `warn!`**，不静默丢弃）：

| HF 字段 | inference payload 字段 | 处理 |
|---------|----------------------|------|
| `temperature` / `top_p` / `top_k` / `repetition_penalty` / `presence_penalty` / `min_p` | 同名 | 直映 |
| `max_new_tokens`（优先）/ `max_length` | `max_tokens` | `max_length` 减去 prompt 长度启发式 |
| `do_sample: false` | `temperature: 0.0`, `top_p: 1.0` | 贪婪解码扁平化 |
| 其它一切未列出字段 | — | **`warn!` 记录字段名与值，不写入** |

**HF `config.json` → 全局字段**（同一转换器在 `gen:model-packs` 中处理；未映射 `warn!`）：

| HF `config.json` 字段 | 全局字段 | 处理 |
|----------------------|---------|------|
| `max_position_embeddings`（优先）/ `n_positions` / `seq_length` | manifest `context_window` | 训练 ctx，作为上下文上限缺省值（运行期再被显存钳制） |
| `model_type` / `architectures` | manifest `family`（辅助推断） | 仅在作者未显式声明时启发 |

#### 3.2.2 云端模型（端点 + 凭证管理）

云端 pack **永不触碰 runtime**——经核实 `compile_default_runtime_bridge()` 对 `PackSource::Cloud` 硬拒（[runtime_bridge.rs:289-293](../../../crates/slab-model-pack/src/runtime_bridge.rs#L289-L293)，测试 `rejects_cloud_source_when_building_runtime_bridge` 在 `:651-702` 钉死）；云端模型在聊天期经 [chat/cloud.rs:82](../../../crates/slab-app-core/src/domain/services/chat/cloud.rs#L82) `should_route_to_cloud`（判据 `UnifiedModelKind::Cloud`，v3 由 `deployment:"cloud"` 在编目入库时派生）走 HTTP。

```jsonc
{
  "$schema": "https://slab.reorgix.com/manifests/v3/slab-manifest.schema.json",
  "schema_version": 3,
  "deployment": "cloud",
  "id": "qwen-max-dashscope",
  "label": "Qwen-Max (DashScope)",
  "family": "qwen",
  "capabilities": ["text_generation", "tool_use", "streaming"],
  "context_window": 1000000,
  "pricing": { "input": 0.0024, "output": 0.0096 },

  "cloud": {
    "provider_id": "dashscope-cn",                         // 引用 chat.providers 注册项，非字面端点
    "remote_model_id": "qwen-max-latest",
    "preferred_api_base": "https://dashscope.aliyuncs.com/compatible-mode/v1",
    "credentials": { "secret_ref": "pmid://chat.providers/dashscope-cn" }
  },

  "sources": [
    { "kind": "cloud", "provider_id": "dashscope-cn", "remote_model_id": "qwen-max-latest" }
  ],
  "variants": [{ "id": "default", "label": "Default", "$ref": "ref://variants/default.json" }],
  "presets":  [{ "id": "default", "label": "Default", "variant_id": "default", "$ref": "ref://presets/default.json" }],
  "default_preset": "default"
}
```

> 云端 `variants`/`presets` 仅用于 UI 默认与计费展示；`backend_hints`/`engines` 对云端**忽略且禁止声明**（validator 拒绝 `deployment:"cloud"` 出现 `engines`）。`provider_id` 指向操作员的 [chat.providers[]](../../../crates/slab-config/src/settings/config.rs#L11-L28) `CloudProviderConfig`，`api_base`/`api_key` 在聊天期由 [cloud.rs:317-333](../../../crates/slab-app-core/src/domain/services/chat/cloud.rs#L317-L333) `resolve_cloud_catalog_model` 从实时配置读取——**pack 永不携带密钥**。

#### 3.2.3 多源下载（HF + ModelScope + HF 镜像）

```jsonc
{
  "schema_version": 3,
  "deployment": "local",
  "id": "qwen2.5-7b-instruct",
  "label": "Qwen2.5 7B Instruct",
  "family": "llama",
  "engines": [{ "id": "ggml.llama", "format": "gguf" }],

  "sources": [
    {
      "kind": "hugging_face",
      "repo_id": "Qwen/Qwen2.5-7B-Instruct-GGUF",
      "revision": "v0.1",
      "endpoint": "https://huggingface.co",            // 可选，缺省取 DEFAULT_HF_ENDPOINT
      "files": [ /* 同逻辑文件清单 */ ],
      "priority": 0
    },
    {
      "kind": "model_scope",
      "repo_id": "Qwen/Qwen2.5-7B-Instruct-GGUF",
      "revision": "master",
      "endpoint": "https://www.modelscope.cn",
      "files": [ /* 同逻辑文件清单 */ ],
      "priority": 1
    },
    {
      "kind": "hugging_face",                           // 复用 HF 协议，换 base endpoint 即得镜像
      "repo_id": "Qwen/Qwen2.5-7B-Instruct-GGUF",
      "revision": "v0.1",
      "endpoint": "https://hf-mirror.com",
      "credentials": { "secret_ref": "pmid://download.hf_mirror" },
      "files": [ /* 同逻辑文件清单 */ ],
      "priority": 2
    }
  ],
  "variants": [{ "id": "q4_k_m", "label": "Q4_K_M", "format": "gguf", "$ref": "ref://variants/q4_k_m.json" }],
  "presets":  [{ "id": "default", "label": "Default", "variant_id": "q4_k_m", "$ref": "ref://presets/default.json" }],
  "default_preset": "default"
}
```

> 镜像用**既有 `PackSource::HuggingFace` + 可选 `endpoint` 字段**表达，**不新增 `PackSource` 变体**——避免污染 `HubProvider` 闭集（决策依据见 §5.2）。

### 3.3 关键字段逐一解释（如何避免层级混乱）

| 字段 | 类型 | 必填 | 守护的不变式（单一语义） | 解决审计 |
|------|------|------|------------------------|---------|
| `schema_version` | `u32` | 是 | manifest 规范版本，loader 拒绝不匹配值；`gen:schemas` 每版本发一 schema | **F10** |
| `deployment` | `"local"\|"cloud"` | 是 | **唯一**的本地/云端路由判别式；`engines`/`sources`/`variants` 仅 `local` 合法 | G1 / 消灭 is_local 歧义 |
| `context_window` | `Option<u32>`（顶层全局） | 否 | 上下文**上限**（= 训练 ctx）。缺省由 `gen:model-packs` 从 HF `config.json` `max_position_embeddings` 提取；运行期生效值 = min(请求值, `context_window`)，再被显存钳制。**禁止**作为 `load_config.payload` 字段（context_length 已从 load 移出） | 清晰分层 |
| ~~`load_config.payload.context_length`~~ | **移除** | — | context_length 是运行期/显存量，非静态加载参数。从 load payload 移除；旧 pack 迁移时迁到顶层 `context_window` | 清晰分层 |
| `engines[]` | `Vec<EngineTarget>` | local 必填 | 有序引擎链，index 0 为主用，余按序回退。取代 `prefer_drivers[0]` 单点选取 | **G2** |
| `EngineTarget.id` | `RuntimeBackendId`（字符串形） | 是 | 标识一个 backend，经 `RuntimeBackendId::from_str` 解析 | G2 |
| `EngineTarget.format` | `ArtifactFormat` | 是 | 该引擎消费的产物格式；与 `variant.format` 精确连接，是兼容性 join key | G2 / F2 类 |
| `EngineTarget.requires_devices` | `Vec<String>?` | 否 | 静态 satisfiability：所需设备（如 `["cuda"]`），缺失则回退时跳过 | G2 |
| **`VariantDocument`**（产物 + 加载） | 结构体 | local 必填 | **变体只载产物与加载参数**。不变式：variant **禁止**出现 `$inference_config`（见校验行），inference 归 preset | **F4/F5**（消除双载） |
| `VariantDocument.format` | `ArtifactFormat`（`gguf\|safetensors\|onnx\|ggml`，`#[non_exhaustive]`） | local 必填 | 该变体产物的磁盘格式，使 loader 无需 probe 即可匹配引擎 | G2 / F2 |
| `VariantDocument.id` | `String` | 是 | **pack 内唯一**（validator 导入期拒绝重复，取代 [resolve.rs:147](../../../crates/slab-model-pack/src/resolve.rs#L147) last-wins） | **F1** |
| `VariantDocument.$load_config` | `ConfigRef?` | 否 | **加载参数单一归属**（num_workers/chat_template/gbnf/flash_attn/device；**不含** context_length——它是全局量，见 `context_window` 行）。回退换变体时随之切换 | **F4/F5**（load 唯一源） |
| **`PresetDocument`**（用例 + 采样） | 结构体 | local 必填 | **预设只载用例绑定与采样参数**。不变式：preset **禁止**出现 `$load_config`（见校验行），load 归 variant | **F4/F5**（消除双载） |
| `PresetDocument.variant_id` | `String` | 是 | 该 preset 优选的变体（→ format → 主引擎）。**`variant_id` 唯一归属 preset**，manifest 入口级不再声明 | **F5**（消灭三处来源） |
| `PresetDocument.$inference_config` | `ConfigRef?` | 否 | **采样参数单一归属**（= 生成配置）。回退换引擎/变体时不变（sampler 与格式无关） | **F4/F5**（inference 唯一源） |
| `ConfigEntryRef.$ref` | `ConfigRef` | 是 | **统一**的"指向子文档"指针词汇（原 `$config` 降级为 serde alias） | **F4** |
| `BackendConfigDocument.scope` | `"load"\|"inference"` | 是 | load/inference 判别式（不变，对齐 F6：`kind` 仅类型 tag，`scope` 才驱动行为） | F6 |
| `BackendConfigDocument.id` | `Option<String>`（可选/元数据） | 否 | 不再作 lookup key（查询走 `ConfigRef.path`），消除死重量 | **F7** |
| `BackendConfigDocument.payload` | `Value`（JSON 对象） | 是 | 自由形态；导入期校验"必须是 JSON 对象 + asset-ref 形态合法"，前移自运行期 | **F2** |
| `ModelPackManifest.status`（顶层） | **删除** | — | 运行期状态不得静态落盘；改由 `RuntimeModelStatus` 表达 | **F8** |
| ~~`manifest.generation_config`~~ | **不引入** | — | generation_config **即** inference payload；HF 字段转换在代码（`gen:model-packs`）完成，未映射字段 `warn!`。无独立声明块 | G1 |
| `PackSourceCandidate.endpoint` | `String?`（URL，**新增**） | 否 | 每源 base endpoint 覆盖（镜像用），喂给 `HubClient::with_*_endpoint` | G3 |
| `PackSourceCandidate.credentials.secret_ref` | `String?`（`pmid://...`，**新增**） | 否 | 每源下载凭证句柄，按引用解析 | G3 / P7 |
| `cloud.provider_id` | `String` | cloud 必填 | 引用 `chat.providers` 注册项；pack 命名、不携带密钥 | G3 / P7 |
| `cloud.preferred_api_base` | `String?` | 否 | 作者推荐 base，仅在注册项无 `api_base` 时生效 | G3 |
| `cloud.credentials.secret_ref` | `String?` | 否 | 云端密钥句柄（缺省走 provider 注册项的 auth） | G3 / P7 |

> **导入期互斥校验（消灭双载）**：`validate_manifest_references`（[pack.rs:197](../../../crates/slab-model-pack/src/pack.rs#L197)）须断言——① 每个 `VariantDocument` **不得**含 `$inference_config`；② 每个 `PresetDocument` **不得**含 `$load_config`；违反即 `ModelPackError::OverlappingConfigOwnership`。这就结构上根除了 [Q4_K_M.json:5](../../../models/llama/Qwen3.5-9B/variants/Q4_K_M.json#L5) 与 [default.json:6-7](../../../models/llama/Qwen3.5-9B/presets/default.json#L6-L7) 那种 variant/preset 双载的 `preset ?? variant`（[resolve.rs:338-349](../../../crates/slab-model-pack/src/resolve.rs#L338-L349)）歧义。
>
> **三轴正交自检**：`engine`（backend 选谁）× `format`（产物什么格式）× `scope`（load 归 variant / inference 归 preset）各管一件事；`deployment` 与 `engines` 互斥（云端禁引擎）；`$ref`（指任意子文档）与 `$load_config`/`$inference_config`（指带 scope 的 backend_config）语义正交不撞名；variant 与 preset 文档内容正交、无交集。**无任何两字段共控同一决策。**

### 3.4 扩展性：纳入新架构 / 新模态（零 Schema 改动）

`ModelFamily`/`Capability`/`RuntimeBackendId` 均已 `#[non_exhaustive]`（已核实）。本规范仅需对 `slab-model-pack` 侧三个**未**标注的枚举补齐 `#[non_exhaustive]`：`PackDocumentKind`、`BackendConfigScope`、`PackSource`（均为一次性、加性、非破坏改动）。此后纳入 Mamba / RWKV / 新模态**无需触碰 Schema**：

```jsonc
// Mamba2（SSM 架构）—— 仅新增枚举字符串值，零 Schema 改动
{
  "schema_version": 3,
  "deployment": "local",
  "id": "Mamba2-2.7B",
  "family": "mamba",                                   // 新 ModelFamily 变体（加性 PR）
  "capabilities": ["text_generation", "chat_generation"],
  "engines": [{ "id": "mamba_ssm", "format": "safetensors" }],  // 新 RuntimeBackendId（加性 PR）
  "sources": [{ "kind": "hugging_face", "repo_id": "state-spaces/mamba2-2.7b",
                "files": [{ "id": "model", "path": "model.safetensors" }] }],
  "variants": [{ "id": "fp16", "label": "FP16", "format": "safetensors", "$ref": "ref://variants/fp16.json" }],
  "presets":  [{ "id": "default", "label": "Default", "variant_id": "fp16", "$ref": "ref://presets/default.json" }],
  "default_preset": "default"
}
```

引擎专属新参数 rides inside `payload`（开放 JSON 对象），新模态无需改 Schema。validator 仅强制：每个变体声明 `format`、每个 `format` 能在 `engines[]` 找到兼容引擎——该校验键击开放枚举，加 family/engine 无需改 validator。

---

## 4. 引擎切换与回滚机制（Engine Switch & Rollback）

### 4.1 当前缺口（经核实）

引擎今天**只是一个值、无回退**：`resolve_runtime_backend`（[runtime_bridge.rs:227-235](../../../crates/slab-model-pack/src/runtime_bridge.rs#L227-L235)）取 `preferred_runtime_backends_from_hints(hints).next()`（`:214-225` 丢弃 `prefer_drivers` 第一项之后），冻结进 `ModelPackRuntimeBridge.backend`（`:31`），由 gRPC 网关一次性消费（[runtime_gateway.rs:194-202](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L194-L202)）。`slab-app-core` 的 supervisor（[supervisor.rs](../../../crates/slab-app-core/src/infra/runtime/supervisor.rs)）是**进程**监督者（崩溃子进程带退避重启，`:727-851`），**没有"该模型在 backend X 加载失败→换 backend Y"的概念**——`load_model` 的 gRPC 失败被映射成 `BackendNotReady`/`RuntimeMemoryPressure`/`Internal` 直传 HTTP，无跨引擎重试。

### 4.2 数据流与契约

```
preset.variant_id → variant.format         (preset 选定优选变体 → 产物格式)
        │  + manifest.engines[]            (有序引擎链，首项主用)
        ▼
ModelPack::resolve()                       resolve.rs:67
   → 为每个 preset 构建 engine_chain：
       · 主引擎 = preset 优选变体 format 对应的 engines[0]
       · 回退项 = engines 中其余项，凡"存在同 format 变体"者按序补入
       · → GGML(gguf) 失败可回退 Candle(safetensors)：因 gguf 变体与 safetensors 变体俱在
   → ResolvedPreset 持 inference_config（采样，回退不变）+ engine_chain + effective_footprint
   → ResolvedVariant 持 load_config（加载，随回退换变体而切换）+ format
        ▼
compile_runtime_bridge()                   runtime_bridge.rs:52-90
   → 三谓词 satisfiability 裁剪链（§4.3）
   → 兼容性 guard：variant.format 必须 ∈ 候选引擎 format 集，否则导入期 NoCompatibleEngineForVariant
   → ModelPackRuntimeBridge { primary_engine, fallback_engines[], engine_load_specs[] }
        ▼
RuntimeGateway::load_model                 runtime_gateway.rs:194-202
   → 按 engine_load_specs 顺序派发；遇"引擎致命"错误分类（§4.4）弹栈下一个引擎重试
   → 耗尽则抛 AppCoreError::RuntimeEngineExhausted（携带 attempts 信封）
```

### 4.3 三谓词 satisfiability（静态 + 动态一致）

每个 `EngineTarget` 可用 iff 三者皆成立；静态（resolve 期）与动态（load 期）判定须一致，否则移出链：

| 谓词 | 判据 | 现状锚点 |
|------|------|---------|
| **(a) 设备可用** | `requires_devices`（如 `["cuda"]`）满足于 supervisor 上报的设备注册表；CPU 引擎为 `[]` | supervisor 仅当 `backend_endpoint(backend).is_some()` 才知 backend 存在（[supervisor.rs:127](../../../crates/slab-app-core/src/infra/runtime/supervisor.rs#L127)） |
| **(b) 资源在预算内** | `effective_footprint.vram_mb` ≤ 空闲 VRAM、`ram_mb` ≤ 空闲 RAM；OOM 倾向者降权、仅留末位兜底 | `ResourceFootprint`（[manifest.rs:360-366](../../../crates/slab-model-pack/src/manifest.rs#L360-L366)）、`DynamicFootprint`（`:368-374`） |
| **(c) backend 已编译进构建** | 引擎 id ∈ 当前构建实际编译进的 backend 集 | `RuntimeBackendId::ALL` 硬编码为 3 个 GGML backend（[backend.rs:70-72](../../../crates/slab-types/src/backend.rs#L70-L72)，注释 :69-71 说明 Candle/ONNX 在桌面发行版未编译）。Phase 0 暴露 `COMPILE_AVAILABLE` 常量使该判据可查询 |

> **Candle 兜底前置条件**：回退要真正生效，Candle 须编译进发行版并注册 endpoint——supervisor 已特判 Candle endpoint（[supervisor.rs:278-287](../../../crates/slab-app-core/src/infra/runtime/supervisor.rs#L278-L287)），管线已就绪。若某构建不含 Candle，链优雅降级为 GGML-only，回退为 no-op（经 §4.3 (c) 可观测）。

> **context_length 运行期协商**（与 §3 `context_window` 呼应）：谓词 (b) 判 VRAM 预算时，先据 `context_window` 上限与空闲 VRAM 反推"本次实际可开多少 KV cache"——生效 `context_length = min(用户请求, context_window, vram 可纳)`。若用户请求 > 显存可纳，运行期**自动降档**而非直接 OOM 失败（OOM 才触发 §4.4 的跨引擎回退）。`context_window` 是静态上限、`context_length` 是动态生效值，二者不混用。

### 4.4 回退触发分类（仅"引擎致命"才跨引擎重试）

| 失败分类 | 来源 | 跨引擎重试？ |
|---------|------|------------|
| OOM / 内存压力 | [runtime_gateway.rs:264](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs#L264) `is_memory_pressure_error` → `RuntimeMemoryPressure` | **是**（GGML OOM → 试 Candle CPU） |
| backend 不可用 / 未注册 | `:234` → `BackendNotReady`；[error.rs:225](../../../bin/slab-server/src/error.rs#L225) `DriverNotRegistered` | **是**（缺编译 → 下一引擎） |
| load-spec 不支持 / 配置畸形 | [error.rs:222](../../../bin/slab-server/src/error.rs#L222) `UnsupportedOperation` | **否**（配置 bug，重试无益，立即上报） |
| 传输瞬时错误（channel 中途死亡） | tonic transport | **否**（属**进程**监督者职责，[supervisor.rs:791-848](../../../crates/slab-app-core/src/infra/runtime/supervisor.rs#L791-L848) 重启子进程；网关不得把传输抖动伪装成换引擎） |
| 会话忙 | `:245` → `Conflict` | **否**（用户侧重试） |

> **职责边界**：**supervisor** 拥有*进程*存活（崩溃子进程带退避重启）；**gateway** 拥有*引擎*存活（同模型换 backend 重试）。两者从不重叠（load-bearing）。

### 4.5 兼容性 guard（根治 F2 类懒校验）

`compile_runtime_bridge` 在 satisfiability 裁剪链后运行 guard：`variant.format` 必须等于候选 `EngineTarget.format`，否则该引擎移出**此变体**的链；若链空，**导入期**返回 `ModelPackError::NoCompatibleEngineForVariant { variant_id, format }`。于是"GGUF-only 变体被强塞给 Candle"在**导入期即拒绝**，而非加载期崩溃——结构上根治 F2 的懒校验模式（[code-audits-2026-06-17.md:147-151](../audits/code-audits-2026-06-17.md#L147-L151)）。

### 4.6 桥接结构演进

`ModelPackRuntimeBridge`（[runtime_bridge.rs:29-36](../../../crates/slab-model-pack/src/runtime_bridge.rs#L29-L36)）增字段：

```rust
pub struct ModelPackRuntimeBridge {
    pub primary_engine: RuntimeBackendId,               // 原 backend
    pub fallback_engines: Vec<RuntimeBackendId>,        // 新：有序回退
    pub engine_load_specs: Vec<(RuntimeBackendId, RuntimeBackendLoadSpec)>, // 新：每引擎一份 load spec
    pub capability: Capability,
    pub model_spec: ModelSpec,
    pub load_defaults: ModelPackLoadDefaults,
    pub inference_defaults: JsonOptions,
}
```

`runtime_load_command`（`:94-100`）→ `runtime_load_commands() -> Vec<RuntimeModelLoadCommand>`；`runtime_load_spec`（`:102-211`）七臂 match（`:130-211`）重构为 `build_load_spec(backend, defaults, source)` 按引擎参数化，**七臂复用不改**。

**配置归属重构（呼应 §3 严格分离）**：
- `ResolvedVariant`（[resolve.rs:48-55](../../../crates/slab-model-pack/src/resolve.rs#L48-L55)）：增 `format: ArtifactFormat`（使 §4.5 guard 成立）；其 `load_config`（原有）**成为 load 的唯一来源**——删去 `ResolvedVariant.inference_config`（今 [resolve.rs:53-54](../../../crates/slab-model-pack/src/resolve.rs#L53-L54)）。
- `ResolvedPreset`（[resolve.rs:57-64](../../../crates/slab-model-pack/src/resolve.rs#L57-L64)）：增 `engine_chain: Vec<EngineTarget>` 与 `effective_footprint: ResourceFootprint`；其 `effective_inference_config`（采样）**成为 inference 的唯一来源**——删去 `effective_load_config`（今 [resolve.rs:62](../../../crates/slab-model-pack/src/resolve.rs#L62)），load 改读 `preset.variant.load_config`。
- 于是 `resolve_effective_backend_config`（[resolve.rs:338-349](../../../crates/slab-model-pack/src/resolve.rs#L338-L349) 的 `preset ?? variant` 双源）整体删除——每个 scope 各只有一个归属，无覆盖语义。

### 4.7 回退状态机 + 可观测（对齐 F-Stack-3）

```
Selected ─dispatch(load_cmd[0])─▶ Loading ─ok─▶ Ready
                                   │
                                   └─引擎致命─▶ LoadFailed ─fallback 非空?─是─▶ Fallback ─▶ Loading(下一)
                                                              │
                                                              └─否─▶ Exhausted ─▶ Error(503 RUNTIME_ENGINE_EXHAUSTED)
```

状态驻于**网关**（每次加载瞬态，不持久化——引擎可用性是动态的：GPU 拔出、VRAM 被他模型释放）。持久态仅存"上次成功引擎" `StoredModelConfig.selected_engine`（冷启动优化，非正确性杠杆）。

**跨 HTTP 边界保留机器可读 code**（根治 F-Stack-3，[code-audits-2026-06-17.md:125-129](../audits/code-audits-2026-06-17.md#L125-L129)）：新增 `ServerError::RuntimeEngineExhausted { model_id, attempts }`（[error.rs:212-271](../../../bin/slab-server/src/error.rs#L212-L271) 一带）与 `AppCoreError::RuntimeEngineExhausted`，HTTP 体携带回退信封：

```jsonc
{
  "code": "RUNTIME_ENGINE_EXHAUSTED",
  "message": "all configured engines failed to load model Qwen3.5-9B",
  "data": {
    "rollback": {
      "model_id": "Qwen3.5-9B",
      "attempts": [
        { "engine": "ggml.llama",   "outcome": "memory_pressure", "detail": "…" },
        { "engine": "candle.llama", "outcome": "backend_missing", "detail": "…" }
      ]
    }
  }
}
```

`outcome` 与 §4.4 重试分类 1:1 映射。每次回退都进信封——**永不静默重试**。

> **与 F-Stack-1 的边界**：引擎回退**不消费**设置热重载路径。`affects_agent_runtime`（[settings.rs:46-48](../../../crates/slab-app-core/src/domain/services/settings.rs#L46-L48)）只对 `agent.*` 触发 reload，本设计不拓宽它——引擎选择经既有 `PUT /v1/models/{id}/config-selection`（写 `model_config_state.selected_engine`）+ 显式 `POST /v1/models/load` 两步契约（F-Stack-4 的"两步隐式" critiques 针对*顺序*，非 reload 信号）。回退成功后把赢家写回 `selected_engine`，下次冷加载跳过已知坏引擎。

---

## 5. 多源下载路由（Multi-Provider Routing）

### 5.1 `source.kind` ↔ `HubProvider` 映射

`PackSource::remote_repository()`（[manifest.rs:187-205](../../../crates/slab-model-pack/src/manifest.rs#L187-L205)）是桥梁：

| `PackSource` 变体（[manifest.rs:118-146](../../../crates/slab-model-pack/src/manifest.rs#L118-L146)） | `source_kind` | `HubProvider`（[provider.rs:6-31](../../../crates/slab-hub/src/provider.rs#L6-L31)） | base_url |
|---|---|---|---|
| `HuggingFace{repo_id,revision,files,endpoint?}` | `hugging_face` | `HfHub` | `endpoints.hf_endpoint`（[provider.rs:44](../../../crates/slab-hub/src/provider.rs#L44)） |
| `ModelScope{repo_id,revision,files,endpoint?}` | `model_scope` | `ModelsCat` | `endpoints.models_cat_endpoint`（[provider.rs:46](../../../crates/slab-hub/src/provider.rs#L46)） |
| `HuggingfaceHubRust`（packs 暂未用） | — | `HuggingfaceHubRust` | `DEFAULT_HF_ENDPOINT` |
| `LocalPath`/`LocalFiles`/`Cloud` | — | 无（`remote_repository()` 返 `None`，[manifest.rs:203](../../../crates/slab-model-pack/src/manifest.rs#L203)） | — |

映射是闭集且 1:1。

### 5.2 镜像表达：`HuggingFace{endpoint}` 而非新变体（决策）

| 维度 | 新增 `HuggingFaceMirror{endpoint}` 变体 | **`HuggingFace{endpoint}` 字段（采纳）** |
|------|----------------------------------------|----------------------------------------|
| wire 形态 | 新 `kind` | 同 `kind:"hugging_face"` + 可选 `endpoint` |
| `remote_repository()` | 需新 arm + 新 `hub_provider` 串 | 不变，仍 `hf_hub` |
| `HubProvider` 闭集 | 需第 4 变体，污染闭集 | 不变，`HfHub` 已吃自定义 endpoint |
| slab-hub probe/cache | 新代码路径、新 feature flag | 复用 `run_with_provider_fallback` 原样 |
| 成本 | 高 churn 无语义增益 | 一个可选字段 + 一次 `with_hf_endpoint` 调用 |

**决策**：给 `PackSource::HuggingFace` 与 `::ModelScope` 加 `endpoint: Option<String>`（[manifest.rs:128-141](../../../crates/slab-model-pack/src/manifest.rs#L128-L141)）。hf-mirror.com 原生说 HF 协议，换 base 即得镜像。真正的第三协议（非 HF 兼容）才需新变体——不在本场景。

### 5.3 slab-hub 端到端路由（伪代码）

```
# 1. pack 解析对源排序（slab-model-pack 拥有）
ordered = ordered_source_candidates(pack.sources)
#   resolve.rs:265-274：按 priority.unwrap_or(i32::MAX) 升序，并列按声明序（稳定）

for candidate in ordered:                              # HF → ModelScope → 镜像
    repo = candidate.source.remote_repository()        # manifest.rs:187-205
    if repo.is_none(): continue                        # local/cloud 无 hub 仓库

    provider = match repo.hub_provider {               # §5.1 映射
        "hf_hub" => HfHub, "models_cat" => ModelsCat, "huggingface_hub_rust" => HuggingfaceHubRust,
    }

    # 2. 每源建 HubClient，套上该源的 endpoint 覆盖
    client = HubClient::default().with_cache_dir(cache_dir)
        .with_provider_preference(Provider(provider))  # client.rs:44-48,188-198
    if let Some(ep) = candidate.source.endpoint {      # 新（§5.2）
        match provider { HfHub|HuggingfaceHubRust => client.with_hf_endpoint(ep),       # client.rs:63-67
                         ModelsCat                => client.with_models_cat_endpoint(ep) } # client.rs:69-73
    }
    # 凭证：secret_ref → PMID 解析 → 注入 token（按引用，非明文进 pack）

    # 3. slab-hub probe + cache + fallback（不变）
    match client.run_with_provider_fallback(|p| download(p, repo)).await {
        Ok(path) => return Ok(path),                   # client.rs:157-162 缓存 provider
        Err(e) if e.kind == NetworkUnavailable => continue,  # client.rs:164-171
        Err(e) => return Err(e),                       # client.rs:172 硬错，不回退
    }
return Err(AllSourcesExhausted)
```

两层职责清晰：`slab-model-pack` 只**排序**候选（[resolve.rs:265-274](../../../crates/slab-model-pack/src/resolve.rs#L265-L274)）；`slab-hub` 拥有每候选内的 probe/cache/fallback（[client.rs:132-183](../../../crates/slab-hub/src/client.rs#L132-L183)）。P6 满足：元数据同构，仅 repo URL + 策略异构。

### 5.4 凭证模型：按引用、永不按值

经核实，PMID 脱敏层正确但由**硬编码白名单**驱动（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §1.3/§3.2 PMID-F9）：`secret()`（[pmid_service.rs:971-975](../../../crates/slab-config/src/pmid_service.rs#L971-L975)）覆盖 `server.admin.token`/`providers.registry`/`agent.tools.websearch.providers`；`redact_setting_value`（`:977`）对 leaf 与对象/数组内 `api_key` 字段脱敏，写回经 `restore_secret_placeholders`（`:1016`）保留原值。

**v3 规则**：pack **不得**含字面 `api_key`/含凭证的 `api_base`/bearer token，只可引用：
- **云端**：`cloud.provider_id`（首选）→ 操作员 `chat.providers[id]` 的 `api_key`/`api_key_env`（已由 `providers.registry` 脱敏，[settings/config.rs:11-28](../../../crates/slab-config/src/settings/config.rs#L11-L28)）。
- **下载**：`credentials.secret_ref: pmid://...` 句柄 → 扩展 `secret()` 白名单纳入 `download.<handle>` 子树，复用既有脱敏/还原管线。schema 生成器把这些 leaf 标 `writeOnly: true`，使**flag 驱动**路径可在后续替换硬编码白名单（PMID-F9 修复）。

pack 文件即便外泄也零密钥——这正是"可批量生成、人类可读"的安全前提。

### 5.5 遗留 `hub_provider` 重映射的日落

`apply_legacy_remote_source_kind`（[manifest.rs:304-323](../../../crates/slab-model-pack/src/manifest.rs#L304-L323)）今天会按 `hub_provider` 字段静默把 `ModelScope` 改写成 `HuggingFace`（反之亦然）。v3 的 priority + 每源 `endpoint` 下，该重映射成 footgun——可能静默翻转作者精心设的 endpoint。`hub_provider` **就是**策略，应由 `kind` 派生而非独立声明。日落路径：
- **v3.0**：`hub_provider` 降级为 deprecated alias；与 `kind` 冲突时 `warn!` 且**尊 `kind`**（与今日 `:308-322` 行为相反）。
- **v3.1**：`hub_provider` 与 `kind` 冲突即报硬错；删除 `apply_legacy_remote_source_kind`；`Flat`/`Legacy` wire 折叠为 `SourceOnly + priority`。
- **v3.2**：`hub_provider` 字段从 wire schema 移除；删 `LegacyRemoteSourceKind`/`LegacyPackSourceCandidateWire`。

---

## 6. 实施路线图

> 每阶段：{改动文件、关闭的审计发现、校验命令（[AGENTS.md](../../../AGENTS.md):56-95）、退出标准}。迁移是**加性、append-only**（[AGENTS.md:32](../../../AGENTS.md#L32)）。现有 6 个发布 pack 的迁移**在本计划内**，不延期。

### Phase 0 — Schema 基元（引擎轴 + 扩展性补齐）

- **文件**：
  - [backend.rs](../../../crates/slab-types/src/backend.rs)：暴露 `COMPILE_AVAILABLE`（或扩 `ALL` 语义），使 §4.3 (c) 可查询（今日真相仅在注释 :69-71）。
  - [runtime.rs](../../../crates/slab-types/src/runtime.rs)：新增 `ArtifactFormat { Gguf, Safetensors, Onnx, Ggml }`（紧邻 `ModelSourceKind` :65-71）。
  - [manifest.rs](../../../crates/slab-model-pack/src/manifest.rs)：`ModelPackManifest` 加 `schema_version`/`deployment`/`engines`/`cloud`；`VariantDocument` 加 `format`（并据 §3 把 `$load_config` 收敛为唯一 load 源、移除 `$inference_config`）；`PresetDocument` 的 `$inference_config` 收敛为唯一 inference 源、移除 `$load_config`；`EngineTarget` 结构；给 `PackDocumentKind`/`BackendConfigScope`/`PackSource` 补 `#[non_exhaustive]`。
  - [error.rs](../../../crates/slab-model-pack/src/error.rs)：加 `NoCompatibleEngineForVariant`/`DuplicateEntryId`/`IncompatibleVariantFormat`。
- **关闭**：无（基础设施）。
- **校验**：`bun run gen:schemas` → `bun run check:rust`。
- **退出标准**：新字段进 `docs/public/manifests/v1/slab-manifest.schema.json`；`engines` 默认空，现有 pack 仍可加载。

### Phase 1 — 解析器 + 引擎轴 + 配置归属重构 + 导入期校验（根治 F1/F2/F4/F5）

- **文件**：
  - [resolve.rs](../../../crates/slab-model-pack/src/resolve.rs)：`ResolvedVariant` 增 `format`、`load_config` 成为 load 唯一源（删 `inference_config`，:53-54）；`ResolvedPreset` 增 `engine_chain`/`effective_footprint`、`effective_inference_config` 成为 inference 唯一源（删 `effective_load_config`，:62）；`resolve_presets`（:161-208）建链（主引擎 = preset 变体 format 对应 engines[0]，回退项 = 其余有同 format 变体的 engines），`engines` 缺省时从 `prefer_drivers` 合成（与 [runtime_bridge.rs:227-235](../../../crates/slab-model-pack/src/runtime_bridge.rs#L227-L235) 逐位兼容）；**删除** `resolve_effective_backend_config` 的 `preset ?? variant` 双源（:338-349）。
  - [pack.rs](../../../crates/slab-model-pack/src/pack.rs)：`validate_manifest_references`（:197-258）新增：① id 唯一性（`DuplicateEntryId`，根治 F1）；② payload 形态 + asset-ref 校验（裸字符串 `chat_template` 在 `from_bytes` 即拒，根治 F2，前移自 [runtime_bridge.rs:362-373](../../../crates/slab-model-pack/src/runtime_bridge.rs#L362-L373)）；③ **配置归属互斥**——variant 不得含 `$inference_config`、preset 不得含 `$load_config`，违者 `OverlappingConfigOwnership`（根治 F4/F5 双载）；`$config`→`$ref` 重命名（serde alias 保兼容，F4）；manifest 入口级 `PresetEntryRef.variant_id` 删除（F5）。
- **关闭**：**F1、F2、F4、F5**。
- **校验**：`bun run test:rust:cargo`，新增负向测试：(i) 重复 `Q4_K_M` 拒绝；(ii) 裸字符串 `chat_template` 在 `from_bytes` 拒绝；(iii) GGUF-only 变体不能选 `candle.llama`；(iv) variant 带 `$inference_config` 拒绝；(v) preset 带 `$load_config` 拒绝。
- **退出标准**：既有 `runtime_bridge.rs:553-885` 与 `resolve.rs:351-803` 测试在迁移子文档后全过（合成回退保兼容）；五个负向测试通过。

### Phase 2 — 桥接引擎链 + 网关回退（根治 F-Stack-3）

- **文件**：
  - [runtime_bridge.rs](../../../crates/slab-model-pack/src/runtime_bridge.rs)：`ModelPackRuntimeBridge` 增 `primary_engine`/`fallback_engines`/`engine_load_specs`（§4.6）；`runtime_load_spec`（:102-211）重构为 `build_load_spec(backend, …)`；`runtime_load_command`→`runtime_load_commands()`。
  - [infra/model_packs/mod.rs](../../../crates/slab-app-core/src/infra/model_packs/mod.rs)：`read_model_pack_runtime_bridge`（:103-115 一带）增链暴露。
  - [runtime_gateway.rs](../../../crates/slab-app-core/src/infra/rpc/runtime_gateway.rs)：`load_model`（:194-202）实现 §4.7 状态机——派发链首，遇 §4.4 引擎致命分类弹栈重试，耗尽抛 `AppCoreError::RuntimeEngineExhausted`。
  - [supervisor.rs](../../../crates/slab-app-core/src/infra/runtime/supervisor.rs)：**不改结构**，仅在 :54-60 一带加 doc 注释澄清"引擎回退是网关职责、进程重启是监督者职责"。
  - [error.rs](../../../bin/slab-server/src/error.rs)（:212-271）+ [app-core error.rs](../../../crates/slab-app-core/src/error.rs)：加 `RuntimeEngineExhausted` 变体与 `data.rollback` 信封，`AppCoreError`→`ServerError` 增映射臂。
- **关闭**：**F-Stack-3**（跨边界保结构）。
- **校验**：`bun run test:rust:cargo`，新增伪网关集成测试：(a) `ggml.llama` 返 `memory_pressure`、`candle.llama` 成功→断言链走通、第二引擎加载；(b) 返 `unsupported`→断言不重试、typed error 进 HTTP 信封。
- **退出标准**：回退在响应体可观测（非仅日志）；`supervisor.rs:1286-1657` 监督测试不改通过。

### Phase 3 — slab-hub 多源对齐

- **文件**（云端/多源设计主导）：
  - [manifest.rs](../../../crates/slab-model-pack/src/manifest.rs)：`PackSource::HuggingFace`/`::ModelScope` 加 `endpoint: Option<String>`；`PackSourceCandidate` 加 `credentials: Option<SecretRef>`。
  - 下载调用点：候选 `endpoint` 喂入 `with_hf_endpoint`/`with_models_cat_endpoint`（[client.rs:63-73](../../../crates/slab-hub/src/client.rs#L63-L73) 调用侧）。
  - [pmid_service.rs:971](../../../crates/slab-config/src/pmid_service.rs#L971)：`secret()` 白名单纳入 `download.<handle>`；leaf 标 `writeOnly`。
  - [compile_runtime_bridge](../../../crates/slab-model-pack/src/runtime_bridge.rs#L52-L90)：`deployment=="cloud"` 早返 `UnsupportedRuntimeBridgeSource { source_kind: "cloud" }`（error 已存 [error.rs:110-111](../../../crates/slab-model-pack/src/error.rs#L110-L111)），**先于**引擎逻辑——云端永不建引擎链。
- **关闭**：G3（多源原生 + 凭证按引用）。
- **校验**：`bun run check:rust`。
- **退出标准**：云端 pack 永不进引擎解析器；本地 pack 永不进云端 provider 路径；镜像源可用自定义 endpoint 下载。

### Phase 4 — DB 迁移（存储 selected_engine）

- **文件**：
  - [migrations/20260618000000_model_config_selected_engine.sql](../../../crates/slab-app-core/migrations/)（新，append-only）：`ALTER TABLE model_config_state ADD COLUMN selected_engine TEXT;`（nullable，回填 NULL）。纯加性，无数据转换。
  - [repository/model_config_state.rs](../../../crates/slab-app-core/src/infra/db/repository/model_config_state.rs) + [entities/model_config_state.rs](../../../crates/slab-app-core/src/infra/db/entities/)：读写 `selected_engine`。
  - [schemas/models.rs](../../../crates/slab-app-core/src/api/schemas/models.rs)：`UpdateModelConfigSelectionRequest`（:213，审计 D5）与 `UnifiedModel` 响应携带 `selected_engine`。**该列经 Rust re-serialize 整列写，禁用 `json_set`**（审计 T1/D4，[code-audits-2026-06-17.md:94-101](../audits/code-audits-2026-06-17.md#L94-L101)）。
- **关闭**：D5 部分（selected 状态跨表问题的*引擎选择*归口 `model_config_state`）。
- **校验**：`bun run gen:api` → `bun run test:rust:cargo`。
- **退出标准**：[v1.d.ts](../../../packages/api/src/v1.d.ts) 暴露 `selected_engine`；round-trip 测试读写一致。

### Phase 5 — 迁移 6 个发布 pack + 修数据 bug（根治 F1/F2/F8）

- **文件**：`models/**` 六个 authored pack。迁移引擎轴时一并修：
  - **F1 重复 `Q4_K_M`**：删 [Qwen3.5-9B/manifest.json:76-80](../../../models/llama/Qwen3.5-9B/manifest.json#L76-L80) 与 Qwen2.5-0.5B 对应重复块。Phase 1 的 `DuplicateEntryId` 上线后，保留会被导入期拒绝。
  - **F2 裸字符串 `chat_template`**：改 [Qwen2.5-0.5B-Instruct/configs/load.json:8](../../../models/llama/Qwen2.5-0.5B-Instruct/configs/load.json#L8) 为对象 asset-ref 形（同 [Qwen3.5-9B/configs/load.json](../../../models/llama/Qwen3.5-9B/configs/load.json)），补 `assets/chatml.jinja`。
  - **F8 误用 `status:"ready"`**：删 [justinpinkney_miniSD/manifest.json:6](../../../models/diffusion/justinpinkney_miniSD/manifest.json#L6)；authored pack 应如 llama 推断 `NotDownloaded`。
  - **引擎轴 + 配置归属 + context_length 迁出**：每个 GGML 主用本地 pack 加 `engines`（主 `ggml.<family>` + 有 safetensors 变体则加 `candle.<family>` 兜底），每个 `VariantDocument` 加 `format`。**把现有 variant 里的 `$inference_config` 移到对应 preset、preset 里的 `$load_config` 移到对应 variant**——消除 [Q4_K_M.json:5](../../../models/llama/Qwen3.5-9B/variants/Q4_K_M.json#L5)（variant 载 inference）与 [default.json:6-7](../../../models/llama/Qwen3.5-9B/presets/default.json#L6-L7)（preset 载 load+inference）的双载。**把 [load.json](../../../models/llama/Qwen3.5-9B/configs/load.json) 里的 `context_length` 迁出到顶层 `context_window`**（如 [Qwen3.5-9B/configs/load.json:7](../../../models/llama/Qwen3.5-9B/configs/load.json#L7) 的 `context_length: 4096`→manifest `context_window`）。`deployment:"local"`。
- **关闭**：**F1、F2、F8**（F4/F5 的数据面在 Phase 1 校验器强制下顺带闭合）。
- **校验**：`bun run gen:model-packs` 重生 `.slab` → `bun run test:rust:cargo` 跑 Phase 1 校验器验重生 pack。
- **退出标准**：六个 pack 在 Phase 1 校验器下干净导入（含配置归属互斥、load payload 无 `context_length`）；`git diff` 见重复块与 `status` 已删、`load.json` 为对象形态且无 context_length、variant 无 inference / preset 无 load。

### Phase 6 — 批量生成 hook + generation_config / config.json 转换器 + 子文档 schema + 复审

- **文件**：
  - 生成器（`bun run gen:model-packs` 派发处，[infra/model_packs/mod.rs:469](../../../crates/slab-app-core/src/infra/model_packs/mod.rs#L469) 一带）：生成的 pack 也产出 `engines`/`format`/`deployment`；**新增两个 HF 转换器**——① `generation_config.json` → inference payload（采样参数，按 §3.2.1 映射表），② `config.json` `max_position_embeddings` → 顶层 `context_window`（训练 ctx 上限）；**未覆盖字段一律 `warn!`**（不静默丢弃，对齐审计 G4）。无独立 `generation_config` 声明块——inference payload 即生成配置。
  - [schema.rs](../../../crates/slab-model-pack/src/schema.rs)（:10-36 仅发 manifest，审计 F3）：扩展为对 `VariantDocument`/`PresetDocument`/`BackendConfigDocument`/`ComponentDocument`/`AdapterDocument` 也发 `$defs`，并扩展 :69-74 对照测试。否则子文档顶部 `$schema` 指针验证空。
  - 复审：基于 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) 出一份新审计文档，确认 F1/F2/F3/F4/F5/F8/F-Stack-3 关闭且引擎路径无新发现。
- **关闭**：**F3**（schema 覆盖子文档）。
- **校验**：`bun run gen:model-packs` → `bun run gen:schemas` → `bun run check:rust` → `bun run test:rust:cargo`。
- **退出标准**：生成 pack 携带引擎链与归属分离的 variant/preset、顶层 `context_window`（训练 ctx）；inference 由 HF generation_config 代码转换产生、`context_window` 由 config.json 提取（未映射字段 warn）；发布 JSON Schema 覆盖所有子文档 kind；复审显示 F1–F5/F8/F-Stack-3 关闭。

### 6.1 验证策略（对齐审计 §6.4）

- **Phase 1**：扩展 `slab-model-pack` 测试，断言重复 id 报 `DuplicateEntryId`、裸字符串 `chat_template` 在 `from_bytes` 即报错、GGUF-only 变体选 Candle 报 `NoCompatibleEngineForVariant`。
- **Phase 4**：迁移后补 round-trip 测试（坏 JSON 写入应失败而非砖读，对齐 T1/D4）。
- **Phase 2**：伪网关集成测试覆盖回退链与 typed error 信封。
- **整体**：每个 Phase 用最窄校验命令先验证（[AGENTS.md:18](../../../AGENTS.md#L18)），再扩到 workspace。

---

## 附录 A：审计发现 → 规范条款 闭环追溯

| 审计发现（[code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md)） | v3 机制 | 实现阶段 |
|---|---|---|
| **F1** variant.id 重复 last-wins（resolve.rs:147；已发布 pack 带病） | `validate_manifest_references` 导入期发 `DuplicateEntryId`；`variant.format` 取代 `file.id==variant.id` 隐式约定 | Phase 1 + Phase 5（删数据） |
| **F2** asset-ref / payload 懒校验（pack.rs:197-258；runtime_bridge.rs:362-373） | payload 形态 + asset-ref 校验前移到 `from_bytes`；`format`-based 兼容 guard | Phase 1 + Phase 5 |
| **F3** schema 只覆盖 manifest（schema.rs:10-36） | 扩展生成子文档 `$defs` | Phase 6 |
| **F4** `$config` vs `$load_config` 双词汇 | 入口统一 `$ref`（`$config` 降级 alias）；两词语义正交文档化 | Phase 1 |
| **F4/F5+** variant 与 preset **双载** `$load_config`/`$inference_config`（`preset ?? variant`，resolve.rs:338-349；实测 [Q4_K_M.json:5](../../../models/llama/Qwen3.5-9B/variants/Q4_K_M.json#L5)+[default.json:6-7](../../../models/llama/Qwen3.5-9B/presets/default.json#L6-L7)） | **严格职责分离**：`$load_config` 只在 variant、`$inference_config`(=generation_config) 只在 preset；导入期互斥校验 `OverlappingConfigOwnership`；删 `resolve_effective_backend_config` 双源 | Phase 1 + Phase 5（迁移数据） |
| **F5** variant_id 三处来源（resolve.rs:170-173） | `variant_id` 只活于 `PresetDocument`，删 manifest 入口级 | Phase 1 |
| **新增** generation_config 与 inference 同义 | 不引入独立声明块；inference payload 即生成配置，HF 字段在 `gen:model-packs` 代码转换、未映射 `warn!`（对齐 G4） | Phase 6 |
| **F6** `kind`+`scope` 双标记 | 文档明确 `scope` 是 load/inference 判别式、`kind` 仅类型 tag（不变，正交） | Phase 0（文档） |
| **F7** `BackendConfigDocument.id` 死重量（manifest.rs:523） | `id` 改 `Option`/元数据 | Phase 0 |
| **F8** `status` 误用为静态属性（justinpinkney_miniSD:6） | 删顶层 `status`，归 `RuntimeModelStatus` | Phase 0 + Phase 5（删数据） |
| **F9** default_preset 单预设隐式（resolve.rs:210-226） | `presets` 非空时 schema 强制 `default_preset` | Phase 1 |
| **F10** `manifest.version` 无消费者 | `schema_version` 接线 + 版本门控 | Phase 0 |
| **F11** PackSource 三 wire 格式 + legacy remap | `hub_provider` 日落（§5.5，v3.0→v3.2） | Phase 3 |
| **F-Stack-3** gRPC 错误跨边界损失结构 | `RuntimeEngineExhausted` + `data.rollback` 信封 | Phase 2 |
| **F-Stack-4** 选择→加载两步隐式 | 引擎回退复用既有两步契约；回退赢家写回 `selected_engine` | Phase 2 + Phase 4 |

## 附录 B：与既有契约的边界（对齐 [AGENTS.md](../../../AGENTS.md)）

- 推理链路边界不变：`bin/slab-app → bin/slab-server → slab-app-core runtime supervisor → GrpcGateway → bin/slab-runtime → slab-runtime-core`。本规范只改 `slab-model-pack`（Schema/解析/桥接）与 `slab-app-core`（编目/网关/迁移），不动 `slab-runtime-core`。
- 跨 crate 契约走 [slab-types](../../../crates/slab-types)/[slab-proto](../../../crates/slab-proto)（`ArtifactFormat` 入 slab-types）。
- API shape 变更走 `bun run gen:api` 刷 [v1.d.ts](../../../packages/api/src/v1.d.ts)（Phase 4）。
- SQLx 迁移 append-only（Phase 4）。
- `bun run gen:model-packs` / `gen:schemas` 是批量生成与 schema 发布的 canonical 入口。

---

*本文由首席架构师主导 + Agent Team 三路并行深度设计（本地多引擎/变体 Schema、云端 + 多源下载路由、引擎切换/回滚 + 实施路线图）整合而成，所有 High 级设计点经主架构师直接读源码落地核实（`ModelFamily`/`RuntimeBackendId` 的 `#[non_exhaustive]` 状态、`affects_agent_runtime` 实际行号、`RuntimeBackendId::ALL` 编译集、runtime_bridge 云端硬拒、slab-hub probe/cache/fallback 均已核实）。规范以 [code-audits-2026-06-17.md](../audits/code-audits-2026-06-17.md) §3.1 的 F1–F11 与 §2.3 跨边界缺陷为闭环目标。*
