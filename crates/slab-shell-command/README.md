# slab-shell-command

Policy-aware shell execution for Slab agent tools.

## Role

`slab-shell-command` owns command safety checks, execution policy resolution, optional sandbox-driver routing, timeout handling, and output truncation for shell tool calls.

It does not own host approval transport. Callers inspect the execution policy and route approval through `slab-agent` / host ports before executing sensitive commands.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-shell-command` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
