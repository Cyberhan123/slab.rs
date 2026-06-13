# @slab/models

Model catalog and pack source workspace for Slab.

## Role

`models` contains source model-pack metadata grouped by model family. The root model packaging script reads these sources and emits generated model pack artifacts under `models/dist/`.

This package does not download model weights or launch inference runtimes. Download state belongs in `crates/slab-app-core`; runtime execution belongs behind `bin/slab-runtime`.

## Type

Bun-managed data package.

## Commands

Run from the repo root:

```sh
bun run gen:model-packs
```

Package-local commands are also available:

```sh
bun run --cwd models pack
bun run --cwd models lint
```

## License

AGPL-3.0-only. See [LICENSE](../LICENSE).
