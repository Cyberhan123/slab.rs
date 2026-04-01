# slab-llama

Safe Rust wrapper for llama.cpp language model inference.

## Role

`slab-llama` provides a safe, idiomatic Rust API over the llama.cpp native library. It is used by `crates/slab-core` to handle text generation and completion requests dispatched to the `ggml.llama` backend. The raw FFI bindings are provided by `crates/slab-llama-sys`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
