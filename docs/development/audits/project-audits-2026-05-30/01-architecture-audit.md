# Architecture & Layering Audit Report

**Audit Date:** 2026-05-30  
**Auditor:** Architecture & Layering Auditor  
**Project:** slab-workspace  
**Method:** Static analysis of workspace structure, dependency graphs, and layer boundaries

---

## Executive Summary

This audit examines the overall architecture of the slab-workspace project, focusing on the separation of concerns between layers, dependency flow, and adherence to stated architectural patterns.

**Key Findings:**
1. **Well-structured monorepo with clear separation** between Rust backend (crates/ + bin/) and TypeScript frontend (packages/)
2. **Hexagonal architecture properly implemented** in slab-app-core with clear domain/infra separation
3. **Dependency flow is generally correct** - no circular dependencies detected, upper layers appropriately depend on lower layers
4. **Some complexity concerns** - numerous specialized crates may benefit from consolidation
5. **Clear protocol boundaries** through slab-proto and slab-types for cross-crate contracts

**Severity Overview:**
- Critical: 0
- High: 2  
- Medium: 4
- Low: 3

---

## Architecture Overview

### Monorepo Structure

The project uses a hybrid monorepo approach with two separate package ecosystems:

**Rust Workspace (Cargo):**
- **bin/**: Entry-point binaries
  - `slab-app`: Tauri desktop application host
  - `slab-server`: HTTP API gateway
  - `slab-runtime`: AI inference worker process
  - `slab-js-runtime`: JavaScript plugin runtime sidecar
  - `slab-python-runtime`: Python plugin runtime sidecar
  - `slab-mcp-server`: MCP protocol server
  - `slab-windows-full-installer`: Windows installer builder

- **crates/**: Rust library crates
  - **Core Domain**: `slab-app-core`, `slab-runtime-core`, `slab-types`
  - **Protocol**: `slab-proto`
  - **Agent**: `slab-agent`, `slab-agent-tools`
  - **AI Capabilities**: `slab-llama`, `slab-whisper`, `slab-diffusion` (plus sys variants)
  - **Infrastructure**: `slab-config`, `slab-utils`, `slab-build-utils`
  - **Integration**: `slab-plugin`, `slab-mcp`, `slab-git`, `slab-file`, `slab-shell-command`
  - **Model Management**: `slab-hub`, `slab-model-pack`

**TypeScript Workspace (Bun):**
- **packages/**: npm packages
  - `slab-desktop`: React/Tauri desktop frontend
  - `slab-components`: Shared UI component library
  - `api`: TypeScript API client (generated from OpenAPI)
  - `slab-plugin-sdk`: Plugin author SDK
  - `slab-i18n`: Internationalization
  - `vitest-rust-reporter`: Test infrastructure

- **plugins/**: Runtime plugin packages
- **bin/slab-app/**: Desktop app frontend code
- **models/**: Model metadata and manifests

### Layer Hierarchy (Bottom-Up)

```
┌─────────────────────────────────────────────────────────────┐
│  Presentation Layer (TypeScript)                           │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ slab-desktop → slab-components → slab-i18n          │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            ↓ HTTP/WebSocket
┌─────────────────────────────────────────────────────────────┐
│  API Gateway Layer (Rust)                                   │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ slab-server (Axum HTTP)                               │  │
│  │ - Routes, middleware, error mapping                   │  │
│  │ - OpenAPI schema generation                           │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            ↓ delegates to
┌─────────────────────────────────────────────────────────────┐
│  Application Core Layer (Rust)                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ slab-app-core (HTTP-free business logic)             │  │
│  │ - domain/ (models, ports, services)                  │  │
│  │ - infra/ (db, rpc, runtime)                          │  │
│  │ - schemas/ (DTOs)                                    │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            ↓ gRPC/IPC
┌─────────────────────────────────────────────────────────────┐
│  Runtime Worker Layer (Rust)                               │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ slab-runtime (Backend composition root)              │  │
│  │ - GGML, Candle, ONNX backends                        │  │
│  │ - Uses slab-runtime-core for protocol               │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            ↓ uses
┌─────────────────────────────────────────────────────────────┐
│  Protocol & Types Layer (Rust)                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ slab-proto (Protobuf gRPC, OpenAPI models)            │  │
│  │ slab-types (Shared semantic types)                   │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Desktop Application Structure

```
┌─────────────────────────────────────────────────────────────┐
│  Desktop Host (Tauri)                                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ bin/slab-app (Rust native layer)                    │  │
│  │ - Tauri commands (host-only features)                │  │
│  │ - Plugin WebView management                         │  │
│  │ - slab-server process spawning                      │  │
│  └──────────────────────────────────────────────────────┘  │
│                          ↓                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ packages/slab-desktop (React frontend)               │  │
│  │ - Pages: chat, audio, images, models, plugins       │  │
│  │ - Uses @slab/api for type-safe HTTP calls            │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Layering Analysis

### 1. Hexagonal Architecture in slab-app-core

**Implementation Status: ✓ Properly Implemented**

The `slab-app-core` crate follows hexagonal (ports-and-adapters) architecture:

**Domain Layer** (`crates/slab-app-core/src/domain/`):
- **models/**: Pure domain entities (ChatSession, ModelConfig, Task, etc.)
- **ports/**: Abstract interfaces for external dependencies (RuntimeInferenceGateway)
- **services/**: Application business logic (ChatService, ModelService, etc.)

**Infrastructure Layer** (`crates/slab-app-core/src/infra/`):
- **db/**: SQLite repository implementations
- **rpc/**: gRPC gateway implementation
- **runtime/**: Runtime process supervision

**Finding:** This is a clean hexagonal implementation. The domain layer has no dependencies on infrastructure details, and services are defined against abstract ports.

### 2. Dependency Flow Analysis

**Directionality Check: ✓ Correct Flow**

```
slab-types (no internal deps)
  ↑
  │
slab-proto → slab-runtime-core → slab-runtime
  ↑                              ↑
  │                              │
slab-app-core ← slab-server ←───┘
  ↑
  │
slab-agent → slab-agent-tools
  ↑
  │
slab-plugin, slab-mcp, slab-git, slab-file, slab-shell-command
  ↑
  │
slab-config, slab-utils, slab-libfetch
```

**Key Points:**
- No circular dependencies detected
- Lower-level crates (types, proto) have no dependencies on higher-level crates
- slab-app-core correctly uses abstract ports instead of concrete implementations
- slab-server only adds HTTP concerns on top of slab-app-core

### 3. Separation of Concerns

**HTTP/Protocol Separation: ✓ Excellent**

**Evidence from AGENTS.md:**
```
- Keep inference behind: slab-app → slab-server → slab-app-core → runtime_supervisor → GrpcGateway → slab-runtime → slab-runtime-core
- Keep plugin dispatch behind: slab-app → slab-server /v1/plugins/rpc → slab-app-core
- Keep slab-app-core HTTP-free
- Keep slab-runtime as the only runtime composition root
- Keep slab-runtime-core limited to scheduler/backend protocol concerns
```

**Finding:** The architecture correctly separates HTTP concerns (slab-server) from business logic (slab-app-core) and runtime execution (slab-runtime).

**Desktop/Backend Separation: ✓ Good**

**Desktop Host (slab-app):**
- Owns Tauri commands and plugin WebView management
- Spawns and monitors slab-server process
- Does NOT implement business logic

**Backend Server (slab-server):**
- Implements HTTP/WebSocket API
- Delegates to slab-app-core
- Manages runtime sidecars

**Finding:** Clear separation between desktop host concerns and backend server concerns.

---

## Findings

### High Severity

**ARCH-H1: Excessive crate granularity may impact maintainability**

- **Description:** The workspace contains 30+ Rust crates with some potentially overlapping responsibilities (slab-utils, slab-build-utils, slab-config; slab-llama-sys vs slab-llama)
- **Severity:** High  
- **Evidence:** `Cargo.toml` workspace members list, multiple `-sys` variants
- **Impact:** More complex dependency graph, potential for duplicated abstractions
- **Recommendation:** 
  - Consolidate utility crates where clear boundaries don't exist
  - Consider merging slab-*utils crates into a single slab-utils with feature flags
  - Evaluate if -sys variants need separate crates or can be features

**ARCH-H2: Agent system complexity may be over-engineered**

- **Description:** The agent system spans multiple crates (slab-agent, slab-agent-tools, slab-shell-command, slab-file, slab-git, slab-mcp) with unclear boundaries
- **Severity:** High
- **Evidence:** `crates/slab-agent/README.md`, agent tool crates structure
- **Impact:** Difficult to understand agent control flow, potential for circular dependencies
- **Recommendation:** Document clear boundaries between agent orchestration and tool implementations, consider consolidating related tool crates

### Medium Severity

**ARCH-M1: Proto/type boundary could be clearer**

- **Description:** Both slab-proto and slab-types define similar contract types (chat messages, plugin manifests)
- **Severity:** Medium
- **Evidence:** `crates/slab-proto/src/lib.rs`, `crates/slab-types/src/lib.rs`
- **Impact:** Potential confusion about which to use for cross-boundary contracts
- **Recommendation:** Establish clear convention: slab-proto for gRPC/IPC, slab-types for shared domain concepts

**ARCH-M2: Frontend workspace has potential dependency confusion**

- **Description:** Multiple packages could have overlapping UI concerns (slab-components, slab-desktop, slab-plugin-ui)
- **Severity:** Medium  
- **Evidence:** `packages/` structure, slab-components exports
- **Impact:** Unclear which package owns which UI patterns
- **Recommendation:** Document clear ownership: slab-components for primitives, slab-desktop for app-specific composition

**ARCH-M3: Plugin architecture spans multiple concerns**

- **Description:** Plugin functionality split across slab-plugin, slab-app-core (plugin runtime), bin/slab-js-runtime, packages/slab-plugin-sdk
- **Severity:** Medium
- **Evidence:** Plugin-related crates and packages, AGENTS.md plugin constraints
- **Impact:** Difficult to trace plugin call flow
- **Recommendation:** Create plugin architecture diagram showing call flow across layers

**ARCH-M4: No clear architecture decision records**

- **Description:** While README files exist, there's no central document explaining why certain architectural decisions were made
- **Severity:** Medium
- **Evidence:** Absence of architecture decision records (ADRs)
- **Impact:** Future maintainers may not understand rationale behind layering
- **Recommendation:** Create ADRs for major decisions (hexagonal architecture, proto vs types, plugin isolation)

### Low Severity

**ARCH-L1: Inconsistent naming conventions**

- **Description:** Some crates use hyphens (slab-app-core), some use consistent patterns (slab-*-tools vs slab-*-command)
- **Severity:** Low
- **Evidence:** Workspace crate names
- **Impact:** Minor confusion when searching for crates
- **Recommendation:** Establish and document naming convention for crates

**ARCH-L2: Missing architecture diagrams**

- **Description:** Existing documentation is text-heavy; visual diagrams would improve understanding
- **Severity:** Low
- **Evidence:** docs/development/ structure
- **Impact:** Slower onboarding for new developers
- **Recommendation:** Add C4 or similar architecture diagrams to docs

**ARCH-L3: Some dead code potential**

- **Description:** Large number of crates suggests potential for unused code in specialized crates
- **Severity:** Low
- **Evidence:** Crate count vs. feature surface area
- **Impact:** Maintenance burden
- **Recommendation:** Audit for unused exports and consolidate underutilized crates

---

## Design Pattern Analysis

### Patterns Identified

**1. Hexagonal Architecture (Ports & Adapters)**
- **Location:** `crates/slab-app-core`
- **Implementation:** domain/ports define interfaces, infra/ provides implementations
- **Quality:** ✓ Well implemented, clean separation
- **Consistency:** Applied consistently across app-core

**2. Repository Pattern**
- **Location:** `crates/slab-app-core/src/infra/db/repository/`
- **Implementation:** Separate repository traits per domain (ChatStore, ModelStore, etc.)
- **Quality:** ✓ Good, follows domain boundaries
- **Consistency:** Consistent naming and structure

**3. Service Layer Pattern**
- **Location:** `crates/slab-app-core/src/domain/services/`
- **Implementation:** *Service per domain* (ChatService, ModelService, etc.)
- **Quality:** ✓ Good, clear responsibilities
- **Consistency:** Consistent across all domains

**4. Gateway Pattern**
- **Location:** `crates/slab-app-core/src/infra/rpc/gateway.rs`
- **Implementation:** GrpcGateway abstracts runtime communication
- **Quality:** ✓ Good, clean abstraction
- **Consistency:** Applied only where needed

**5. Process Supervisor Pattern**
- **Location:** `crates/slab-app-core/src/runtime_supervisor/`
- **Implementation:** RuntimeSupervisor manages runtime worker lifecycle
- **Quality:** ✓ Appropriate for the use case
- **Consistency:** N/A (single instance)

**6. Dependency Injection**
- **Location:** `crates/slab-app-core/src/context/`
- **Implementation:** AppState, ModelState, WorkerState as dependency containers
- **Quality:** ✓ Good manual DI
- **Consistency:** Applied consistently

### Pattern Consistency Assessment

**Overall: ✓ Patterns are applied consistently**

The codebase shows good discipline in applying the same patterns across similar domains:
- All services follow the same structure
- All repositories follow the same naming
- All DTOs follow the same organization

---

## Best Practices Comparison

### Industry Standards for Similar Projects

**Desktop Application with AI Backend:**

| Aspect | Industry Standard | Slab Implementation | Assessment |
|---|---|---|---|
| **Monorepo Structure** | Separate frontend/backend | ✓ Separate packages/ and crates/ | Good |
| **API Design** | OpenAPI/GraphQL | ✓ OpenAPI with type generation | Excellent |
| **Backend Arch** | Layered/Clean/Hexagonal | ✓ Hexagonal in app-core | Excellent |
| **Process Isolation** | Separate runtime processes | ✓ slab-runtime as separate process | Excellent |
| **Type Safety** | Shared types across boundary | ✓ slab-types, slab-proto | Excellent |
| **Frontend State** | Centralized state management | ✓ TanStack Query + Zustand | Good |
| **Testing** | Integrated test coverage | ✓ Test infrastructure in place | Good |
| **Documentation** | ADRs + API docs | ⚠ API docs present, no ADRs | Needs ADRs |
| **Dependency Management** | Minimal dependencies | ⚠ Many specialized crates | Could consolidate |

**Areas for Improvement:**
1. Architecture Decision Records (ADRs)
2. Crate consolidation opportunities
3. Architecture diagrams

---

## Actionable Recommendations

### Priority 1 (Next Sprint)

**1. Create Architecture Decision Records**
- Document why hexagonal architecture was chosen
- Document the slab-proto vs slab-types boundary rationale
- Document plugin isolation strategy
- Location: `docs/development/adr/`

**2. Consolidate Utility Crates**
- Merge slab-build-utils into slab-utils with feature flags
- Evaluate if slab-utils can be split more logically
- Target: Reduce workspace member count by 2-3

**3. Add Architecture Diagrams**
- Create C4-style diagrams for key components
- Document plugin call flow across layers
- Document runtime worker lifecycle
- Location: `docs/development/architecture/diagrams/`

### Priority 2 (Next Quarter)

**4. Clarify Agent System Boundaries**
- Document clear separation between agent orchestration and tool implementations
- Consider consolidating slab-file, slab-git, slab-shell-command into slab-agent-tools with feature modules
- Create agent architecture documentation

**5. Establish Naming Convention**
- Document crate naming rules
- Apply consistently across workspace
- Update naming in future crates

**6. Audit for Dead Code**
- Run cargo-udeps to identify unused dependencies
- Audit unused exports in specialized crates
- Consolidate underutilized crates

### Priority 3 (Ongoing)

**7. Maintain Layer Integrity**
- Enforce HTTP-free constraint in slab-app-core via CI
- Prevent direct HTTP dependencies in domain layer
- Keep runtime composition only in bin/slab-runtime

**8. Document Evolution**
- Keep ADRs updated for major changes
- Update architecture diagrams when layers change
- Maintain architectural principles in AGENTS.md

---

## Conclusion

The slab-workspace project demonstrates **strong architectural fundamentals** with:
- Clean hexagonal architecture implementation
- Proper dependency flow without circular dependencies  
- Clear separation between HTTP, business logic, and runtime concerns
- Good use of design patterns (Repository, Service, Gateway)

**Key Strengths:**
1. Well-structured monorepo with clear Rust/TypeScript separation
2. Properly implemented hexagonal architecture in slab-app-core
3. Clean protocol boundaries through slab-proto and slab-types
4. Appropriate process isolation for runtime workers

**Main Areas for Improvement:**
1. Excessive crate granularity could be consolidated
2. Missing architecture decision records
3. Need for visual architecture diagrams
4. Some boundaries (agent system, plugin architecture) could be clearer

**Overall Assessment:** The architecture is **well-designed and maintainable**, with room for improvement in documentation and consolidation of specialized crates. The foundational patterns are solid and consistently applied.

---

**Audit Completed:** 2026-05-30  
**Next Review:** After major architectural changes or 3 months
