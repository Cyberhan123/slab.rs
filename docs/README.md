# @slab/docs

VitePress documentation workspace for Slab.

## Role

`docs` owns user-facing and development documentation. Generated public schemas and model metadata live under `docs/public/` and are refreshed by repo-root generation commands.

Keep repo-wide AI workflow rules in `AGENTS.md`; keep module ownership and validation notes in the nearest package or crate README.

## Type

Bun-managed documentation package.

## Commands

Run from the repo root:

```sh
bun run docs:dev
bun run docs:build
bun run docs:preview
```

Refresh generated schema assets before docs changes that depend on them:

```sh
bun run gen:schemas
```

## License

AGPL-3.0-only. See [LICENSE](../LICENSE).
