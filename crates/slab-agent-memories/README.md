# slab-agent-memories

Agent memory pipeline helpers for Slab.

## Role

`slab-agent-memories` owns reusable memory pipeline logic used by the app-core agent host:

- Phase 1 rollout filtering and memory candidate shaping.
- Phase 2 input selection and workspace summary rendering.
- Memory read prompts, citation parsing, and citation source classification.
- Agent memory hook helpers, templates, redaction, and memory workspace git diffs.

This crate does not own database scheduling, HTTP routes, UI projection, or model execution. Those integrations belong in `crates/slab-app-core`, `bin/slab-server`, and host layers.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-agent-memories
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
