# slab-jsonrpc

Shared JSON-RPC 2.0 envelope helpers for Slab.

## Role

`slab-jsonrpc` provides small typed helpers for JSON-RPC 2.0 messages:

- Request, notification, response, and error payload builders.
- Incoming message parsing.
- Response serialization.
- Stable id key normalization.
- A reusable sidecar host pipe in `slab_jsonrpc::host` for line-delimited
  JSON-RPC transports, bounded pending requests, request timeouts, outbound
  draining, and inbound dispatch through a caller-provided `RequestHandler`.

The host pipe owns transport mechanics only. Runtime process lifecycle, ready
payload content, authorization, plugin dispatch, and business routing belong in
the host crates that use this crate.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-jsonrpc
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
