# slab-ggml

Safe Rust wrapper for the ggml tensor library.

## Role

`slab-ggml` provides a safe Rust API over the ggml C library, which underlies all GGUF-format model inference in Slab. It is used by higher-level engine crates (`slab-llama`, `slab-whisper`) and by `crates/slab-core`. The raw bindings are provided by `crates/slab-ggml-sys`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
