# slab-sandbox-helper

Elevated Windows helper binary for `slab-windows-sandbox`. Launched elevated (UAC `runas`) by
the orchestrator (slab-server) to apply OS-enforced sandbox isolation that a non-elevated process
cannot grant itself — specifically spawning each sandboxed child under a Low-integrity restricted
token assigned to a Job Object, and applying the SACL integrity-label ACLs.

## Modes

- `slab-sandbox-helper payload <path>` — one-shot: read a signed payload file, perform the op
  (S2a: Provision — write marker + honest result; S2b: apply ACLs + token), write the signed
  result file, exit 0. Non-zero exit ⇒ the orchestrator fails closed (no stale rules).
- `slab-sandbox-helper serve --pipe <name>` — (S2b) the long-lived elevated daemon serving a
  named pipe for Spawn/Kill/Ping RPCs. Not implemented in earlier slices.
- `slab-sandbox-helper version` — print version.

## IPC

Payload + result files are HMAC-SHA256-signed (`ring::hmac`) with a DPAPI-sealed per-user key at
`<app_home>/sandbox-helper.key` (shared between the non-elevated orchestrator and this elevated
helper — same-user DPAPI unprotects across integrity levels). See
[`crates/slab-windows-sandbox`](../../crates/slab-windows-sandbox) for the wire types + framing.

## Out of scope for this slice

- Piped-stdio only — no ConPTY (interactive TUI tools won't work) until S6.
- Network isolation (WFP/firewall) is S3; Low-IL does not block sockets.

## Build

Built + staged alongside the other Tauri sidecars via `bun run build:sidecars` (it is declared
under `bundle.resources`, not `externalBin`, because slab-server launches it directly by absolute
path rather than through the Tauri shell plugin).

Windows-only. On Linux/macOS this binary prints a no-op message and exits (it exists as a
workspace member so `cargo check --workspace` stays green in cross-OS CI).
