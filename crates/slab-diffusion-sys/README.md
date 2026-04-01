# slab-diffusion-sys

Unsafe FFI bindings to the stable-diffusion.cpp native library.

## Role

`slab-diffusion-sys` is the `-sys` crate that links against the prebuilt diffusion native library and exposes raw C bindings. It is consumed exclusively by `crates/slab-diffusion`, which provides the safe Rust wrapper.

## Type

Rust library crate (native bindings / FFI).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
