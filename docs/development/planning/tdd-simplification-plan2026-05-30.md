# Slab Workspace — TDD 简化与优化实施计划

**Date:** 2026-05-30
**Based on:** 10 份审计报告合并分析（`docs/development/audits/project-audits-2026-05-30/00~06`）
**Commit:** ff54f8b53a8dc9786ddbf02eeea8dba0e2792f96
**Method:** TDD（测试驱动重构）— 先写失败测试，再重构，测试通过即完成

---

## 目录

1. [合并发现总览](#1-合并发现总览)
2. [TDD 重构原则](#2-tdd-重构原则)
3. [Phase 1 — Rust 后端核心服务拆分](#phase-1--rust-后端核心服务拆分)
4. [Phase 2 — Rust 运行时去重](#phase-2--rust-运行时去重)
5. [Phase 3 — TypeScript 前端 Hook 拆分](#phase-3--typescript-前端-hook-拆分)
6. [Phase 4 — 跨层一致性与基础设施](#phase-4--跨层一致性与基础设施)
7. [Phase 5 — 低优先级持续改进](#phase-5--低优先级持续改进)
8. [风险矩阵与验收标准](#风险矩阵与验收标准)
9. [审计来源索引](#审计来源索引)

---

## 1. 合并发现总览

经合并 10 份审计报告，去重后共 **32 项独立发现**，按影响面归类如下：

### 按领域分布

| 领域 | HIGH | MEDIUM | LOW | 来源审计 |
|------|------|--------|-----|---------|
| Rust 后端服务层 | 4 | 4 | 3 | 02, 03, 04 |
| Rust 运行时后端 | 2 | 2 | 1 | 06 |
| TypeScript 前端 | 3 | 3 | 2 | 02-frontend, 03-frontend |
| 架构分层 | 0 | 2 | 1 | 01, 05 |
| API 与接口 | 0 | 3 | 3 | 04, 04-api |
| **合计** | **9** | **14** | **10** | |

### 去重合并说明

以下发现跨多份审计重复出现，已合并为单一行动项：

| 重复主题 | 出现位置 | 合并至 |
|----------|---------|--------|
| Chat 服务嵌套过深 | 02-rust, 03-backend | P1-T1 |
| Media 服务 ~80% 重复 | 03-backend, 06-runtime | P2-T1 |
| use-audio.ts 过大 | 03-frontend, 02-frontend | P3-T1 |
| use-workspace-page.ts 过大 | 03-frontend, 02-frontend | P3-T2 |
| AudioWorkbench 94 props | 03-frontend | P3-T3 |
| TypeScript 类型漂移风险 | 04-api, 05-consistency | P4-T1 |
| 错误处理不一致 | 02-rust, 03-frontend, 04-api | P4-T2 |
| 嵌套三元/复杂条件 | 02-frontend, 03-frontend | P3-T4 |
| 测试目录不一致 | 05-consistency | P4-T3 |
| Runtime 后端代码重复 | 06-runtime | P2-T2, P2-T3 |

---

## 2. TDD 重构原则

每次重构遵循严格的 **Red → Green → Refactor** 循环：

```
┌──────────────────────────────────────────────────┐
│  1. RED    — 为当前行为编写表征测试（characterization test）│
│  2. GREEN  — 确认测试通过（当前实现无变化）              │
│  3. REFACTOR — 执行重构                              │
│  4. GREEN  — 确认测试仍通过（行为未改变）               │
│  5. CLEAN  — 删除冗余测试，保留有价值的回归测试          │
└──────────────────────────────────────────────────┘
```

### 约束条件

1. **功能不变** — 只改 *如何做*，不改 *做什么*
2. **每个 Task 独立可交付** — 不依赖后续 Task
3. **每个 Task 有明确验收测试** — 全部通过才能合并
4. **AGENTS.md 硬约束不可违反** — 参见 AGENTS.md Hard Constraints
5. **SQLx 迁移只追加** — 不修改已有迁移文件

### 全局验收标准

每个 Phase 完成后必须通过：

```bash
# Rust 侧
cargo check --workspace
cargo test --workspace

# TypeScript 侧
bun install
bun run lint
bun run test
bun run build:desktop

# API 类型同步
bun run gen:api
git diff --exit-code packages/api/src/v1.d.ts  # 无漂移
```

---

## Phase 1 — Rust 后端核心服务拆分

> **目标：** 降低 slab-app-core 核心服务复杂度
> **预估工期：** 2 周
> **审计来源：** 02-rust-backend-audit, 03-backend-audit, 01-architecture-analysis

---

### P1-T1: 拆分 Chat 服务嵌套逻辑

**来源审计：** 02-rust §1.1.1, 03-backend §Key Finding 2
**严重度：** HIGH
**当前问题：** `create_chat_completion_with_state` 函数 250+ 行，嵌套 4-5 层

#### Red — 表征测试

```rust
// 文件: crates/slab-app-core/src/domain/services/chat/__tests.rs (新建)
// 或集成测试: crates/slab-app-core/tests/chat_routing.rs

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：本地文本补全应返回 ChatCompletionOutput::Json
    #[tokio::test]
    async fn local_text_completion_returns_json() {
        // Arrange: 配置本地后端，非流式请求
        // Act: 调用 create_chat_completion_with_state
        // Assert: 返回 ChatCompletionOutput::Json 包含 choices
    }

    /// 测试：流式补全应返回 ChatCompletionOutput::Stream
    #[tokio::test]
    async fn stream_completion_returns_stream() {
        // Arrange: 配置 stream=true
        // Act: 调用 create_chat_completion_with_state
        // Assert: 返回 ChatCompletionOutput::Stream
    }

    /// 测试：云端路由正确分发到 cloud 模块
    #[tokio::test]
    async fn cloud_routing_dispatches_to_cloud_module() {
        // Arrange: 配置 route_to_cloud = true
        // Act: 调用 create_chat_completion_with_state
        // Assert: 请求经过 cloud::create_chat_completion
    }

    /// 测试：多轮对话 (n > 1) 生成正确数量 choices
    #[tokio::test]
    async fn multiple_choices_generates_n_results() {
        // Arrange: 设置 n = 3
        // Act: 调用 create_chat_completion_with_state
        // Assert: choices.len() == 3
    }
}
```

#### Refactor — 拆分方案

```rust
// 当前（250+ 行，4-5 层嵌套）:
// crates/slab-app-core/src/domain/services/chat/mod.rs:638-888

// 拆分为:

// 1. 路由决策 — 纯函数，无 IO
// crates/slab-app-core/src/domain/services/chat/routing.rs (新建)
pub enum ChatRoute {
    LocalStream { ... },
    LocalText { n: u32, ... },
    CloudStream { ... },
    CloudText { n: u32, ... },
}

pub fn resolve_chat_route(state: &ModelState, command: &ChatCompletionCommand) -> ChatRoute {
    // 卫语句替代嵌套 if-else
}

// 2. 请求执行 — 处理单次推理
// crates/slab-app-core/src/domain/services/chat/execution.rs (新建)
pub async fn execute_single_completion(
    route: &ChatRoute,
    command: ChatCompletionCommand,
) -> Result<GeneratedChatOutput, AppCoreError> {
    match route {
        ChatRoute::LocalStream { .. } | ChatRoute::CloudStream { .. } => { ... }
        ChatRoute::LocalText { .. } | ChatRoute::CloudText { .. } => { ... }
    }
}

// 3. 响应构建 — 纯转换逻辑
// crates/slab-app-core/src/domain/services/chat/response.rs (新建)
pub fn build_completion_response(
    choices: Vec<Choice>,
    model_id: &str,
) -> ChatCompletionOutput { ... }
```

#### Green — 验收标准

- [ ] `cargo test -p slab-app-core -- chat` 全部通过
- [ ] `create_chat_completion_with_state` 缩减至 < 80 行
- [ ] 无函数超过 50 行
- [ ] 嵌套层级 ≤ 2

---

### P1-T2: 抽取 Media 服务共享逻辑

**来源审计：** 03-backend §Key Finding 1 & 4
**严重度：** HIGH
**当前问题：** AudioService、ImageService、VideoService 共享 ~80% 代码

#### Red — 表征测试

```rust
// 文件: crates/slab-app-core/src/domain/services/__tests__/media_task_test.rs (新建)

#[cfg(test)]
mod tests {
    /// 测试：音频任务创建流程与现有行为一致
    #[tokio::test]
    async fn audio_task_creation_matches_current_behavior() {
        // 通过 AudioService 创建任务 → 验证 task_id, status, type
    }

    /// 测试：图片任务创建流程与现有行为一致
    #[tokio::test]
    async fn image_task_creation_matches_current_behavior() { ... }

    /// 测试：视频任务创建流程与现有行为一致
    #[tokio::test]
    async fn video_task_creation_matches_current_behavior() { ... }

    /// 测试：任务状态轮询行为一致
    #[tokio::test]
    async fn task_status_polling_behavior_preserved() { ... }

    /// 测试：任务取消行为一致
    #[tokio::test]
    async fn task_cancellation_behavior_preserved() { ... }
}
```

#### Refactor — 拆分方案

```rust
// 新建: crates/slab-app-core/src/domain/services/media_task.rs

/// 统一的媒体任务操作 — 替代 Audio/Image/Video 各自重复的实现
pub struct MediaTaskService {
    worker_state: WorkerState,
}

impl MediaTaskService {
    pub async fn create_task(
        &self,
        task_type: MediaTaskType,    // Audio | Image | Video
        backend_id: RuntimeBackendId,
        request: MediaTaskRequest,
    ) -> Result<TaskView, AppCoreError> { ... }

    pub async fn cancel_task(&self, task_id: &str) -> Result<(), AppCoreError> { ... }

    pub async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus, AppCoreError> { ... }

    pub async fn list_tasks(
        &self,
        filter: MediaTaskType,
    ) -> Result<Vec<TaskView>, AppCoreError> { ... }
}

pub enum MediaTaskType {
    Audio,
    Image,
    Video,
    Subtitle,
}
```

```rust
// 各服务简化为薄代理:
// crates/slab-app-core/src/domain/services/audio.rs
pub struct AudioService {
    media: MediaTaskService,
}

impl AudioService {
    pub async fn transcribe(&self, req: AudioRequest) -> Result<TaskView, AppCoreError> {
        self.media.create_task(
            MediaTaskType::Audio,
            self.backend_id(),
            req.into(),
        ).await
    }
}
```

#### Green — 验收标准

- [ ] AudioService/ImageService/VideoService 各自 < 60 行
- [ ] `cargo test -p slab-app-core` 全部通过
- [ ] 所有媒体任务端到端行为不变

---

### P1-T3: 简化 Plugin 验证逻辑

**来源审计：** 02-rust §1.1.3
**严重度：** HIGH（代码量）
**当前问题：** `validate_contributions` 180+ 行，6+ 重复校验循环

#### Red — 表征测试

```rust
// 文件: crates/slab-app-core/src/domain/services/__tests__/plugin_validation_test.rs

#[cfg(test)]
mod tests {
    #[test]
    fn valid_manifest_passes_validation() { /* 完整合法 manifest */ }

    #[test]
    fn duplicate_route_ids_rejected() { /* 重复 route.id */ }

    #[test]
    fn missing_permission_for_routes_rejected() { /* route 贡献无 route:create 权限 */ }

    #[test]
    fn invalid_route_path_rejected() { /* route path 格式错误 */ }

    #[test]
    fn duplicate_sidebar_ids_rejected() { /* ... */ }

    #[test]
    fn duplicate_command_ids_rejected() { /* ... */ }

    #[test]
    fn duplicate_setting_ids_rejected() { /* ... */ }

    // ... 每种 contribution type 的合法/非法场景
}
```

#### Refactor — 注册表模式

```rust
// crates/slab-app-core/src/domain/services/plugin/validation.rs (新建)

struct ContributionValidator {
    contribution_type: &'static str,
    id_extractor: fn(&PluginContributes) -> Vec<String>,
    required_permission: Option<&'static str>,
    item_validator: Option<fn(&serde_json::Value) -> Result<(), String>>,
}

const CONTRIBUTION_VALIDATORS: &[ContributionValidator] = &[
    ContributionValidator {
        contribution_type: "routes",
        id_extractor: |c| c.routes.iter().map(|r| r.id.clone()).collect(),
        required_permission: Some("route:create"),
        item_validator: Some(validate_route_item),
    },
    ContributionValidator {
        contribution_type: "sidebar",
        id_extractor: |c| c.sidebar.iter().map(|s| s.id.clone()).collect(),
        required_permission: None,
        item_validator: None,
    },
    // ... commands, settings, views ...
];

fn validate_contributions(manifest: &PluginManifest) -> Result<(), String> {
    for validator in CONTRIBUTION_VALIDATORS {
        validate_duplicate_ids(validator.contribution_type, (validator.id_extractor)(&manifest.contributes))?;
        if let Some(perm) = validator.required_permission {
            ensure_permission(&manifest.permissions, perm, ...)?;
        }
        if let Some(validate) = validator.item_validator {
            for item in extract_items(&manifest.contributes, validator.contribution_type) {
                validate(item)?;
            }
        }
    }
    Ok(())
}
```

#### Green — 验收标准

- [ ] 验证函数从 180 行缩减至 ~60 行
- [ ] 新增 contribution type 只需添加一行到 `CONTRIBUTION_VALIDATORS`
- [ ] 所有现有验证测试通过

---

### P1-T4: 简化 Model Config 字段构建

**来源审计：** 02-rust §1.1.2
**严重度：** HIGH（代码量）
**当前问题：** `build_model_config_sections` 200+ 行重复调用

#### Red — 表征测试

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn summary_section_contains_expected_fields() {
        // 验证 summary section 字段列表与重构前一致
    }

    #[test]
    fn all_sections_have_consistent_structure() {
        // 每个 field 都有 id, scope, label, description, value_type
    }

    #[test]
    fn field_count_matches_current_output() {
        // 字段总数不变
    }
}
```

#### Refactor — 声明式宏

```rust
macro_rules! config_field {
    ($section:expr, $id:expr, $label:expr, $desc:expr, $value:expr, $origin:expr) => {
        build_model_config_field(
            $id,
            $section,
            $label,
            Some($desc.into()),
            value_type_of(&$value),
            $value,
            $origin,
        )
    };
}

// 使用:
let summary_fields = vec![
    config_field!(ModelConfigFieldScope::Summary, "model.id", "Model ID",
        "Catalog identifier from pack manifest.", &model.id, &origin),
    config_field!(ModelConfigFieldScope::Summary, "model.display_name", "Display Name",
        "Human-readable name.", &model.display_name, &origin),
    // ...
];
```

#### Green — 验收标准

- [ ] 代码行数减少 ~40%
- [ ] 字段输出与重构前完全一致（逐字段对比测试）

---

## Phase 2 — Rust 运行时去重

> **目标：** 消除 slab-runtime 后端实现 ~78% 代码重复
> **预估工期：** 2 周
> **审计来源：** 06-runtime-audit

---

### P2-T1: 提取通用 BackendEngine trait

**来源审计：** 06-runtime §Priority 1.1
**严重度：** HIGH
**当前问题：** 5 个后端实现各自重复 library loading, context management, error handling

#### Red — 表征测试

```rust
// 文件: bin/slab-runtime/tests/backend_abstraction_test.rs (新建)

#[tokio::test]
async fn ggml_llama_load_unload_cycle() { /* 表征现有行为 */ }

#[tokio::test]
async fn ggml_whisper_load_unload_cycle() { /* ... */ }

#[tokio::test]
async fn candle_llama_load_unload_cycle() { /* ... */ }

#[tokio::test]
async fn onnx_load_unload_cycle() { /* ... */ }

#[tokio::test]
async fn error_types_preserve_current_messages() {
    // 验证错误消息格式不变
}
```

#### Refactor — 通用 trait

```rust
// crates/slab-runtime-core/src/backend/mod.rs (新建目录)

pub trait BackendEngine: Send + Sync + 'static {
    type LoadConfig;
    type LoadMetadata;
    type InferenceRequest;
    type InferenceResponse;
    type Error: std::error::Error + Send + Sync;

    fn load_model(&mut self, config: Self::LoadConfig) -> Result<Self::LoadMetadata, Self::Error>;
    fn unload_model(&mut self) -> Result<(), Self::Error>;
}

// crates/slab-runtime-core/src/backend/worker.rs

pub struct BackendWorker<E: BackendEngine> {
    engine: Option<Arc<E>>,
}

// 通用 load/unload 实现，消除 5 个后端的重复代码
```

#### Green — 验收标准

- [ ] 每个 backend 的 engine.rs 缩减 > 30%
- [ ] worker.rs 使用通用 `BackendWorker<E>`
- [ ] `cargo test -p slab-runtime` 全部通过

---

### P2-T2: 合并 Engine Error 类型

**来源审计：** 06-runtime §Priority 2.4
**严重度：** MEDIUM
**当前问题：** 5 个后端各自定义 90% 相同的 EngineError

#### Red → Refactor → Green

```rust
// crates/slab-runtime-core/src/backend/error.rs (新建)

/// 通用引擎错误 — 参数化为后端特定变体
#[derive(Debug, thiserror::Error)]
pub enum EngineError<L: std::fmt::Debug, C: std::fmt::Debug> {
    #[error("failed to initialize dynamic library at {path}: {source}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        source: libloading::Error,
    },
    #[error("context not initialized")]
    ContextNotInitialized,
    #[error("lock poisoned during {operation}")]
    LockPoisoned { operation: String },
    #[error("library error: {0:?}")]
    Library(L),
    #[error("context error: {0:?}")]
    Context(C),
}
```

- [ ] 错误消息格式与现有完全一致
- [ ] 消除 ~70% 错误类型定义重复

---

### P2-T3: 提取共享 Session 管理

**来源审计：** 06-runtime §Finding R5
**严重度：** MEDIUM
**当前问题：** ggml.llama 和 candle.llama 重复实现 session 管理

#### Refactor

```rust
// crates/slab-runtime-core/src/backend/session.rs (新建)

pub struct SessionManager<S: SessionSnapshot> {
    bindings: HashMap<String, SessionBinding<S>>,
}

pub struct SessionBinding<S> {
    pub snapshot: Option<S>,
    pub cached_prompt: String,
    pub grammar: Option<String>,
}

impl<S: SessionSnapshot> SessionManager<S> {
    pub fn plan_session_reuse(...) -> SessionReusePlan { ... }
    pub fn get_or_create(&mut self, session_id: &str) -> &mut SessionBinding<S> { ... }
}
```

- [ ] `cargo test -p slab-runtime` 通过

---

### P2-T4: 消除 contract.rs 文件

**来源审计：** 06-runtime §Finding R1
**严重度：** LOW
**当前问题：** 9 个 contract.rs 文件 100% 重复（纯 re-export）

#### Refactor

直接在 engine.rs 和 worker.rs 中 import `crate::domain::models::*`，删除全部 contract.rs。

- [ ] 删除 9 个文件
- [ ] `cargo check --workspace` 无 warning

---

## Phase 3 — TypeScript 前端 Hook 拆分

> **目标：** 降低前端代码复杂度，消除大型 Hook 和 prop drilling
> **预估工期：** 2 周
> **审计来源：** 03-frontend-typescript-audit, 02-frontend-audit

---

### P3-T1: 拆分 use-audio.ts (932 行 → 4 个 Hook)

**来源审计：** 03-frontend §1.1, 02-frontend §Finding 3
**严重度：** HIGH

#### Red — 表征测试

```typescript
// 文件: packages/slab-desktop/src/pages/audio/__tests__/use-audio.behavior.test.ts (新建)

describe('use-audio behavior preservation', () => {
  it('should start transcription with selected model')
  it('should handle model download flow')
  it('should update VAD settings')
  it('should maintain task history state')
  it('should handle transcription errors gracefully')
  it('should compute selectedVadModelId with correct fallback chain')
  it('should respect enableVad flag')
})
```

#### Refactor

```
pages/audio/hooks/
├── use-audio.ts              → 薄编排层 (< 80 行)
├── use-audio-transcription.ts → 核心转录逻辑
├── use-audio-model.ts         → 模型管理/下载
├── use-audio-vad.ts           → VAD 配置 + selectedVadModelId 计算
└── use-audio-history.ts       → 任务历史管理
```

#### Green — 验收标准

- [ ] `bun run test` 通过
- [ ] 每个 hook < 250 行
- [ ] use-audio.ts 作为编排层 < 80 行
- [ ] 页面行为无变化（人工验证转录流程）

---

### P3-T2: 拆分 use-workspace-page.ts (815 行 → 4 个 Hook)

**来源审计：** 03-frontend §1.1
**严重度：** HIGH

#### Refactor

```
pages/workspace/hooks/
├── use-workspace-page.ts      → 薄编排层 (< 80 行)
├── use-workspace-files.ts      → 文件读写/保存
├── use-workspace-git.ts        → git status/diff/stage/commit
├── use-workspace-lsp.ts        → LSP 集成（已有，整合）
└── use-workspace-search.ts     → 搜索功能
```

#### Green — 验收标准

- [ ] `bun run test` 通过
- [ ] `bun run build:desktop` 成功
- [ ] 工作区文件操作行为不变

---

### P3-T3: 重构 AudioWorkbench prop drilling (94 props)

**来源审计：** 03-frontend §1.3
**严重度：** HIGH

#### Red — 表征测试

```typescript
describe('AudioWorkbench context', () => {
  it('should pass decode settings through context')
  it('should pass VAD settings through context')
  it('should maintain all user interactions')
})
```

#### Refactor

```typescript
// 新建 contexts 将相关 props 分组
// pages/audio/contexts/
├── decode-settings-context.tsx   // 14 个 decode props → 1 个 context
├── vad-settings-context.tsx      // 7 个 VAD props → 1 个 context
└── audio-actions-context.tsx     // 20+ action callbacks → 1 个 context

// AudioWorkbench 简化为:
interface AudioWorkbenchProps {
  children?: React.ReactNode
  // < 10 个顶层 props
}
```

#### Green — 验收标准

- [ ] AudioWorkbench props < 15
- [ ] `bun run test` 通过
- [ ] 所有音频 UI 交互不变

---

### P3-T4: 消除嵌套三元运算符与复杂条件

**来源审计：** 02-frontend §Finding 4, 03-frontend §1.2
**严重度：** MEDIUM

#### 扫描与修复清单

使用 `grep -rn "? " --include="*.ts" --include="*.tsx" packages/slab-desktop/src` 扫描所有嵌套三元。每个按以下模式修复：

```typescript
// BEFORE (嵌套三元):
const label = isPending ? 'Pending' : isLocal ? 'Local' : `Imported from ${repo}`;

// AFTER (卫语句 + 提前返回):
function getModelLabel(model: ModelItem, t: TranslationFunction): string {
  if (model.pending) return t('pages.hub.catalog.descriptions.pending', { backend });
  if (model.local_path) return t('pages.hub.catalog.descriptions.local', { backend });
  return t('pages.hub.catalog.descriptions.imported', { backend, repo });
}
```

#### 重点关注文件

| 文件 | 问题 |
|------|------|
| `pages/audio/hooks/use-audio.ts` | VAD 模型选择逻辑 (L260-285) |
| `pages/assistant/lib/assistant-message-projection.ts` | 消息状态映射 |
| `pages/workspace/lib/workspace-page-utils.ts` | 条件工具函数 |
| `pages/task/utils.ts` | 任务状态计算 |
| `pages/hub/components/hub-catalog-table.tsx` | 模型描述逻辑 |

#### Green — 验收标准

- [ ] 无嵌套三元运算符（lint 规则验证）
- [ ] `bun run lint` 通过
- [ ] 所有条件逻辑输出与重构前一致

---

## Phase 4 — 跨层一致性与基础设施

> **目标：** 统一错误处理、类型安全、测试组织
> **预估工期：** 1 周
> **审计来源：** 02-rust, 04-api, 05-consistency

---

### P4-T1: 自动化 TypeScript 类型漂移检测

**来源审计：** 04-api §M1, 05-consistency
**严重度：** MEDIUM

#### 实施

```yaml
# .github/workflows/api-type-check.yml (新建)
name: API Type Drift Check
on: [pull_request]
jobs:
  check-types:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: bun install
      - run: bun run gen:api
      - run: git diff --exit-code packages/api/src/v1.d.ts
```

- [ ] PR 中修改 Rust API schema 但未更新 v1.d.ts 时 CI 失败

---

### P4-T2: 统一错误处理模式

**来源审计：** 02-rust §3.2.1, 03-frontend §1.4, 04-api §3
**严重度：** MEDIUM

#### Rust 侧

```rust
// 实现 From<AgentError> for AppCoreError
// 替换 agent_err_to_server 函数为标准 From trait
impl From<AgentError> for AppCoreError {
    fn from(e: AgentError) -> Self {
        match e {
            AgentError::ThreadNotFound(id) => Self::NotFound(format!("thread {id}")),
            AgentError::ThreadLimitExceeded { current, max } =>
                Self::TooManyRequests(format!("thread limit: {current}/{max}")),
            AgentError::ThreadBusy(id) =>
                Self::TooManyRequests(format!("thread {id} busy")),
            other => Self::Internal(other.to_string()),
        }
    }
}
```

#### TypeScript 侧

```typescript
// 全局搜索替换:
// error instanceof Error ? error.message : String(error)
// → getErrorMessage(error)

// 确保 packages/slab-desktop 中所有 error message 提取统一使用:
import { getErrorMessage } from '@slab/api'
```

#### Green — 验收标准

- [ ] Rust: `agent_err_to_server` 函数删除，全部使用 `?` 操作符
- [ ] TS: 无直接 `error instanceof Error ? error.message : String(error)` 模式
- [ ] `cargo test --workspace && bun run test` 通过

---

### P4-T3: 标准化测试目录结构

**来源审计：** 05-consistency §Testing Patterns
**严重度：** MEDIUM

#### 目标结构

```
packages/api/
├── tests/
│   ├── unit/              # *.unit.test.ts
│   └── integration/       # *.integration.test.ts

packages/slab-desktop/
├── tests/
│   ├── unit/              # *.unit.test.ts
│   ├── browser/           # *.browser.test.tsx
│   └── e2e/               # *.e2e.test.ts

bin/slab-server/
├── tests/
│   ├── integration/       # *.integration.test.ts
│   └── smoke/             # *.smoke.test.ts
```

- [ ] 所有 `src/__tests__/` 迁移至 `tests/unit/`
- [ ] 测试文件命名一致：`*.unit.test.ts`, `*.browser.test.tsx`, `*.integration.test.ts`

---

### P4-T4: 服务分组重构

**来源审计：** 01-architecture §Warning 2, 02-rust §P7
**严重度：** MEDIUM

#### Refactor

```
crates/slab-app-core/src/domain/services/
├── mod.rs              → AppServices 结构体不变
├── media/              → 新目录
│   ├── mod.rs
│   ├── audio.rs
│   ├── image.rs
│   ├── video.rs
│   ├── subtitle.rs
│   └── media_task.rs   → P1-T2 抽取的共享逻辑
├── workspace/          → 新目录
│   ├── mod.rs
│   ├── workspace.rs
│   └── workspace_lsp.rs
├── admin/              → 新目录
│   ├── mod.rs
│   ├── settings.rs
│   ├── setup.rs
│   └── system.rs
├── chat/               → 已有目录，P1-T1 扩展
├── model/              → 已有目录
├── agent.rs
├── backend.rs
├── ffmpeg.rs
├── plugin.rs
├── session.rs
└── ui_state.rs
```

#### Green — 验收标准

- [ ] `cargo check -p slab-app-core` 无 warning
- [ ] `mod.rs` 中 pub 导出不变（外部 crate 无感知）
- [ ] 服务实例化逻辑不变

---

## Phase 5 — 低优先级持续改进

> **目标：** 小幅改进，可穿插在日常工作中
> **预估工期：** 持续
> **审计来源：** 所有审计的 LOW 项

---

### P5-T1: Zustand store 通用工具

```typescript
// packages/slab-desktop/src/lib/string-utils.ts
export function validateAndTrim(value: string): string | null {
  const trimmed = value.trim();
  return trimmed || null;
}
```

- [ ] 所有 store 中 `x.trim()` 模式替换为 `validateAndTrim()`

### P5-T2: 拆分 AppCoreError::Internal

```rust
pub enum AppCoreError {
    // ... 现有变体
    InternalSerialization(String),  // JSON/序列化
    InternalState(String),          // 意外状态
    InternalOperation(String),      // 杂项操作
}
```

### P5-T3: 统一 Handler State 模式

- [ ] 全部使用 `State<Service>` 或 `State<AppState>`，不混用

### P5-T4: 响应类型命名统一

- [ ] `DeleteSessionResponse` vs `DeletedModelView` → 选一种模式

### P5-T5: 减少 Arc 不必要 clone

- [ ] 审计 `self.state.clone()` 热路径，改为引用

---

## 风险矩阵与验收标准

### 风险评估

| Phase | 风险 | 缓解措施 |
|-------|------|---------|
| P1 | 重构 Chat 服务可能影响流式响应 | 表征测试覆盖所有 stream 路径 |
| P1 | Media 服务合并可能引入回归 | 每个媒体类型独立表征测试 |
| P2 | Runtime trait 抽象可能过度设计 | 保持 backend_handler 宏不变，只抽共享逻辑 |
| P3 | Hook 拆分可能破坏 React 重渲染 | 每个拆分独立提交，浏览器验证 |
| P4 | 服务分组可能破坏 pub 导出 | `mod.rs` re-export 保持外部接口不变 |

### 总体验收流程

每个 Phase 完成后执行：

```
1. cargo check --workspace     # 编译无错
2. cargo test --workspace      # 所有测试通过
3. bun install && bun run lint # 前端 lint
4. bun run test                # 前端测试通过
5. bun run build:desktop       # 构建成功
6. bun run gen:api             # 类型无漂移
7. 手动冒烟测试关键路径:
   - 聊天补全（流式 + 非流式）
   - 音频转录
   - 工作区文件操作
   - 插件安装/卸载
```

---

## 审计来源索引

| 审计文件 | 关键贡献 |
|----------|---------|
| [00-executive-summary.md](../audits/00-executive-summary.md) | 总体评级、优先级排序 |
| [01-architecture-analysis.md](../audits/01-architecture-analysis.md) | 分层验证 PASS、依赖分析、服务分组建议 |
| [01-architecture-audit.md](../audits/01-architecture-audit.md) | Crate 复杂度、六边形架构验证 |
| [02-rust-backend-audit.md](../audits/02-rust-backend-audit.md) | Chat 嵌套、Model 字段、Plugin 验证、错误处理 |
| [02-frontend-audit.md](../audits/02-frontend-audit.md) | 嵌套三元、thinking parser、状态标签 |
| [03-backend-audit.md](../audits/03-backend-audit.md) | Media 服务 ~80% 重复、Chat 复杂度 |
| [03-frontend-typescript-audit.md](../audits/03-frontend-typescript-audit.md) | Hook 大小、prop drilling、错误处理不一致 |
| [04-api-interface-audit.md](../audits/04-api-interface-audit.md) | 类型漂移、REST 一致性、验证框架 |
| [04-api-protocol-audit.md](../audits/04-api-protocol-audit.md) | Schema 重复、OpenAI 协议深度 |
| [05-consistency-audit.md](../audits/05-consistency-audit.md) | 测试目录、TS 配置、命名一致性 |
| [06-runtime-audit.md](../audits/06-runtime-audit.md) | 后端 ~78% 重复、BackendHandler 宏、FFI 安全 |

---

*Generated by Claude Code — 2026-05-30*
*本文档应随实施进展更新状态标记*
