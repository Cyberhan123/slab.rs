# API Design and Cross-Boundary Interface Audit

**Date:** 2026-05-30  
**Auditor:** Claude Code (Senior API Designer)  
**Scope:** REST API, gRPC interfaces, WebSocket/JSON-RPC plugin API, TypeScript client types, database schemas, and domain service boundaries

## Executive Summary

### Overall Assessment

| Area | Grade | Severity | Status |
|------|-------|----------|--------|
| REST API Consistency | B+ | Low | Generally consistent with minor inconsistencies |
| Schema Design | A- | Low | Well-structured with good separation of concerns |
| Cross-Boundary Type Safety | B | Medium | TypeScript sync requires manual regeneration |
| Database Schema | B+ | Low | Clean evolution with append-only migrations |
| Plugin API Design | A | Low | Solid JSON-RPC 2.0 implementation |
| Interface Segregation | B+ | Low | Generally well-designed service interfaces |

### Key Strengths
- Consistent OpenAPI documentation using `utoipa`
- Centralized validation framework in `schemas/validation.rs`
- Clean separation between API schemas (`schemas/*.rs`) and domain models (`domain/models/*.rs`)
- Proper use of HTTP status codes and error responses
- JSON-RPC 2.0 compliance for plugin dispatch
- Append-only migration strategy for database schema evolution

### Critical Issues
- **Medium Severity:** TypeScript definitions (`packages/api/src/v1.d.ts`) require manual regeneration via `bun run gen:api`, creating potential drift between Rust and TypeScript contracts
- **Medium Severity:** OpenAI compatibility error handling in chat handler diverges from standard error format
- **Low Severity:** Some handlers use direct `State<Service>` while others use `State<AppState>`

---

## REST API Consistency Analysis

### 1. Handler Pattern Consistency

**Finding:** Generally consistent handler patterns across v1 modules with minor variations.

#### Consistent Patterns Observed:
- All handlers use `ValidatedJson<T>` for request body validation
- All handlers use `ValidatedQuery<T>` for query parameter validation
- All path parameters use `Path<T>` with validation wrapper structs
- All responses use `Json<T>` for JSON responses
- All handlers document their routes with `#[utoipa::path(...)]` attributes

#### Pattern Examples:

**Standard CRUD Pattern (Session Handler):**
```rust
#[utoipa::path(
    post,
    path = "/v1/sessions",
    tag = "sessions",
    request_body = CreateSessionRequest,
    responses(
        (status = 200, description = "Session created", body = SessionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn create_session(
    State(service): State<SessionService>,
    ValidatedJson(req): ValidatedJson<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, ServerError>
```

**Resource with Sub-routes (Models Handler):**
```rust
.route("/models", get(list_models).post(create_model))
.route("/models/import-pack", post(import_model_pack))
.route("/models/{id}", get(get_model).put(update_model).delete(delete_model))
.route("/models/{id}/config-document", get(get_model_config_document))
.route("/models/available", get(list_available_models))
.route("/models/load", post(load_model))
.route("/models/download", post(download_model))
```

### 2. HTTP Method Usage

**Finding:** Proper HTTP verb usage following REST conventions.

| Operation | HTTP Method | Pattern |
|-----------|-------------|---------|
| List resources | GET | `GET /v1/{resources}` |
| Get single resource | GET | `GET /v1/{resources}/{id}` |
| Create resource | POST | `POST /v1/{resources}` |
| Update resource | PUT | `PUT /v1/{resources}/{id}` |
| Delete resource | DELETE | `DELETE /v1/{resources}/{id}` |
| Action operations | POST | `POST /v1/{resources}/{action}` |

**Action Operations Examples:**
- `POST /v1/models/load` - Load model into memory
- `POST /v1/models/unload` - Unload model from memory
- `POST /v1/models/switch` - Switch active model
- `POST /v1/plugins/{id}/enable` - Enable plugin
- `POST /v1/plugins/{id}/disable` - Disable plugin

### 3. Response Format Consistency

**Finding:** Standard response formats across endpoints with one notable exception.

#### Standard Error Response:
```rust
struct ErrorResponse {
    code: u16,
    data: Option<AppCoreErrorData>,
    message: String,
}
```

#### Exception - OpenAI-Compatible Error Format:
The `/v1/chat/completions` endpoint uses a custom OpenAI-compatible error format:

```rust
struct OpenAiErrorResponse {
    error: OpenAiError {
        message: String,
        error_type: String,
        param: Option<String>,
        code: Option<String>,
    }
}
```

**Severity:** Low - This is intentional for OpenAI API compatibility.

### 4. Validation Patterns

**Finding:** Excellent centralized validation framework.

#### Shared Validation Functions (`schemas/validation.rs`):
- `validate_non_blank()` - Non-empty string validation
- `validate_absolute_path()` - Absolute path with traversal protection
- `validate_positive_u32()` - Positive integer validation
- `validate_backend_id()` - Backend ID format validation
- `validate_chat_role()` - Chat role whitelist validation
- `validate_ffmpeg_output_format()` - Format whitelist validation

#### Usage Pattern:
```rust
#[derive(Debug, Deserialize, Validate)]
struct SessionIdPath {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    pub id: String,
}
```

---

## Schema Design Assessment

### 1. Schema Organization

**Finding:** Well-organized schema structure with clear separation of concerns.

```
crates/slab-app-core/src/schemas/
├── mod.rs              # Shared utilities and base64 decoding
├── validation.rs       # Centralized validation functions
├── agent.rs            # Agent-related schemas
├── audio.rs            # Audio transcription schemas
├── backend.rs          # Backend status schemas
├── chat.rs             # Chat completion schemas
├── ffmpeg.rs           # FFmpeg conversion schemas
├── images.rs           # Image generation schemas
├── models.rs           # Model management schemas
├── plugin.rs           # Plugin management schemas
├── session.rs          # Session management schemas
├── setup.rs            # Setup/onboarding schemas
├── subtitles.rs        # Subtitle rendering schemas
├── system.rs           # System/GPU status schemas
├── tasks.rs            # Background task schemas
├── ui_state.rs         # UI state persistence schemas
└── video.rs            # Video generation schemas
```

### 2. Schema Design Quality

**Finding:** High-quality schema design with proper use of Rust type system.

#### Positive Patterns:

1. **Proper Serde Attributes:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]  // Consistent naming convention
pub struct InstallPluginRequest {
    #[validate(custom(...))]
    pub plugin_id: String,
    pub source_id: Option<String>,
    // ...
}
```

2. **Clean Conversion Traits:**
```rust
impl From<SessionView> for SessionResponse {
    fn from(session: SessionView) -> Self {
        Self {
            id: session.id,
            name: session.name,
            // Direct field mapping
        }
    }
}

impl From<CreateSessionRequest> for CreateSessionCommand {
    fn from(request: CreateSessionRequest) -> Self {
        Self { name: request.name }
    }
}
```

3. **Path Parameter Structs:**
```rust
#[derive(Debug, Deserialize, IntoParams, Validate)]
pub struct SessionIdPath {
    #[validate(custom(...))]
    pub id: String,
}
```

### 3. Naming Conventions

**Finding:** Generally consistent naming with some minor inconsistencies.

#### Conventions Observed:
- Request structs: `{Operation}{Resource}Request`
- Response structs: `{Resource}Response`
- Path params: `{Resource}Path`
- Query params: `{Resource}Query`
- Error responses: `{Resource}ErrorResponse`

#### Minor Inconsistencies:
- Some modules use `DeleteSessionResponse`, others use `DeletedModelView`
- Chat module uses `OpenAiErrorResponse` (compatibility) vs standard `ErrorResponse`

---

## Cross-Boundary Type Consistency

### 1. Rust-to-TypeScript Type Safety

**Finding:** Good type safety with manual regeneration requirement.

#### TypeScript Generation Process:
```bash
bun run gen:api  # Regenerates packages/api/src/v1.d.ts from Rust handlers
```

#### Generated TypeScript Structure:
```typescript
export interface paths {
    "/v1/sessions": {
        get: operations["list_sessions"];
        post: operations["create_session"];
    };
    "/v1/sessions/{id}": {
        delete: operations["delete_session"];
        put: operations["update_session"];
    };
}

export interface components {
    schemas: {
        SessionResponse: { /* ... */ };
        CreateSessionRequest: { /* ... */ };
        // ...
    };
}
```

**Severity Rating:** Medium - Manual regeneration step creates potential for drift if developers forget to run after API changes.

### 2. Database Entity to Domain Model Mapping

**Finding:** Clean separation with proper conversion layers.

#### Layer Structure:
```
Database (entities/*) → Domain Models (domain/models/*) → API Schemas (schemas/*)
```

#### Example Mapping:
```rust
// Database Entity
pub struct ChatSession {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Domain Model View
impl From<&ChatSession> for SessionView {
    fn from(session: &ChatSession) -> Self {
        Self {
            id: session.id.clone(),
            name: session.name.clone(),
            state_path: session.state_path.clone(),
            created_at: session.created_at.to_rfc3339(),  // ISO-8601 conversion
            updated_at: session.updated_at.to_rfc3339(),
        }
    }
}
```

**Finding:** Excellent use of `DateTime<Utc>` to `String` (ISO-8601) conversion at boundaries.

### 3. Tauri vs HTTP API Contract

**Finding:** Intentional dual API with shared schemas.

#### Shared Schema Usage:
```rust
// Used by both HTTP (bin/slab-server) and Tauri (bin/slab-app)
use slab_app_core::schemas::session::{
    CreateSessionRequest, SessionResponse, DeleteSessionResponse
};
```

#### Tauri Command Example:
```rust
#[tauri::command(async)]
pub async fn create_session(
    state: tauri::State<'_, Arc<AppState>>,
    req: CreateSessionRequest,  // Shared schema type
) -> Result<SessionResponse, String>
```

**Severity Rating:** Low - Well-architected for dual entry points.

---

## Database Schema Review

### 1. Migration Strategy

**Finding:** Excellent append-only migration approach.

#### Migration Pattern:
```
20240101000000_initial.sql
20260325000000_agent_tables.sql
20260408000000_model_kind_and_backend.sql
20260408010000_model_config_versions.sql
20260408020000_model_capabilities.sql
20260409000000_model_config_state.sql
20260410000000_ui_state.sql
20260410010000_model_downloads.sql
20260530000000_remove_models_provider.sql
20260530010000_task_payload_envelopes.sql
```

**Positive Aspects:**
- Timestamp-based naming prevents conflicts
- Append-only (no migrations are modified)
- Incremental schema evolution
- Proper indexing strategy

### 2. Schema Design Quality

**Finding:** Clean database schema with proper normalization.

#### Key Tables:

**Tasks Table:**
```sql
CREATE TABLE IF NOT EXISTS tasks (
    id              TEXT    PRIMARY KEY,
    core_task_id    INTEGER,
    model_id        TEXT,
    task_type       TEXT    NOT NULL,
    status          TEXT    NOT NULL,
    input_data      TEXT,
    result_data     TEXT,
    error_msg       TEXT,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_task_type ON tasks(task_type);
CREATE INDEX idx_tasks_model_id ON tasks(model_id);
```

**Sessions Table:**
```sql
CREATE TABLE IF NOT EXISTS chat_sessions (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL DEFAULT '',
    state_path  TEXT,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);
```

**Messages Table:**
```sql
CREATE TABLE IF NOT EXISTS chat_messages (
    id          TEXT    PRIMARY KEY,
    session_id  TEXT    NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role        TEXT    NOT NULL,
    content     TEXT    NOT NULL,
    created_at  TEXT    NOT NULL
);
CREATE INDEX idx_chat_messages_session ON chat_messages(session_id, created_at);
```

### 3. Data Storage Patterns

**Finding:** Appropriate use of JSON for semi-structured data.

#### JSON Storage Examples:
- `models.spec` - Model specification (complex nested structure)
- `models.runtime_presets` - Runtime configuration presets
- `tasks.input_data` / `tasks.result_data` - Task payload envelopes

**Severity Rating:** Low - JSON storage is appropriate for these use cases.

---

## Plugin API Design Assessment

### 1. JSON-RPC 2.0 Implementation

**Finding:** Standards-compliant JSON-RPC 2.0 implementation.

#### Protocol Structure:
```rust
struct JsonRpcRequest {
    jsonrpc: String,    // Must be "2.0"
    id: serde_json::Value,
    method: String,     // Format: "plugin_id.function_name"
    params: serde_json::Value,
}

struct JsonRpcResponse {
    jsonrpc: &'static str,  // "2.0"
    id: serde_json::Value,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}
```

#### Error Codes:
```rust
struct JsonRpcError {
    code: i64,
    message: String,
}
```

**Standard Error Codes Used:**
- `-32700` - Parse error
- `-32600` - Invalid Request (jsonrpc version)
- `-32601` - Method not found
- `-32000` - Server error (custom application errors)

**Severity Rating:** Low - Excellent JSON-RPC 2.0 compliance.

### 2. Plugin Dispatch Architecture

**Finding:** Clean separation between WebSocket gateway and plugin runtime.

#### WebSocket Routes:
- `GET /v1/plugins/rpc` - JSON-RPC 2.0 plugin dispatch
- `GET /v1/plugins/events` - Plugin UI event streaming

#### Dispatch Flow:
```
WebSocket → JsonRpcRequest → PluginService::dispatch_rpc()
                            → bin/slab-js-runtime (JS plugins)
                            → bin/slab-wasm-runtime (WASM plugins)
                            → Plugin WebView (UI plugins)
```

### 3. Plugin Schema Design

**Finding:** Comprehensive plugin response schema with proper state tracking.

#### PluginResponse Structure:
```rust
pub struct PluginResponse {
    // Identity
    pub id: String,
    pub name: String,
    pub version: String,
    
    // Validation
    pub valid: bool,
    pub error: Option<String>,
    pub manifest_version: u32,
    
    // Capabilities
    pub compatibility: Option<PluginCompatibilityManifest>,
    pub contributions: Option<PluginContributesManifest>,
    pub permissions: Option<PluginPermissionsManifest>,
    
    // Runtime
    pub runtime_status: String,
    pub last_error: Option<String>,
    
    // Lifecycle Timestamps
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
    pub last_started_at: Option<String>,
    pub last_stopped_at: Option<String>,
    
    // Updates
    pub available_version: Option<String>,
    pub update_available: bool,
    pub removable: bool,
}
```

---

## Interface Design Recommendations

### 1. REST API Improvements

#### Recommendation: Standardize Error Response Format
**Priority:** Low  
**Current:** OpenAI-compatible errors for `/v1/chat/completions`  
**Suggested:** Keep for compatibility, but document divergence clearly

#### Recommendation: Automate TypeScript Generation
**Priority:** Medium  
**Current:** Manual `bun run gen:api` required  
**Suggested:** Add pre-commit hook or CI check to detect drift

```bash
# Example pre-commit hook
#!/bin/bash
bun run gen:api
if git diff --quiet packages/api/src/v1.d.ts; then
    echo "TypeScript definitions are up to date"
else
    echo "ERROR: TypeScript definitions need regeneration"
    echo "Run: bun run gen:api"
    exit 1
fi
```

#### Recommendation: Consistent Service State Extraction
**Priority:** Low  
**Current:** Mixed `State<Service>` and `State<AppState>` patterns  
**Suggested:** Standardize on one pattern (prefer `State<AppState>` for future flexibility)

### 2. Schema Design Enhancements

#### Recommendation: Add Schema Version Tracking
**Priority:** Low  
**Suggested:** Add version field to major request/response schemas for future evolution

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionResponse {
    pub version: u8,  // Add version field
    pub id: String,
    pub name: String,
    // ...
}
```

#### Recommendation: Consolidate Response Wrapper Types
**Priority:** Low  
**Current:** Multiple similar response types (e.g., `DeleteSessionResponse`, `DeletedModelView`)  
**Suggested:** Consider generic `OperationResponse<T>` for simple cases

### 3. Cross-Boundary Type Safety

#### Recommendation: Type Generation CI Check
**Priority:** Medium  
**Suggested:** Add CI workflow that regenerates TypeScript and fails if changed

```yaml
# .github/workflows/api-type-check.yml
name: API Type Check
on: [pull_request]
jobs:
  check-types:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: bun install
      - run: bun run gen:api
      - run: git diff --exit-code packages/api/src/v1.d.ts
```

### 4. Database Schema

#### Recommendation: Add Schema Documentation
**Priority:** Low  
**Suggested:** Add ER diagram generation or schema documentation

#### Recommendation: Migration Testing
**Priority:** Medium  
**Suggested:** Add tests that verify migrations from previous versions

---

## Industry Best Practices Comparison

### REST API Design

#### ✅ Follows Best Practices:
- OpenAPI 3.0 documentation with utoipa
- Proper HTTP verb usage
- Consistent response formats
- Centralized validation framework
- Proper status code usage

#### ⚠️ Deviations from Best Practices:
- OpenAI-specific error format (intentional compatibility)
- Manual TypeScript generation step

### Schema-First Design

#### ✅ Follows Best Practices:
- Clear separation: Database → Domain Models → API Schemas
- Conversion traits at boundaries
- Centralized validation functions
- Proper serialization attributes

#### ⚠️ Areas for Improvement:
- No explicit schema versioning (implicit through migrations)
- TypeScript drift potential

### Type Safety

#### ✅ Follows Best Practices:
- Rust's type system for server-side
- Generated TypeScript definitions
- ISO-8601 timestamp strings at boundaries
- Proper option handling

#### ⚠️ Areas for Improvement:
- Manual regeneration step
- No runtime type checking for plugin payloads

---

## Conclusion

The slab-workspace API design demonstrates **strong engineering discipline** with consistent patterns, proper separation of concerns, and good use of the Rust type system. The dual API approach (HTTP + Tauri) is well-architected with shared schemas ensuring contract consistency.

### Overall Grade: B+

**Key Strengths:**
- Consistent REST API patterns across 17 v1 modules
- Standards-compliant JSON-RPC 2.0 plugin API
- Clean schema organization with centralized validation
- Append-only migration strategy
- Proper OpenAPI documentation

**Priority Improvements:**
1. **Medium:** Automate TypeScript type generation checks
2. **Low:** Document OpenAI error format divergence
3. **Low:** Consider schema versioning for major types

The codebase follows industry best practices for API design with room for automation improvements around type generation and drift detection.
