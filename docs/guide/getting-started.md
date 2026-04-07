---
title: Getting Started
outline: deep
---

# Getting Started

Slab is a local-first machine learning workspace built around a shared runtime core. It can run as a desktop application, as a headless HTTP host, or as a runtime worker depending on the surface you need.

## Choose a host

### Desktop

The desktop host is built with Tauri and React. It is the main user-facing surface for chat, image generation, audio tooling, settings, and plugin-driven workflows.

### Headless

The headless host exposes `/v1` HTTP APIs through `slab-server` while reusing the same application core and runtime supervision model as the desktop app.

## Core capabilities

- Chat and completion workflows backed by local or cloud-connected model routing.
- Speech-to-text with Whisper-class backends.
- Image generation pipelines driven by runtime workers.
- Task-oriented flows for longer-running inference jobs.
- Shared settings and manifest contracts across hosts and tooling.

## Public schemas

Slab publishes raw JSON Schemas from this docs site so external editors, validators, and manifests can use stable URLs.

- Model manifest schema: [`/reference/model-manifests`](/reference/model-manifests)
- Settings document schema: [`/reference/settings-document`](/reference/settings-document)

## For contributors

Internal planning, audits, engineering notes, and AI maintenance references live under [`/development/`](/development/). That section stays out of the ordinary-user navigation, but it remains available for contributors and repo maintenance work.
