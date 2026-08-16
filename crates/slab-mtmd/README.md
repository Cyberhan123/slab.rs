# slab-mtmd

Safe Rust wrapper around the llama.cpp multimodal (`mtmd`) library.

## Role

`slab-mtmd` loads `mtmd.dll` / `libmtmd.so` via libloading and exposes a safe API
for multimodal (vision) inference: loading a vision projector (`mmproj` GGUF)
bound to a llama text model, decoding image bytes into mtmd bitmaps, tokenizing
interleaved text+image prompts, and driving the combined text/image prefill
against a live llama context. It depends on [`slab-llama`](../slab-llama) for the
text model/context handles, which are passed across the `-sys` boundary as opaque
pointers (cast with a documented `// SAFETY:` rationale).

The safe API mirrors the `mtmd` module of the vendored `llama-cpp-rs-main`
reference. Video (ffmpeg-backed) helpers are out of scope.

## Type

Rust library crate (FFI wrapper).

## Hard boundaries

- Does **not** depend on `slab-runtime`, `slab-app-core`, or any HTTP/proto
  layer — it is a pure inference primitive.
- Depends on `slab-llama` (one-way: `slab-mtmd → slab-llama`). `slab-llama` must
  not depend on `slab-mtmd` (no cycle). Because the live `llama_context` lives on
  a worker thread, the runtime reaches it via slab-llama's
  `LlamaRuntime::run_with_context` escape-hatch + `MtmdContext::eval_chunks_raw`.

## Local validation

```sh
cargo test -p slab-mtmd --lib                 # offline bitmap / lib-load tests
cargo clippy -p slab-mtmd --all-targets -- -D warnings
```

Live multimodal verification (image → description round-trip) needs a multimodal
GGUF + matching `mmproj` and is manual — load a model pack with `mmproj_path`
set and POST an image to `/v1/chat/completions`.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
