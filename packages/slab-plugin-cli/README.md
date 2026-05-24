# @slab/plugin-cli

Plugin author CLI for packaging Slab plugins.

## Role

`slab-plugin-cli` owns single-plugin packaging. It stages the runtime assets
declared by `plugin.json`, builds JS and Python backend bundles when needed,
writes package-only `integrity.filesSha256`, and emits a `.plugin.slab`
archive.

Repository-wide built-in plugin generation remains in
`scripts/plugins/generate-plugin-packs.ts`, which discovers `plugins/*` and
invokes this package for each plugin.

## Usage

```sh
slab-plugin-cli pack --plugin-dir ./my-plugin --out-dir ./dist
```

Python plugins can declare `python/requirements.txt`; pure-Python packages are
installed into the `.slabpy` bundle. Native extensions are rejected.
