# slab-parakeet

Safe Rust wrapper over the whisper.cpp `parakeet` speech-to-text (ASR) library.

## Role

`slab-parakeet` provides a safe, idiomatic Rust wrapper around the parakeet shared
library (`parakeet.dll` / `libparakeet.so`) exposed by
[`crates/slab-parakeet-sys`](../slab-parakeet-sys). It is the local ASR peer of
[`crates/slab-whisper`](../slab-whisper) and is used by `bin/slab-runtime` to handle
audio transcription requests dispatched to the `ggml.parakeet` backend.

Parakeet is **standalone** — it owns its own `parakeet_context` / `parakeet_state` and
does **not** depend on `slab-whisper` (no escape-hatch needed, unlike `slab-mtmd` which
borrows a `llama_context`). It depends on `slab-ggml` only (one-way), so there is no
cycle risk. The surface is a subset of whisper's: parakeet is greedy-only (no beam
search), has no language detection and no VAD, and its `ContextParams` exposes only
`use_gpu` / `gpu_device`.

## Type

Rust library crate (safe wrapper over a dynamically-loaded FFI library).

## Local validation

```sh
cargo check -p slab-parakeet
cargo clippy -p slab-parakeet --all-targets -- -D warnings
cargo test -p slab-parakeet --lib
```

## Hard boundaries

- Does **not** depend on `slab-runtime`, `slab-app-core`, or any HTTP/proto layer — it
  is a pure inference primitive.
- Depends on `slab-ggml` only (one-way: `slab-parakeet → slab-ggml`). No `slab-whisper`
  dependency.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
