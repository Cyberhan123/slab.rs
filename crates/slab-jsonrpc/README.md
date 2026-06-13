# slab-jsonrpc

Shared JSON-RPC 2.0 envelope helpers for Slab.

## Role

`slab-jsonrpc` provides small typed helpers for JSON-RPC 2.0 messages:

- Request, notification, response, and error payload builders.
- Incoming message parsing.
- Response serialization.
- Stable id key normalization.

Protocol transports, authorization, plugin dispatch, and runtime lifecycle management belong in the host crates that use these envelopes.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-jsonrpc
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
