# slab-windows-full-installer

Windows-only full-installer bootstrap for Slab.

## Role

`slab-windows-full-installer` is the outer installer binary that wraps the desktop NSIS installer with runtime payload staging logic.

- Packs a self-extracting installer executable around the resource-less Tauri NSIS `setup.exe` plus CAB payloads.
- Stages runtime payload archives and a payload manifest for distribution.
- Detects the best runtime variant for the current Windows machine.
- Expands staged payloads into a temporary area and lets NSIS complete the install.
- Applies selected runtime payloads into the installed app layout through its helper mode.

## CLI Surface

- `pack`: build a bundled full installer executable.
- `stage-payloads`: stage CAB payloads and the payload manifest without producing the outer bootstrap.
- `run`: execute the bundled bootstrap flow.
- `apply`: copy a selected runtime payload into the installed app layout.
- `detect-gpu`: print the best detected runtime variant.

## Type

Rust binary crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).