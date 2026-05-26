# Backend + Frontend Merged Audit Report — slab.rs

**Audit Date:** 2026-05-26  
**Scope:** Rust backend + TypeScript/React frontend  
**Merged From:** `backend_audit-2026-05-26.md` + `frontend_audit-2026-05-26.md`  
**Method:** Merge + repository fact verification against current `main`

---

## Executive Summary

This document merges the previous backend/frontend audit reports and keeps findings that were re-verified against the current repository state.

- Verified high/medium-priority findings: 12
- Confirmed: 11
- Partially confirmed: 1
- Not confirmed in verified set: 0

Residual note: this is a fact-verified merged report, not a full re-audit of every historical low-priority item.

---

## Verification Matrix

| ID | Area | Status | Fact Check Result |
|---|---|---|---|
| SEC-C1 | Backend | Partially confirmed | Auth middleware is applied only in backend/settings routers; broad unauthenticated surface claim is directionally correct, but impact wording in old report is broader than code-level proof.
| SEC-C2 | Backend | Confirmed | Bearer token comparison uses `==` in auth middleware.
| SEC-C3 | Backend | Confirmed | `WebOptions::default()` uses `DefaultWebPermissions`; default implementation allows operations broadly.
| SEC-M3 | Backend | Confirmed | `/v1/plugins/rpc` websocket route has no auth middleware.
| ARCH-M2 | Backend | Confirmed | `append_message` performs two SQL statements without a transaction.
| INFRA-C1 | Infra | Confirmed | CI has no `cargo audit` or `cargo-deny` step.
| INFRA-C2 | Infra | Confirmed | Artifact checksum fields in `vendor/slab-artifacts.toml` are commented placeholders.
| FE-SEC-C1 | Frontend | Confirmed | Plugin SDK exposes raw `host.invoke(command, args)`.
| FE-SEC-C2 | Frontend | Confirmed | `withGlobalTauri` is `true` in Tauri config.
| FE-SEC-M1 | Frontend | Confirmed | Workspace markdown preview renders HTML via `dangerouslySetInnerHTML`.
| FE-ARCH-M1 | Frontend | Confirmed | Query client is created as `new QueryClient({})` with no defaults.
| FE-SEC-m1 | Frontend | Confirmed | API middleware `onError` returns error objects instead of throwing.

---

## Confirmed Findings (Merged)

### Critical

1. **SEC-C2: Non-constant-time token comparison**  
   - Evidence: `bin/slab-server/src/api/middleware/auth.rs`  
   - Current behavior: `provided.map(|p| p == expected).unwrap_or(false)`.

2. **SEC-C3: JS runtime default permissions are permissive**  
   - Evidence: `bin/slab-js-runtime/src/infra/deno/ext/web/options.rs`, `bin/slab-js-runtime/src/infra/deno/ext/web/permissions.rs`  
   - Current behavior: `WebOptions::default()` uses `DefaultWebPermissions`, and default checks return allow.

3. **FE-SEC-C1: Plugin SDK raw host invoke escape hatch**  
   - Evidence: `packages/slab-plugin-sdk/src/index.ts`  
   - Current behavior: public API includes `invoke<T>(command: string, args?: unknown)`.

4. **FE-SEC-C2: Global Tauri API exposed**  
   - Evidence: `bin/slab-app/src-tauri/tauri.conf.json`  
   - Current behavior: `"withGlobalTauri": true`.

### High / Major

1. **SEC-C1: Auth coverage is narrow (partially confirmed)**  
   - Evidence: `bin/slab-server/src/api/v1/backend/handler.rs`, `bin/slab-server/src/api/v1/settings/handler.rs`, `bin/slab-server/src/api/v1/mod.rs`  
   - Current behavior: `auth_middleware` is only wired in backend/settings routers.

2. **SEC-M3: Plugin RPC websocket has no auth middleware**  
   - Evidence: `bin/slab-server/src/api/v1/plugins/handler.rs`.

3. **ARCH-M2: Chat append uses no transaction**  
   - Evidence: `crates/slab-app-core/src/infra/db/repository/chat.rs`.

4. **INFRA-C1: No dependency audit job in CI**  
   - Evidence: `.github/workflows/ci.yml`.

5. **INFRA-C2: Artifact checksum verification not enforced**  
   - Evidence: `vendor/slab-artifacts.toml`.

6. **FE-SEC-M1: Markdown preview injects generated HTML directly**  
   - Evidence: `packages/slab-desktop/src/pages/workspace/components/workspace-markdown-preview.tsx`.

7. **FE-ARCH-M1: No QueryClient defaults**  
   - Evidence: `packages/slab-desktop/src/lib/query-client.ts`.

8. **FE-SEC-m1: API middleware onError returns value instead of throwing**  
   - Evidence: `packages/api/src/errors.ts`.

---

## Prioritized Action Plan

### P0 (before next release)

1. Add auth protection for high-risk route groups (`/v1/plugins/*`, `/v1/agents/*`, `/v1/system/*`, `/v1/tasks/*`) when admin token is configured.
2. Replace token equality check with constant-time comparison.
3. Remove or strictly gate raw `host.invoke` from plugin SDK public surface.
4. Set `withGlobalTauri` to `false` unless a documented compatibility reason requires otherwise.

### P1 (next sprint)

1. Add CI dependency vulnerability checks (`cargo-deny` or `cargo-audit`).
2. Enforce artifact checksum verification in `vendor/slab-artifacts.toml` + download pipeline.
3. Add sanitization hardening for markdown preview output path.
4. Add transaction around multi-statement chat append flow.

### P2 (next quarter)

1. Define shared QueryClient defaults for consistent caching/retry behavior.
2. Align API fetch error middleware behavior with desired throw/propagation contract.

---

## Notes on Historical Findings

The original backend/frontend reports include additional lower-priority items. They are intentionally not copied verbatim here unless re-verified in this merge pass. Use git history to inspect the original two source files if needed.
