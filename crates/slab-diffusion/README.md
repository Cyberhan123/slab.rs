# slab-diffusion

Rust wrapper for the stable-diffusion.cpp image generation backend.

## Role

`slab-diffusion` provides a safe Rust API over the native diffusion engine. It is used by `crates/slab-runtime-core` to handle image generation requests dispatched from `bin/slab-runtime`. The underlying native bindings are provided by `crates/slab-diffusion-sys`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
