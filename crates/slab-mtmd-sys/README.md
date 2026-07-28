# slab-mtmd-sys

Unsafe FFI bindings to the llama.cpp multimodal (`mtmd`) native library.

## Role

`slab-mtmd-sys` is the `-sys` crate that bindgen-generates raw C bindings for the
mtmd shared library (`mtmd.dll` / `libmtmd.so`) and exposes them behind a
`MtmdLib` libloading handle. It is consumed exclusively by
[`crates/slab-mtmd`](../slab-mtmd), which provides the safe Rust wrapper.

mtmd ships **inside** the llama SDK (`vendor/llama/{include,bin}`), so this crate
reuses the `"llama"` primary artifact (+ `ggml` dep) in its `build.rs`. The
bindgen allowlist is restricted to `mtmd_*` / `MTMD_*` symbols, so the
llama/ggml types the mtmd API references (`llama_model`, `llama_context`, …)
surface as opaque types **local to this crate** — they do not clash with
`slab-llama-sys` (separate crates). The safe wrapper casts slab-llama's concrete
pointers across the boundary at FFI call sites.

## Type

Rust library crate (native bindings / FFI, dynamic libloading — no `#[link]`).

## Local validation

```sh
cargo check -p slab-mtmd-sys
```

Bindings regenerate at build time into `OUT_DIR/bindings.rs` (no checked-in
fallback). The runtime DLL is synced to `bin/slab-app/src-tauri/resources/libs/`
by the llama artifact's build step.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
