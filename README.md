# Slab

Slab is a local-first machine learning desktop application built with Rust and Tauri. It provides a unified interface for running language, speech, and image generation models entirely on-device, without requiring cloud connectivity.

> The project is under development and stability and compatibility are not guaranteed.

## Features

- Local language model inference (LLaMA-based) with chat and completion APIs
- Speech-to-text transcription via Whisper
- Image generation via diffusion models
- Plugin system for extending functionality with WebAssembly modules
- Tauri-based desktop shell with a React frontend
- HTTP API gateway (`/v1`) compatible with common LLM client libraries
- Task queue for managing long-running inference jobs
- Automatic model unloading to manage memory

## Architecture

```
bin/slab-app        Tauri desktop host; launches the server sidecar and mounts plugin webviews
bin/slab-server     HTTP gateway (axum); exposes /v1 routes and delegates to slab-app-core
bin/slab-runtime    gRPC worker process; composes GGML, Candle, and ONNX backends
crates/slab-app-core    HTTP-free business logic (domain, infra, context, config)
crates/slab-runtime-core Runtime orchestration, scheduler, and dispatch contracts
crates/slab-types       Shared semantic types and contract definitions
crates/slab-proto       Protobuf definitions for server/runtime IPC
```

See [AGENTS.md](./AGENTS.md) for the full workspace layout.

## Development

**Prerequisites**

- Rust (stable toolchain)
- LLVM (required by `bindgen` for native bindings)
- `cargo-make`: `cargo install cargo-make`
- Bun (for the frontend workspace)

**Build and run**

```sh
# Run the full development stack (server + frontend)
cargo make dev

# Build all Rust workspace members
cargo build --workspace

# Run Rust tests
cargo test --workspace

# Type-check the desktop frontend
cd packages/slab-desktop && bun run build
```

**Server compatibility tests**

```sh
pip install -r bin/slab-server/tests/requirements.txt
pytest bin/slab-server/tests
```

## License

Copyright (c) Cyberhan123 and contributors.

This project is licensed under the [GNU Affero General Public License v3.0](./LICENSE) (AGPL-3.0-only).

Third-party vendored libraries in `testdata/` retain their original licenses.
