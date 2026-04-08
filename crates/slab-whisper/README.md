# slab-whisper

Safe Rust wrapper for whisper.cpp speech-to-text inference.

## Role

`slab-whisper` provides a safe, idiomatic Rust API over the whisper.cpp native library. It is used by `crates/slab-runtime-core` to handle audio transcription requests dispatched to the `ggml.whisper` backend. The raw FFI bindings are provided by `crates/slab-whisper-sys`.

It originated as a fork of [whisper-rs](https://codeberg.org/tazz4843/whisper-rs) with modifications to integrate with the Slab runtime architecture.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).

