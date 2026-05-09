# Workspace Mode Design

## Product Goals

Workspace mode lets Slab open a local project folder as the current operating context. Opening a folder creates a `.slab` directory inside that folder and stores Slab workspace data there. The workspace settings file becomes the active settings source for the desktop sidecar, so workspace settings can override the user's global settings.

Plugins remain global while a workspace is active. Workspace mode uses the existing plugin page/runtime logic and the global plugin install directory; a workspace can only store a disabled override for a plugin.

Users enter workspace mode from:

- a folder selected inside Slab,
- an OS launch with a folder argument, including folder drag-to-app behavior,
- a recently opened workspace.

When no workspace is active, the workspace surface shows a welcome/recent-workspace state instead of an empty editor.

## Files

Each opened project owns:

```text
<project>/
  .slab/
    settings.json
    workspace.json
    slab.db
    sessions/
    models/
```

- `settings.json` is the normal Slab settings document used by `slab-server`.
- `workspace.json` stores workspace-only UI/runtime preferences that are not part of the global settings schema yet.
- `slab.db`, `sessions`, and `models` keep workspace state local when the sidecar starts in workspace mode.

## Workspace Config

`workspace.json` starts small:

```json
{
  "schemaVersion": 1,
  "plugins": {}
}
```

Plugin entries are keyed by plugin id:

```json
{
  "schemaVersion": 1,
  "plugins": {
    "video-subtitle-translator": {
      "enabled": false
    }
  }
}
```

The frontend writes this config when the user disables a plugin for the active workspace. Missing plugin entries mean the workspace inherits the global plugin configuration. Enabling a plugin in workspace mode removes the workspace disabled override; it does not change the global plugin install directory or global enablement state.

## Host Runtime

The Tauri host owns workspace discovery and file access because browser-side filesystem access would either be unavailable or too broad.

- Normalize and canonicalize workspace roots.
- Reject file/tree requests when no workspace is open.
- Resolve all relative paths against the workspace root and reject escapes.
- Hide `.slab`, `.git`, `node_modules`, `target`, `dist`, and build/cache folders from tree scans by default.
- Limit directory scan size and file read size.
- Return text files only; reject binary content.

When Slab is launched with a folder argument, the host creates `.slab` before starting `slab-server` and passes workspace paths as bootstrap args:

- `--settings-path <workspace>/.slab/settings.json`
- `--database-url sqlite:///<workspace>/.slab/slab.db?mode=rwc`
- `--model-config-dir <workspace>/.slab/models`
- `--session-state-dir <workspace>/.slab/sessions`
- `--plugins-dir <global plugin install directory>`

Opening a workspace after startup creates the same files and restarts the sidecar with these paths.

## Frontend

Workspace mode is a first-class route in the desktop shell:

- Welcome/recent state when no workspace is active.
- Folder open action.
- Virtualized file tree using `react-arborist`.
- File tabs for switching between opened files.
- Code preview using Monaco Editor in read-only mode.
- Plugin controls that persist workspace disabled overrides to `.slab/workspace.json`.

The tree is loaded lazily and bounded. File contents are requested only after selection, and large or binary files display an error instead of blocking the UI.

## Performance

- The host never recursively scans unbounded project trees.
- Directory listing skips heavy folders and caps per-directory entries.
- The frontend uses deferred selection state and virtualized rows.
- Monaco is loaded only on the workspace route.
- File reads are capped to 1 MiB.

## Security

- The renderer never receives unrestricted filesystem permissions.
- All workspace file operations go through custom Tauri commands that enforce canonical path containment.
- `.slab` internals are hidden from the file tree and cannot be opened through relative file reads.
- Workspace plugin config stores user intent only; plugin sandbox, CSP, and permission enforcement stay in the existing plugin host layers.
