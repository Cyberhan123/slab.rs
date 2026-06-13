# slab-candle

Candle-backed runtime engines for Slab model features.

## Role

`slab-candle` provides runtime-facing Candle implementations for:

- Text generation via `CandleLlmEngine`.
- Audio transcription via `CandleWhisperEngine`.
- Image generation via `CandleDiffusionEngine`.
- Shared device resolution and the `CandleRuntimeEngine` trait.

This crate defines typed load configs, requests, responses, and engine errors. It is not a runtime composition root and must not own HTTP, gRPC, task scheduling, model hub downloads, or desktop concerns. `bin/slab-runtime` and `crates/slab-runtime-core` own runtime orchestration.

## Features

- `cuda`: enables Candle CUDA support.
- `metal`: enables Candle Metal support.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-candle
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
