# slab-parakeet-sys

Unsafe FFI bindings to the whisper.cpp `parakeet` native (speech-to-text) library.

## Role

`slab-parakeet-sys` is the `-sys` crate that bindgen-generates raw C bindings for
the parakeet shared library (`parakeet.dll` / `libparakeet.so`) and exposes them
behind a `ParakeetLib` libloading handle. It is consumed exclusively by
[`crates/slab-parakeet`](../slab-parakeet), which provides the safe Rust wrapper.

parakeet ships **inside** the whisper SDK (`vendor/whisper/{include,bin}`), so
this crate reuses the `"whisper"` primary artifact (+ `ggml` dep) in its
`build.rs`. The bindgen allowlist is restricted to `parakeet_*` / `PARAKEET_*`
symbols, so the ggml types the parakeet API references (`ggml_abort_callback`,
`ggml_log_callback`, …) surface as opaque types **local to this crate** — they do
not clash with `slab-whisper-sys` or `slab-ggml-sys` (separate crates).

## Type

Rust library crate (native bindings / FFI, dynamic libloading — no `#[link]`).

## Local validation

```sh
cargo check -p slab-parakeet-sys
```

Bindings regenerate at build time into `OUT_DIR/bindings.rs` (no checked-in
fallback). The runtime DLL is synced to `bin/slab-app/src-tauri/resources/libs/`
by the whisper artifact's build step.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
