# Slab App Run/Test Worklog (2026-02-27)

## Scope
- Re-scan current code from disk after user updates.
- Run and validate `slab-app` frontend + Tauri/Rust builds.
- Fix compile/type errors and API contract drift.
- Re-verify using the project package manager (`bun`).

## Environment
- Repo: `slab.rs`
- App: `slab-app`
- OS: Windows
- Bun: `1.2.21`

## What Was Executed
1. Frontend build:
   - `npm run build` (initial pass to surface current errors)
2. Rust checks/tests:
   - `cargo check -p slab-app`
   - `cargo test -p slab-app`
3. Tauri build validation:
   - `npm run tauri build -- --no-bundle` (initial)
4. Bun-aligned validation after user correction:
   - `bun run build`
   - `bun run tauri build --no-bundle`

## Main Problems Found
- TypeScript strict-mode failures (`noUnusedLocals`, wrong imports, mismatched symbols).
- API client drift:
  - `openapi-react-query` client no longer supports direct `api.get/post/delete` usage.
  - Error model expected fields/methods not implemented (`ApiError.fromResponse`, status handling, timeout/network classes).
- Generated API contract mismatch at call sites:
  - Invalid path/query/body combinations in Hub/Settings/Chat.
- Missing route export for chat page (`@/pages/chat`).
- `tauri.conf.json` build hooks still using `npm` while workflow is `bun`.

## Changes Implemented
- Cleaned strict TypeScript blockers:
  - Removed unused imports/symbols in diagnostics/theme/audio/chat components.
- API/error layer fixes:
  - Added `ApiError.status`.
  - Added `ApiError.fromResponse(...)`.
  - Added `NetworkError` and `TimeoutError`.
  - Fixed fetch URL extraction for `URL | Request`.
  - Exported raw fetch client as `apiFetch` for imperative API calls.
- Diagnostics fixes:
  - Removed unused generated type import.
  - Corrected diagnostic log call signatures to use valid log types.
- Tauri environment detection:
  - Added named `isTauri()` export and retained hook default export.
- Chat hook rewrite:
  - Migrated to generated contract via `apiFetch.GET/POST/DELETE`.
  - Kept non-stream + stream paths; improved SSE parsing safety.
- Route repair:
  - Added `src/pages/chat/index.tsx` to satisfy `@/pages/chat` import.
- Hub/settings contract fixes:
  - Removed invalid path params from model download/switch calls.
  - Corrected backend download mutation misuse (endpoint expects richer payload).
  - Added safe backend list typing from unknown response.
- Bun workflow alignment:
  - Updated `src-tauri/tauri.conf.json`:
    - `beforeDevCommand`: `bun run dev`
    - `beforeBuildCommand`: `bun run build`

## Verification Results
- `bun run build`: passed.
- `cargo check -p slab-app`: passed.
- `cargo test -p slab-app`: passed (currently 0 tests).
- `bun run tauri build --no-bundle`: passed.
  - Built app binary:
    - `target/release/slab-app.exe`

## Architecture Notes (Current State)
- Frontend:
  - React + React Router + React Query.
  - OpenAPI-generated typing from `src/lib/api/v1.d.ts` is source of truth.
- API layer:
  - Hook-based query/mutation via `openapi-react-query`.
  - Imperative typed calls now done via exported `apiFetch`.
- Tauri backend:
  - Commands in `src-tauri/src/lib.rs` expose API URL, backend health, and system info.

## Remaining Risks / Follow-up
- Frontend bundle size warning remains (`~761 KB` main chunk); needs code-splitting/manual chunks.
- Settings backend download UI is still incomplete for full request body (`owner/repo/tag/target_path`).
- Rust test coverage for `slab-app` is effectively absent (0 tests).

## Files Touched In This Round
- `slab-app/src/components/diagnostics-panel.tsx`
- `slab-app/src/components/theme-preview.tsx`
- `slab-app/src/hooks/use-tauri.ts`
- `slab-app/src/lib/api/client.ts`
- `slab-app/src/lib/api/diagnostics.ts`
- `slab-app/src/lib/api/errors.ts`
- `slab-app/src/lib/api/index.ts`
- `slab-app/src/pages/audio/hooks/use-transcribe.tsx`
- `slab-app/src/pages/chat/hooks/use-slab-chat.ts`
- `slab-app/src/pages/chat/slab-chat.tsx`
- `slab-app/src/pages/chat/index.tsx`
- `slab-app/src/pages/hub/index.tsx`
- `slab-app/src/pages/settings/index.tsx`
- `slab-app/src-tauri/tauri.conf.json`
