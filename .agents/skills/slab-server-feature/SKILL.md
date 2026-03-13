---
name: slab-server-feature
description: Add or refactor HTTP gateway features in slab-server. Use for API routes, schemas, domain services, app state wiring, persistence, OpenAPI updates, and gateway-side orchestration in this repository.
---

# Slab Server Feature

Use this skill for `slab-server` feature work, especially:

- `slab-server/src/api/**`
- `slab-server/src/domain/**`
- `slab-server/src/context/**`
- `slab-server/src/infra/**`
- migrations under `slab-server/migrations`

## Repo Defaults

- Server layering is `config`, `context`, `api`, `domain`, and `infra`.
- Extend the existing `/v1` route tree rather than creating parallel API namespaces.
- AI inference must stay behind `GrpcGateway -> slab-runtime -> slab-core`.
- Long-running AI work should prefer task-based flows.

## Workflow

1. Start from the nearest existing route module in `src/api/v1`.
2. Keep request and response schema changes near the route module.
3. Put business logic in `src/domain/services`.
4. Reuse `AppState`, `AppContext`, and existing service wiring in `src/context`.
5. Keep infrastructure concerns in `src/infra`.
6. Follow typed error patterns from `slab-server/src/error.rs`.
7. If persistence changes are needed, add a new migration; never modify an old migration.
8. If the feature touches runtime orchestration or async runtime internals, also open `../slab-runtime-async/SKILL.md`.

## Avoid

- Do not bypass the gRPC runtime boundary.
- Do not add HTTP or SQL concerns to `slab-core` or `slab-runtime`.
- Do not create a second service layer or a parallel routing tree.
- Do not add ad hoc status endpoints when task entities already fit the workflow.

## Useful Files

- `slab-server/src/api/v1/mod.rs`
- `slab-server/src/context/mod.rs`
- `slab-server/src/domain/services/mod.rs`
- `slab-server/src/error.rs`
- `slab-server/src/infra/rpc/gateway.rs`
- `slab-server/migrations/**`

## Done When

- Route, schema, service, state, and infra changes align with current layering.
- OpenAPI-facing routes stay inside the existing `v1` structure.
- Runtime calls still flow through the supervisor and gateway model.
