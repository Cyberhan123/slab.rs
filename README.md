# Slab

A desktop tool based on machine learning, developed out of personal interest.

## development
1. install rust 
2. intall llvm for bindgen
2. install cargo-make: `cargo install cargo-make`
3. start dev : `cargo make dev`

## License

Copyright (c) Cyberhan123.

This repository is multi-licensed by component.

Apache-2.0:
- Repository root files (unless otherwise stated)
- `slab-app`
- `slab-proto`
- `slab-diffusion-sys`
- `slab-llama-sys`
- `slab-whisper-sys`

See [LICENSE](./LICENSE).

AGPL-3.0-only:
- `slab-core`
- `slab-core-macros`
- `slab-diffusion`
- `slab-libfetch`
- `slab-llama`
- `slab-runtime`
- `slab-server`
- `slab-whisper`

Each AGPL component contains its own `LICENSE` file in that directory.
