# slab-jsonrpc

Shared JSON-RPC 2.0 envelope helpers for Slab.

## Role

`slab-jsonrpc` provides typed JSON-RPC 2.0 message models and host transport
helpers:

- A strongly typed `RequestId` for string and integer request ids.
- A `JSONRPCMessage` enum covering request, notification, response, and error
  payloads.
- Typed request, notification, response, and error structs that intentionally
  model the message body separately from the wire `jsonrpc: "2.0"` envelope.
- Optional request-level `W3cTraceContext` propagation through the `trace`
  field.
- A reusable sidecar host pipe in `slab_jsonrpc::host` for line-delimited
  JSON-RPC transports, bounded pending requests, request timeouts, outbound
  draining, and inbound dispatch through a caller-provided `RequestHandler`,
  with `serve_stdio` / `serve_uds` transport entry points for sidecar runtimes.

The host pipe owns transport mechanics only. Runtime process lifecycle, ready
payload content, authorization, plugin dispatch, and business routing belong in
the host crates that use this crate.

Valid JSON-RPC request ids are limited to strings and integers. Boolean, null,
object, array, and floating-point ids are rejected by the typed model.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-jsonrpc
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
