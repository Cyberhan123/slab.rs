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
  - [Build Workflow Guide](#build-workflow-guide)
- [Slab Documentation](#slab-documentation)
- [Contributors](#contributors)
- [License](#license)

## Introduction

Slab is built for developers, researchers, creators, and teams who want to run AI workflows on their own machines. Think of it as a single entry point where you can download and manage models, start chats, process audio, generate images, work inside a built-in workspace, and track long-running jobs — local models by default, with the option to connect cloud providers when you need them.

## Why Choose Slab

- One app covers multiple AI workflows, so you do not need to jump between separate tools for chat, transcription, image generation, and model management.
- It is local-first: many tasks run directly on your device for privacy and offline use, while chat and completion can optionally route to cloud providers when you supply an API key.
- It is built for daily use, with a task queue for long jobs, centralized model management, a built-in workspace, and plugin-driven extensibility for add-on workflows.
- It runs as a desktop application, but the same application core also powers a headless HTTP host and a runtime worker, so it can fit into your broader tooling and workflows.

## Key Features

### Available Today

- **AI Chat**  
  Chat with local or cloud-connected models in one interface for writing help, Q&A, summarization, and everyday reasoning, with optional agent tooling for multi-step tasks such as file edits and shell commands, plus automatic context management.
- **Workspace**  
  A built-in workspace with a file explorer, code editor, language servers, and git so you can keep a project open right next to the assistant.
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
- **Multi-Runtime Plugin Backends**  
  Plugins can ship backend logic as JavaScript, Python, or WebAssembly, with frontend UIs hosted in sandboxed Tauri child WebViews.

## Project Structure

The tree below is a high-level view distilled from the current repository. It is meant to help you understand the role of each area in the product without diving into implementation detail.

```text
.
|-- bin/
|   |-- slab-app/                      Desktop host app and Tauri packaging
|   |-- slab-server/                   Local service entry for product APIs
|   |-- slab-runtime/                  Runtime worker for AI task execution
|   |-- slab-js-runtime/               Supervised JavaScript plugin runtime
|   |-- slab-python-runtime/           Supervised Python plugin runtime
|   |-- slab-mcp-server/               Model Context Protocol bridge server
|   `-- slab-windows-full-installer/   Windows full installer bootstrap
|-- crates/
|   |-- slab-app-core/                 Shared application logic
|   |-- slab-agent/                    Agent control-plane and orchestration kernel
|   |-- slab-agent-tools/              Built-in deterministic agent tools
|   |-- slab-cloud-provider/           Cloud model-provider routing (genai)
|   |-- slab-hub/                      Model hub abstraction
|   |-- slab-proto/                    Shared protocol definitions
|   |-- slab-runtime-core/             Backend worker substrate and admission core
|   |-- slab-types/                    Shared data contracts and settings types
|   `-- ...                            Engine bindings and supporting crates
|-- packages/
|   |-- slab-desktop/                  Desktop frontend application
|   |-- slab-components/               Shared UI component library
|   |-- slab-plugin-sdk/               Plugin author SDK package
|   |-- slab-i18n/                     Shared internationalization package
|   `-- ...                            API client, plugin CLI/UI, and test utilities
|-- docs/                              Documentation site and guides
|-- models/                            Model packaging scripts and assets
|-- plugins/                           Runtime plugin package workspace
|-- testdata/                          Sample media and test fixtures
`-- vendor/                            Vendored third-party runtime artifacts
```

- `packages/slab-desktop` is the desktop interface users interact with every day.
- `bin/slab-app`, `bin/slab-server`, and `bin/slab-runtime` together support the local app shell, task execution, and service entry points, while `bin/slab-js-runtime`, `bin/slab-python-runtime`, and `bin/slab-mcp-server` host supervised plugin runtimes and the MCP bridge.
- `crates/` contains the main shared capability layer for models, tasks, contracts, and reusable logic.
- `plugins/` contains runtime plugin packages. Manifest v1 declares runtime assets, extension contributions, permissions, and agent capabilities, while the host tracks install/runtime state separately.
- `docs/`, `models/`, `testdata/`, and `vendor/` support documentation, model packaging assets, sample data, and bundled runtime resources.

## Development Guide

This section keeps only the most common and practical development entry points. For deeper engineering details, see the project documentation.

### Install

- Install the Rust stable toolchain.
- Install Bun.

```sh
# From the repository root
bun install
```

### Development

Use these commands from the repository root for the most common day-to-day workflows.

```sh
# Start the main development stack (desktop host + sidecars + server/runtime)
bun run dev:app

# Start the desktop frontend only
bun run dev:desktop
```

### Build

These commands cover the usual build, check, and test workflows using Bun and Cargo.

```sh
# Run the standard workspace checks
bun run check
bun run check:rust
bun run lint:rust

# Run the standard automated test suite (frontend + Rust workspace)
bun run test

# Run targeted test suites
bun run test:frontend
bun run test:browser
bun run test:e2e

# Build the desktop frontend only
bun run build:desktop

# Build and stage desktop sidecars
bun run build:sidecars

# Build the desktop app binary without an installer bundle
bun run build:app

# Build the Windows full installer
bun run build:windows-installer

# Regenerate generated assets
bun run gen:api
bun run gen:schemas
bun run gen:plugin-packs
bun run gen:model-packs
```

### Build Workflow Guide

For build, generation, and vendor patch workflow details, see
[`docs/development/guides/build.md`](docs/development/guides/build.md).

## Slab Documentation

- Getting Started: https://slab.reorgix.com/guide/getting-started
- Documentation Home: https://slab.reorgix.com/

## Contributors

Issues, documentation improvements, feature ideas, and pull requests are all welcome. Contributions help make Slab a more practical local AI workspace.

- Contributor graph: https://github.com/Cyberhan123/slab.rs/graphs/contributors

## License

This project is licensed under the [GNU Affero General Public License v3.0](./LICENSE) (AGPL-3.0-only). Third-party materials in `testdata/` retain their original licenses.
