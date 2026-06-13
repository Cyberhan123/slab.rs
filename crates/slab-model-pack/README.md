# slab-model-pack

Model pack manifest and archive parsing for Slab.

## Role

`slab-model-pack` owns strict parsing and schema generation for `.slab` model packs. It provides:

- `manifest.json` model pack types.
- Pack archive loading and referenced document validation.
- Variant, component, adapter, preset, artifact, and backend config resolution.
- Runtime bridge helpers for translating pack metadata into runtime load defaults.
- JSON Schema generation for model pack manifests.

This crate does not download model files, install packs, or launch runtimes. Download state and runtime supervision belong in `crates/slab-app-core`, while execution belongs behind `bin/slab-runtime`.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-model-pack
```

Regenerate the public manifest schema from the repo root after schema changes:

```sh
bun run gen:schemas
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
