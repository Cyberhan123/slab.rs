# slab-ggml-sys

Unsafe FFI bindings to the ggml C library.

## Role

`slab-ggml-sys` is the `-sys` crate that links against the prebuilt ggml shared libraries (vendored in `testdata/`) and exposes raw C bindings. It is consumed exclusively by `crates/slab-ggml`, which provides the safe Rust wrapper.

## Type

Rust library crate (native bindings / FFI).

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
