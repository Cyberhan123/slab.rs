# slab-windows-sandbox

Windows OS-enforced sandbox for slab.rs — the platform implementation that sits beneath
[`slab-sandboxing`](../slab-sandboxing). It produces restricted-token, integrity-label-isolated
child processes via an elevated helper (`slab-sandbox-helper`), opt-in through UAC.

## Role

- Holds the Windows-specific isolation mechanism: Job Object tree-cleanup, restricted token
  (`CreateRestrictedToken` + Low integrity label), SACL mandatory-label ACEs, **AppContainer
  network isolation** (S3 — the child runs as an AppContainer without `internetClient`, so the OS
  default WFP rule blocks outbound traffic, plus a session-scoped user-mode WFP filter keyed on the
  package SID), and the elevated helper IPC (HMAC-signed payload/result files,
  `ShellExecuteExW("runas")`).
- Owns the `SpawnedChild` seam: returns a raw `tokio::process::Child` + `kill_tree` closure to
  `slab-sandboxing`, which feeds it into the **shared** `wait_for_child` output loop.
- **ConPTY (S6a, opt-in):** `conpty.rs` can spawn the elevated child under a Windows pseudoconsole
  (`CreatePseudoConsole` + `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` combined with the AppContainer
  `SECURITY_CAPABILITIES` attribute) for terminal-aware output (ANSI/TUI fidelity) instead of piped
  stdio. Default off (`windows_use_conpty` config knob); the piped path remains the safe default.
  The AppContainer+ConPTY combination is not a documented Win32 scenario and must be empirically
  validated via the gated `os_conpty_*` test before it is relied on; fail-closed on attach failure.

## Hard boundaries

- **MUST NOT depend on `slab-sandboxing`.** The dependency direction is one-way:
  `slab-sandboxing` depends downward (cfg-gated `cfg(target_os = "windows")`) on this crate.
  The cycle is broken by keeping every seam type (`SpawnRequest`, `SpawnedChild`,
  `CapabilitySnapshot`, `WindowsSandboxError`, …) defined here as decoupled mirrors.
- **Windows-only.** Every module is `#[cfg(target_os = "windows")]`-gated; on Linux/macOS the
  crate compiles to an empty shell so `cargo check --workspace` stays green in cross-OS CI.
- `wait_for_child` (the load-bearing output-capture/tree-kill loop) stays in `slab-sandboxing`
  and is never moved or forked into this crate.

## Local validation

```sh
cargo test -p slab-windows-sandbox
cargo clippy -p slab-windows-sandbox --all-targets -- -D warnings
```

OS-enforced isolation tests self-skip unless `SLAB_SANDBOX_ELEVATED=1` is set (mirrors
`slab-sandboxing`'s `SLAB_SANDBOX_SMOKE_ALLOW_SKIP=1` convention); the suite never requires an
admin shell.

## Status

Part of the slab permission + sandbox hardening mega-plan (Track S). **S2 (Low-IL restricted token +
ACL filesystem isolation), S3 (AppContainer + WFP network isolation), and S6a (opt-in ConPTY
pseudoconsole) are implemented.** See `~/.claude/plans/delightful-coalescing-diffie.md` (Track S),
the S2 sub-plan (`slab-mega-plan-snuggly-beacon.md`), the S3 sub-plan
(`slab-mega-plan-hashed-fountain.md`), and the S5/S6 plan
(`ultracode-slab-unified-crystal.md`).

Honest capability once provisioned: `filesystem=true (OsEnforced)`,
`network=true (OsEnforced)`, `setup_kind=ElevatedAclTokenWfp`, `isolation=Full`.
