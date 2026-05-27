# slab-shell-command

Policy-aware shell execution for Slab agent tools.

## Role

`slab-shell-command` owns command safety checks, execution policy resolution, optional sandbox-driver routing, timeout handling, and output truncation for shell tool calls.

It does not own host approval transport. Callers inspect the execution policy and route approval through `slab-agent` / host ports before executing sensitive commands.

## Exec Rules

Shell execution can be refined with `.rule` files. The desktop/server wiring loads these from the global Slab rules directory, which defaults to the `rules` directory beside the global `settings.json`.

Rule files are evaluated by file name and then line number. Blank lines and lines starting with `#` are ignored. The first matching rule wins:

```txt
allow prefix cargo check
require_approval prefix cargo test
block contains Remove-Item
```

Each rule is `<action> <matcher> <pattern>`. Actions are `allow`, `require_approval`, and `block`; matchers are `exact`, `prefix`, and `contains`. Built-in dangerous-command checks still hard-block commands even when a rule allows them.

`prefix` rules require a token boundary after the pattern and do not match chained shell segments, so `allow prefix cargo check` matches `cargo check -p slab-agent` but not `cargo checkout` or `cargo check && ...`.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-shell-command` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
