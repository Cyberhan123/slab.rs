<div align="center">
  <a href="./README_zh.md">中文</a> / English
</div>
<br>

# Slab
Slab is a local-first AI desktop workspace that brings chat, speech transcription, image generation, video-related workflows, and model management into one app. It is designed around practical day-to-day use rather than exposing users to unnecessary technical complexity.

## Table of Contents

- [Introduction](#introduction)
- [Why Choose Slab](#why-choose-slab)
- [Key Features](#key-features)
- [Project Structure](#project-structure)
- [Development Guide](#development-guide)
  - [Install](#install)
  - [Development](#development)
  - [Build](#build)
- [Slab Documentation](#slab-documentation)
- [Contributors](#contributors)
- [License](#license)

## Introduction

Slab is built for developers, researchers, creators, and teams who want to run AI workflows on their own machines. Think of it as a single entry point where you can download and manage models, start chats, process audio, generate images, and track long-running jobs.

## Why Choose Slab

- One app covers multiple AI workflows, so you do not need to jump between separate tools for chat, transcription, image generation, and model management.
- It fits privacy-first, offline-friendly, and local-control workflows, with many tasks handled directly on your device.
- It is built for daily use, with a task queue for long jobs, centralized model management, and plugin-driven extensibility for add-on workflows.
- It works both as a desktop application and as a unified interface that can connect with your broader tooling and workflows.

## Key Features

### Available Today

- **AI Chat**  
  Chat with local models in one interface for writing help, Q&A, summarization, and everyday reasoning.
- **Audio Transcription**  
  Turn speech or audio into text for meeting notes, interview cleanup, lecture capture, and content archiving.
- **Image Generation**  
  Generate images locally for concept sketches, visual exploration, marketing experiments, and creative work.
- **Video-Related Workflows**  
  Keep video-related tasks inside the same workspace so they can sit alongside subtitles, audio, and other media workflows.
- **Model Hub**  
  Browse, download, switch, and manage models from one place instead of juggling scattered entry points.
- **Task Queue**  
  Long-running jobs can be queued and tracked in the background without interrupting the rest of your work.
- **Practical Hardware Compatibility**  
  Windows is currently the most complete path: in the full installer, Slab uses `ggml` plus our packaged runtime layer to choose the most suitable local variant during setup, so NVIDIA systems prefer CUDA, AMD systems prefer HIP, and when dedicated GPU paths are unavailable the bundled base runtime still ships with Vulkan and CPU backends. For macOS, Slab also targets `ggml`-based local acceleration paths on Apple Silicon so local inference can take advantage of the platform's native acceleration stack. Linux is likely supportable as well, and the repository already includes Linux target artifacts, but Linux compatibility is not fully adapted or validated on the maintainer side yet. If you are interested in helping push Linux support forward, contributions are very welcome.
- **Unified Settings**  
  Manage runtime preferences, model choices, and app settings in one place to reduce day-to-day setup friction.

### Plugin Extensibility

- **Plugin Lifecycle Management**  
  Desktop builds manage installed plugins while keeping `plugin.json` as the static source of truth for runtime assets, permissions, and contribution points.

## Project Structure

The tree below is a high-level view distilled from the current repository. It is meant to help you understand the role of each area in the product without diving into implementation detail.

```text
.
|-- bin/
|   |-- slab-app/                      Desktop host app and Tauri packaging
|   |-- slab-server/                   Local service entry for product APIs
|   |-- slab-runtime/                  Runtime worker for AI task execution
|   `-- slab-windows-full-installer/   Windows full installer bootstrap
|-- crates/
|   |-- slab-app-core/                 Shared application logic
|   |-- slab-agent/                    Agent control-plane and orchestration kernel
|   |-- slab-agent-tools/              Built-in deterministic agent tools
|   |-- slab-hub/                      Model hub abstraction
|   |-- slab-proto/                    Shared protocol definitions
|   |-- slab-runtime-core/             Backend worker substrate and admission core
|   |-- slab-types/                    Shared data contracts and settings types
|   `-- ...                            Engine bindings and supporting crates
|-- packages/
|   |-- slab-desktop/                  Desktop frontend application
|   |-- slab-components/               Shared UI component library
|   |-- slab-plugin-sdk/              Plugin author SDK package
|   `-- slab-i18n/                     Shared internationalization package
|-- docs/                              Documentation site and guides
|-- models/                            Model packaging scripts and assets
|-- plugins/                           Runtime plugin package workspace
|-- testdata/                          Sample media and test fixtures
`-- vendor/                            Vendored third-party runtime artifacts
```

- `packages/slab-desktop` is the desktop interface users interact with every day.
- `bin/slab-app`, `bin/slab-server`, and `bin/slab-runtime` together support the local app shell, task execution, and service entry points.
- `crates/` contains the main shared capability layer for models, tasks, contracts, and reusable logic.
- `plugins/` contains runtime plugin packages. Manifest v1 declares runtime assets, extension contributions, permissions, and agent capabilities, while the host tracks install/runtime state separately.
- `docs/`, `models/`, `testdata/`, and `vendor/` support documentation, model packaging assets, sample data, and bundled runtime resources.

## Development Guide

This section keeps only the most common and practical development entry points. For deeper engineering details, see the project documentation.

### Install

- Install the Rust stable toolchain.
- Install `cargo-make`: `cargo install cargo-make`
- Install Bun.
- Install Python as well if you plan to run the server compatibility tests.

```sh
# From the repository root
bun install
```

### Development

Use these commands from the repository root for the most common day-to-day workflows.

```sh
# Start the main development stack
cargo make dev

# Type-check the desktop frontend
cd packages/slab-desktop
bun run build
```

### Build

These commands cover the usual build, check, and test workflows.

```sh
# Rust workspace
cargo build --workspace
cargo test --workspace
cargo check --workspace

# Focused checks
cargo check -p slab-server
cargo check -p slab-runtime
cargo check -p slab-windows-full-installer

# Desktop frontend
cd packages/slab-desktop
bun run build

# Windows full installer
cd ../..
cargo make build-windows-full-installer

# Server compatibility tests
python -m pip install -r bin/slab-server/tests/requirements.txt
pytest bin/slab-server/tests
```

## Slab Documentation

- Getting Started: https://slab.reorgix.com/guide/getting-started
- Documentation Home: https://slab.reorgix.com/

## Contributors

Issues, documentation improvements, feature ideas, and pull requests are all welcome. Contributions help make Slab a more practical local AI workspace.

- Contributor graph: https://github.com/Cyberhan123/slab.rs/graphs/contributors

## License

This project is licensed under the [GNU Affero General Public License v3.0](./LICENSE) (AGPL-3.0-only). Third-party materials in `testdata/` retain their original licenses.
