---
name: slab-tauri-app
description: Build or review slab-app/src-tauri code. Use for Tauri commands, capabilities, permissions, sidecar setup, plugin runtime, webview integration, and desktop security boundaries in this repository.
---

# Slab Tauri App

Use this skill for work in `slab-app/src-tauri`, including:

- Rust commands in `src-tauri/src/**`
- sidecar lifecycle in `src-tauri/src/setup.rs`
- capabilities and permission files
- plugin runtime and plugin webviews
- `tauri.conf.json` changes

## Repo Defaults

- The desktop app launches `slab-server` as a sidecar.
- Capabilities already include `main-window` and `plugin-webview`.
- CSP is explicitly configured and should be preserved or tightened by default.

## Workflow

1. Read the current implementation in `src-tauri/src/lib.rs`, `src-tauri/src/setup.rs`, and the relevant capability file.
2. If you need generic Tauri API guidance, open `../tauri-v2/SKILL.md`.
3. Preserve the current sidecar and permission model unless the user explicitly asks to change it.
4. Use owned types in async commands and `Result<T, E>` for fallible commands.
5. If frontend code invokes a command or plugin capability, confirm the matching permission exists.
6. Treat CSP, capabilities, shell execution, and plugin webviews as security-sensitive changes.

## Avoid

- Do not broaden capabilities casually.
- Do not assume browser-only APIs are safe in the desktop host.
- Do not hardcode paths when Tauri path APIs or existing setup helpers already handle them.
- Do not introduce a second app lifecycle path when the current sidecar setup already covers it.

## Useful Files

- `slab-app/src-tauri/tauri.conf.json`
- `slab-app/src-tauri/src/lib.rs`
- `slab-app/src-tauri/src/setup.rs`
- `slab-app/src-tauri/src/plugins/**`
- `slab-app/src-tauri/capabilities/default.json`
- `slab-app/src-tauri/capabilities/plugin-webview.json`

## Done When

- Command registration, permissions, and frontend usage all line up.
- Sidecar and plugin behavior still follows the current app model.
- Security boundaries remain explicit.
