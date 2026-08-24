# slab-cloud-provider

Cloud model-provider infrastructure that wraps the [`genai`](https://crates.io/crates/genai) crate.

## Role

Single owner of slab's cloud-provider knowledge:

- **Family → adapter mapping** — `family_to_adapter_kind` maps slab's `ProviderFamily` (in
  `slab-config`) to genai's `AdapterKind`, so each configured vendor drives its native protocol.
- **Credential resolution** — `resolve_api_key` resolves a provider's `api_key` / `api_key_env`,
  with a fallback to the adapter kind's canonical env var (e.g. `OPENAI_API_KEY`).
- **Default model catalog** — `default_models_for_provider` returns a curated flagship model list
  per `ProviderFamily`, used to activate cloud models as soon as a provider is configured. Returns
  plain `CloudModelSpec` data so callers own the DB writes. (genai 0.6.5's `Client::all_model_names`
  is a live web call that several vendors don't support, so it is not used as the activation source.)
  Zhipu's `Zai` and `BigModel` families share the same GLM catalog.
- **Live model discovery** — `list_remote_model_ids` probes an `OpenaiCompatible` provider's
  `GET {api_base}/models` endpoint via genai's `all_model_names` (Bearer auth, `{data:[{id}]}`
  shape). A live web call: callers own caching/timeouts, and a failed discovery must keep the
  previously known catalog.

## Boundaries

- Consumed by `slab-app-core`. **Must not depend on `slab-app-core`** (avoids a cyclic graph).
- Depends on `genai`, `slab-config`, `slab-types`.
- Does **not** own HTTP routing, model DB persistence, or settings application — those stay in
  `slab-app-core`. The genai chat-execution glue (request/response/stream mapping) is being moved
  here incrementally.

## Sync points

When `genai` adds an `AdapterKind` variant, add it in three places:

1. `ProviderFamily` (and `ProviderFamily::all_str()`) in `slab-config`.
2. `family_to_adapter_kind` in this crate.
3. The frontend kind metadata in `packages/slab-desktop/.../cloud-provider-kinds.ts`.

## Local validation

```sh
cargo test -p slab-cloud-provider
cargo clippy -p slab-cloud-provider -- -D warnings
```
