# slab-config

Settings and launch configuration library for Slab.

## Role

`slab-config` owns the user settings document, PMID catalog, settings file loading and migration, typed settings views, host config defaults, and runtime launch resolution.

This crate is independent of HTTP, SQLx, Tauri, and app-core business services. Host crates may adapt its errors and migration results to their storage or transport layers.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
