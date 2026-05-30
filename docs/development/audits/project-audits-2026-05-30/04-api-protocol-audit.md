# API & Protocol Design Audit Report

**Date:** 2026-05-30  
**Auditor:** API & Protocol Design Auditor  
**Scope:** slab-workspace API design, protocol definitions, and interface contracts

## Executive Summary

### Key Findings

1. **Schema Duplication Between HTTP and Tauri Layers** - The codebase maintains identical schema definitions in both `bin/slab-server/src/api/v1/*/schema.rs` and `crates/slab-app-core/src/schemas/*.rs`, creating redundancy and maintenance overhead.

2. **OpenAI Protocol Nesting Excessively Deep** - The `crates/slab-proto/src/openai/models/chat/completion/request/message.rs` path structure goes 7 levels deep, creating unnecessary complexity for relatively simple message types.

3. **Error Handling Inconsistencies Across Layers** - While TypeScript and Rust error codes align (4004, 4000, 5000-5003), the OpenAI-specific error handling in chat handlers diverges from the standard ServerError pattern.

4. **Validation Patterns Consistent But Could Be Simplified** - The `ValidatedJson` and `ValidatedQuery` wrappers provide good consistency, but the repeated path parameter validation in individual handlers could be centralized.

5. **Good Domain Port Pattern** - The `RuntimeInferenceGateway` trait in `domain/ports/runtime.rs` provides a clean abstraction between business logic and runtime implementation details.

---

## API Design Consistency Assessment

### Handler/Schema Pattern Analysis

**Pattern Observed:**
```
bin/slab-server/src/api/v1/{domain}/
├── handler.rs      # HTTP handlers with utoipa docs
└── schema.rs       # Re-exports from slab-app-core
```

**Consistency Level:** ★★★★☆ (4/5)

#### Strengths:
1. **Uniform Structure** - All 14 API modules (agent, audio, backend, chat, ffmpeg, images, models, plugins, session, settings, subtitles, system, tasks, ui_state, video, workspace_lsp) follow the same handler/schema pattern
2. **Schema Reuse** - `pub use slab_app_core::schemas::*::*` eliminates duplication between HTTP and Tauri layers
3. **Validation Consistency** - All handlers use `ValidatedJson<T>` and `ValidatedQuery<T>` wrappers
4. **Error Conversion** - Consistent `Into<ServerError>` conversions across modules

#### Issues Identified:

**Issue #1: Redundant Schema Module Pattern**
- **Location:** All API modules
- **Severity:** Medium  
- **Description:** Each API module has a `schema.rs` that only contains `pub use slab_app_core::schemas::*::*`, creating 15+ nearly identical files
- **Impact:** Maintenance overhead when adding new schemas
- **Recommendation:** Consider a centralized schema re-export or direct imports

**Issue #2: Path Parameter Validation Duplication**
- **Location:** `bin/slab-server/src/api/v1/tasks/handler.rs:152-159`, `models/handler.rs:382-389`, etc.
- **Severity:** Low
- **Description:** Path parameter validation logic duplicated across modules:
  ```rust
  #[derive(Debug, Deserialize, Validate)]
  struct ModelIdPath {
      #[validate(custom(function = "...", message = "id must not be empty"))]
      id: String,
  }
  ```
- **Impact:** Code duplication, potential for inconsistencies
- **Recommendation:** Create a shared `IdPath` type or validation function

**Issue #3: OpenAI Error Handling Divergence**
- **Location:** `bin/slab-server/src/api/v1/chat/handler.rs:144-199`
- **Severity:** Medium
- **Description:** Chat completions handler implements custom OpenAI error format instead of using standard `ServerError::IntoResponse`
- **Impact:** Inconsistent error responses between chat and other endpoints
- **Recommendation:** Align with standard ServerError pattern or document the divergence reason

### Request Validation Consistency

**Assessment:** ★★★★★ (5/5)

The validation framework is exemplary:
- Shared validation functions in `crates/slab-app-core/src/schemas/validation.rs`
- Custom validators: `validate_non_blank`, `validate_absolute_path`, `validate_backend_id`, `validate_chat_role`, `validate_ffmpeg_output_format`
- Consistent use of `validator::Validate` derive macro
- Good error messages with context

### Response Type Consistency

**Assessment:** ★★★★☆ (4/5)

**Strengths:**
- Consistent use of `Result<Json<T>, ServerError>` for synchronous responses
- Consistent use of `(StatusCode, Json<T>)` for accepted operations (202)
- Good SSE streaming pattern in chat completions

**Issue #4: Mixed Response Patterns**
- **Location:** `bin/slab-server/src/api/v1/images/handler.rs:118-124`, `video/handler.rs:112-118`
- **Severity:** Low
- **Description:** Some endpoints return `impl IntoResponse` instead of concrete types
- **Impact:** Slightly less predictable response handling
- **Recommendation:** Standardize on concrete response types where feasible

---

## OpenAI Compatibility Assessment

### Protocol Structure Analysis

**Module Structure:**
```
crates/slab-proto/src/openai/
├── models/
│   ├── chat/
│   │   ├── completion/
│   │   │   └── request/
│   │   │       ├── assistant.rs
│   │   │       └── message.rs  ← 7 levels deep!
│   ├── tools/
│   ├── responses/
│   └── ...
```

**Assessment:** ★★☆☆☆ (2/5)

#### Strengths:
1. **Comprehensive Coverage** - Implements most OpenAI API types
2. **Generated from OpenAPI** - Types are generated from `openai/openapi/openapi.yaml`
3. **Test Coverage** - Tests in `src/openai/tests/` verify behavior

#### Critical Issues:

**Issue #5: Excessive Module Nesting**
- **Location:** `crates/slab-proto/src/openai/models/chat/completion/request/message.rs`
- **Severity:** High
- **Description:** The path structure goes 7 levels deep for what are essentially simple message types
- **Example:** 
  ```rust
  pub use crate::openai::models::chat::completion::request::message::
      ChatCompletionRequestSystemMessage
  ```
- **Impact:** Difficult to navigate, poor IDE performance, cognitive overhead
- **Recommendation:** Flatten to `crate::openai::models::ChatCompletionRequestSystemMessage`

**Issue #6: Redundant Type Wrappers**
- **Location:** Throughout `slab-proto/src/openai/models/`
- **Severity:** Medium
- **Description:** Many types are wrapped in `Box<>` unnecessarily:
  ```rust
  pub content: Box<models::ChatCompletionRequestSystemMessageContent>
  pub image_url: Box<models::ChatCompletionRequestMessageContentPartImageImageUrl>
  ```
- **Impact:** Runtime overhead without clear benefit
- **Recommendation:** Remove unnecessary boxing

**Issue #7: Duplicate Message Type Definitions**
- **Location:** `slab-proto` vs `slab-app-core/src/schemas/chat.rs`
- **Severity:** High
- **Description:** `ChatMessage` defined twice:
  - `slab-proto/src/openai/models/chat/completion/request/message.rs` (OpenAI standard)
  - `slab-app-core/src/schemas/chat.rs:103-120` (Slab-specific)
- **Impact:** Confusion about which to use, potential for divergence
- **Recommendation:** Use proto types directly or document transformation path

### OpenAI API Coverage

**Assessment:** ★★★★☆ (4/5)

**Supported:**
- Chat completions (streaming/non-streaming)
- Text completions
- Audio transcription
- Image generation
- Tool/function calling
- Structured output
- Reasoning controls (thinking, verbosity)

**Missing/Partial:**
- Embeddings (defined but not integrated in main API)
- Batch API
- Fine-tuning endpoints
- Some newer OpenAI features (audio input in messages)

---

## Interface Design Assessment

### Domain Port Pattern

**Location:** `crates/slab-app-core/src/domain/ports/`

**Assessment:** ★★★★★ (5/5)

**Excellent Design:**

```rust
#[async_trait]
pub trait RuntimeInferenceGateway: Send + Sync + std::fmt::Debug {
    async fn chat(&self, request: RuntimeTextGenerationRequest) 
        -> Result<RuntimeTextGenerationResponse, AppCoreError>;
    async fn generate_image(&self, request: RuntimeDiffusionImageRequest) 
        -> Result<RuntimeDiffusionImageResult, AppCoreError>;
    // ...
}
```

**Strengths:**
1. **Clean Abstraction** - Hides protobuf/gRPC/implementation details
2. **Async Trait Pattern** - Proper use of `async_trait`
3. **Domain Types** - Uses domain types, not backend types
4. **Error Handling** - Consistent `Result<T, AppCoreError>`
5. **No Leaks** - No HTTP, no protobuf, no backend-specific types

### Schema-to-Domain Transformation

**Location:** `crates/slab-app-core/src/schemas/chat.rs:665-890`

**Assessment:** ★★★☆☆ (3/5)

**Pattern:** Extensive `From<T>` implementations for bidirectional conversion

**Strengths:**
- Comprehensive coverage
- Type-safe transformations
- Clear mapping logic

**Issues:**

**Issue #8: Complex Transformation Logic**
- **Location:** `slab-app-core/src/schemas/chat.rs:1068-1122`
- **Severity:** Medium
- **Description:** Complex nested `if let` chains in `structured_output_from_api`:
  ```rust
  fn structured_output_from_api(...) -> Option<DomainStructuredOutput> {
      if let Some(schema) = json_schema {
          return Some(...);
      }
      let response_format = response_format?;
      match response_format.format_type {
          ChatResponseFormatType::Text => None,
          ChatResponseFormatType::JsonObject | ChatResponseFormatType::JsonSchema => 
              response_format.json_schema.map(...)
              .or_else(|| response_format.schema.map(...))
              .or(Some(...))
      }
  }
  ```
- **Impact:** Hard to reason about, easy to introduce bugs
- **Recommendation:** Simplify with guard clauses or early returns

**Issue #9: Inconsistent Field Naming**
- **Location:** Various schema files
- **Severity:** Low
- **Description:** Mix of camelCase (API) and snake_case (domain) with inconsistent transformations
- **Examples:**
  - API: `reasoningEffort` → Domain: `reasoning_effort` (handled via alias)
  - API: `backend_id` → Domain: `backend_id` (same)
  - API: `repoId` → Domain: `repo_id` (inconsistent alias usage)
- **Impact:** Confusion for API consumers
- **Recommendation:** Standardize on consistent alias naming pattern

---

## Error Handling Assessment

### Error Type Hierarchy

**TypeScript Error Codes** (`packages/api/src/errors.ts`):
```typescript
export const ErrorCodes = {
  NOT_FOUND: 4004,
  BAD_REQUEST: 4000,
  BACKEND_NOT_READY: 5003,
  RUNTIME_ERROR: 5000,
  DATABASE_ERROR: 5001,
  INTERNAL_ERROR: 5002,
} as const;
```

**Rust Error Codes** (`bin/slab-server/src/error.rs`):
```rust
mod error_codes {
    pub const NOT_FOUND: u16 = 4004;
    pub const BAD_REQUEST: u16 = 4000;
    pub const BACKEND_NOT_READY: u16 = 5003;
    pub const RUNTIME_ERROR: u16 = 5000;
    pub const DATABASE_ERROR: u16 = 5001;
    pub const INTERNAL_ERROR: u16 = 5002;
    pub const NOT_IMPLEMENTED: u16 = 5010;
    pub const TOO_MANY_REQUESTS: u16 = 4029;
}
```

**Assessment:** ★★★★☆ (4/5)

**Strengths:**
1. **Consistent Codes** - Same numeric codes across TypeScript and Rust
2. **Good Coverage** - All major error categories represented
3. **Security Conscious** - Internal errors logged but generic messages returned

**Issues:**

**Issue #10: Error Type Redundancy**
- **Location:** `ServerError` vs `AppCoreError`
- **Severity:** Low
- **Description:** Nearly identical error types in different crates:
  - `bin/slab-server/src/error.rs:ServerError`
  - `crates/slab-app-core/src/error.rs:AppCoreError`
- **Impact:** Maintenance overhead
- **Recommendation:** Could consolidate or use inheritance pattern

**Issue #11: OpenAI Error Format Inconsistency**
- **Location:** `chat/handler.rs:144-199` vs standard error handling
- **Severity:** Medium
- **Description:** Chat completions return OpenAI-style errors:
  ```rust
  OpenAiErrorResponse { error: OpenAiError { message, error_type, param, code } }
  ```
  While other endpoints return:
  ```rust
  ErrorResponse { code, data, message }
  ```
- **Impact:** Inconsistent error responses across API
- **Recommendation:** Standardize or document divergence clearly

**Issue #12: Validation Error Handling**
- **Location:** `error.rs:200-242`
- **Severity:** Low
- **Description:** Validation error formatting is complex and nested:
  ```rust
  fn collect_validation_messages(...) {
      for (field, kind) in errors.errors() {
          match kind {
              ValidationErrorsKind::Field(field_errors) => { ... }
              ValidationErrorsKind::Struct(nested) => { ... }
              ValidationErrorsKind::List(items) => { ... }
          }
      }
  }
  ```
- **Impact:** Difficult to customize error formatting
- **Recommendation:** Consider simpler error format or configurable formatter

---

## Detailed Findings

### Finding #1: Schema Module Pattern Redundancy

**File:** `bin/slab-server/src/api/v1/*/schema.rs`

**Severity:** Medium

**Description:**
All API modules contain schema files that only re-export from `slab-app-core`:
```rust
// bin/slab-server/src/api/v1/audio/schema.rs
pub use slab_app_core::schemas::audio::*;

// bin/slab-server/src/api/v1/chat/schema.rs
pub use slab_app_core::schemas::chat::*;

// ... repeated 15+ times
```

**Recommendation:**
Option 1: Direct imports in handlers
```rust
use slab_app_core::schemas::audio::*;
```

Option 2: Centralized schema re-export
```rust
// bin/slab-server/src/api/schemas.rs
pub mod audio { pub use slab_app_core::schemas::audio::*; }
pub mod chat { pub use slab_app_core::schemas::chat::*; }
```

---

### Finding #2: Excessive OpenAI Module Nesting

**File:** `crates/slab-proto/src/openai/models/` directory structure

**Severity:** High

**Description:**
Current structure:
```
openai/
  models/
    chat/
      completion/
        request/
          message.rs (7 levels!)
```

Example type path:
```rust
crate::openai::models::chat::completion::request::message::ChatCompletionRequestUserMessage
```

**Recommendation:**
Flatten to 2-3 levels maximum:
```
openai/
  models/
    chat_request.rs
    chat_response.rs
    common.rs
    tools.rs
```

Result:
```rust
crate::openai::models::ChatCompletionRequestUserMessage
```

---

### Finding #3: Duplicate ChatMessage Definitions

**Files:** 
- `crates/slab-proto/src/openai/models/chat/completion/request/message.rs`
- `crates/slab-app-core/src/schemas/chat.rs:103-120`

**Severity:** High

**Description:**
Two separate `ChatMessage` definitions serve similar purposes:

**OpenAI version** (447 lines):
```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionRequestMessage {
    ChatCompletionRequestDeveloperMessage(...),
    ChatCompletionRequestSystemMessage(...),
    ChatCompletionRequestUserMessage(...),
    ChatCompletionRequestAssistantMessage(...),
    ChatCompletionRequestToolMessage(...),
    ChatCompletionRequestFunctionMessage(...),
}
```

**Slab version** (20 lines):
```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<ChatMessageContent>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<ChatToolCall>,
}
```

**Recommendation:**
Option 1: Use OpenAI types directly and add validation
Option 2: Document transformation path and keep separate
Option 3: Merge into single type with feature flags

---

### Finding #4: Inconsistent Path Parameter Validation

**Files:** Multiple handler files

**Severity:** Low

**Description:**
Path parameter validation duplicated:
```rust
// tasks/handler.rs:152
#[derive(Debug, Deserialize, Validate)]
struct TaskIdPath {
    #[validate(custom(function = "...", message = "id must not be empty"))]
    id: String,
}

// models/handler.rs:382
#[derive(Debug, Deserialize, Validate)]
struct ModelIdPath {
    #[validate(custom(function = "...", message = "id must not be empty"))]
    id: String,
}
```

**Recommendation:**
Create shared type:
```rust
// api/path_params.rs
#[derive(Debug, Deserialize, Validate)]
pub struct IdPath {
    #[validate(custom(function = "validate_non_blank", message = "id must not be empty"))]
    pub id: String,
}
```

---

### Finding #5: OpenAI Error Response Divergence

**File:** `bin/slab-server/src/api/v1/chat/handler.rs:144-199`

**Severity:** Medium

**Description:**
Chat completions returns OpenAI-style errors:
```rust
OpenAiErrorResponse { 
    error: OpenAiError { 
        message, 
        error_type,  // OpenAI's "type"
        param, 
        code 
    } 
}
```

Standard ServerError returns:
```rust
ErrorResponse { 
    code,      // HTTP status * 10
    data, 
    message 
}
```

**Recommendation:**
1. Document why chat uses OpenAI format (compatibility requirement?)
2. Or standardize all endpoints to same format
3. Or add middleware to convert based on Accept header

---

### Finding #6: Complex Structured Output Transformation

**File:** `crates/slab-app-core/src/schemas/chat.rs:1086-1112`

**Severity:** Medium

**Description:**
The `structured_output_from_api` function has deeply nested logic:
```rust
fn structured_output_from_api(...) -> Option<DomainStructuredOutput> {
    if let Some(schema) = json_schema {
        return Some(...);
    }
    let response_format = response_format?;
    match response_format.format_type {
        ChatResponseFormatType::Text => None,
        ChatResponseFormatType::JsonObject | ChatResponseFormatType::JsonSchema => 
            response_format.json_schema.map(structured_output_json_schema_from_api)
            .or_else(|| response_format.schema.map(|schema| ...))
            .or(Some(DomainStructuredOutput::JsonObject))
    }
}
```

**Recommendation:**
Simplify with guard clauses:
```rust
fn structured_output_from_api(...) -> Option<DomainStructuredOutput> {
    // Legacy field takes precedence
    if let Some(schema) = json_schema {
        return Some(DomainStructuredOutput::JsonSchema(...));
    }
    
    let response_format = response_format?;
    
    // Text format means no structured output
    if matches!(response_format.format_type, ChatResponseFormatType::Text) {
        return None;
    }
    
    // Try json_schema, then schema, then default to JsonObject
    response_format.json_schema
        .map(DomainStructuredOutput::JsonSchema)
        .or_else(|| response_format.schema.map(...))
        .or(Some(DomainStructuredOutput::JsonObject))
}
```

---

## Industry Best Practices Comparison

### API Design Patterns

| Practice | Status | Notes |
|----------|--------|-------|
| RESTful resource naming | ✓ Good | `/v1/models`, `/v1/sessions`, etc. |
| Consistent error responses | △ Partial | OpenAI vs standard divergence |
| Request validation | ✓ Excellent | Validator crate + custom validators |
| OpenAPI documentation | ✓ Excellent | Utoipa integration |
| Versioning strategy | ✓ Good | `/v1` prefix, clear path structure |

### Type Safety

| Practice | Status | Notes |
|----------|--------|-------|
| Shared types across layers | ✓ Good | Schema re-use pattern |
| Domain-driven types | ✓ Excellent | Clear domain models |
| Error type consistency | △ Partial | Some redundancy |
| Validation at boundary | ✓ Excellent | ValidatedJson/Query wrappers |

### Protocol Design

| Practice | Status | Notes |
|----------|--------|-------|
| Interface segregation | ✓ Excellent | Clean port traits |
| Dependency inversion | ✓ Excellent | RuntimeInferenceGateway abstraction |
| DRY principle | ✗ Poor | Schema duplication, excessive nesting |

---

## Prioritized Recommendations

### Priority 1 (Critical - Address Immediately)

1. **Flatten OpenAI Module Structure** - Reduce nesting from 7 to 2-3 levels
2. **Resolve ChatMessage Duplication** - Choose single source of truth or document transformation clearly

### Priority 2 (High - Address Soon)

3. **Standardize Error Response Format** - Either all OpenAI-style or all standard
4. **Consolidate Schema Re-exports** - Eliminate redundant schema.rs files
5. **Simplify Structured Output Logic** - Reduce nesting in transformation functions

### Priority 3 (Medium - Consider for Next Iteration)

6. **Create Shared Path Parameter Types** - Reduce validation duplication
7. **Review Field Naming Consistency** - Standardize camelCase ↔ snake_case mappings
8. **Document Divergence Patterns** - Add inline docs for intentional variations

### Priority 4 (Low - Nice to Have)

9. **Standardize Response Types** - Use concrete types instead of `impl IntoResponse`
10. **Add Error Type Hierarchy** - Consider inheritance for ServerError/AppCoreError

---

## Conclusion

The slab-workspace API and protocol design demonstrates **good foundational patterns** with clean domain separation, consistent validation, and comprehensive OpenAPI documentation. However, there are **areas requiring attention**:

**Strengths:**
- Excellent domain port pattern (RuntimeInferenceGateway)
- Consistent validation framework
- Good error code alignment across layers
- Comprehensive OpenAPI integration

**Areas for Improvement:**
- Reduce module nesting complexity in OpenAI types
- Eliminate schema/re-export duplication
- Standardize error response formats
- Simplify transformation logic

**Overall Assessment:** ★★★☆☆ (3.5/5)

The codebase would benefit from a focused simplification effort, particularly around the OpenAI protocol types and schema organization, without sacrificing the good patterns already established.

---

## Appendix: File Inventory

### API Handler Files
- `bin/slab-server/src/api/v1/agent/handler.rs` (568 lines)
- `bin/slab-server/src/api/v1/audio/handler.rs` (91 lines)
- `bin/slab-server/src/api/v1/backend/handler.rs` 
- `bin/slab-server/src/api/v1/chat/handler.rs` (200 lines)
- `bin/slab-server/src/api/v1/ffmpeg/handler.rs`
- `bin/slab-server/src/api/v1/images/handler.rs` (144 lines)
- `bin/slab-server/src/api/v1/models/handler.rs` (390 lines)
- `bin/slab-server/src/api/v1/plugins/handler.rs`
- `bin/slab-server/src/api/v1/session/handler.rs` (164 lines)
- `bin/slab-server/src/api/v1/setup/handler.rs`
- `bin/slab-server/src/api/v1/settings/handler.rs`
- `bin/slab-server/src/api/v1/subtitles/handler.rs`
- `bin/slab-server/src/api/v1/system/handler.rs`
- `bin/slab-server/src/api/v1/tasks/handler.rs` (160 lines)
- `bin/slab-server/src/api/v1/ui_state/handler.rs`
- `bin/slab-server/src/api/v1/video/handler.rs` (138 lines)
- `bin/slab-server/src/api/v1/workspace_lsp/handler.rs`

### Schema Files
- `crates/slab-app-core/src/schemas/mod.rs` (85 lines)
- `crates/slab-app-core/src/schemas/chat.rs` (1378 lines)
- `crates/slab-app-core/src/schemas/audio.rs` (419 lines)
- `crates/slab-app-core/src/schemas/validation.rs` (99 lines)
- `crates/slab-app-core/src/schemas/tasks.rs` (100+ lines)
- (Additional schema files for each domain)

### Protocol Files
- `crates/slab-proto/src/openai/mod.rs` (13 lines)
- `crates/slab-proto/src/openai/models/mod.rs` (32 lines)
- `crates/slab-proto/src/openai/models/chat/mod.rs` (18 lines)
- `crates/slab-proto/src/openai/models/chat/completion/request/message.rs` (447 lines)
- (Many more OpenAI type files)

### Error Files
- `packages/api/src/errors.ts` (215 lines)
- `bin/slab-server/src/error.rs` (242 lines)
- `crates/slab-app-core/src/error.rs` (105 lines)

### Interface Files
- `crates/slab-app-core/src/domain/ports/mod.rs` (12 lines)
- `crates/slab-app-core/src/domain/ports/runtime.rs` (242 lines)

---

**End of Report**
