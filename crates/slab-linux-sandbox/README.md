# slab-linux-sandbox

Linux OS-enforced sandbox primitives for slab.rs, sitting beneath [`slab-sandboxing`](../slab-sandboxing).

## Role

Owns the Linux isolation stack that `slab-sandboxing::platform::linux` delegates to via the
`SpawnedChild` seam (mirrors [`slab-windows-sandbox`](../slab-windows-sandbox)):

- **`bwrap`** (bubblewrap) — the primary filesystem mechanism: builds a namespace view via
  read-only / bind mounts and `--unshare-net`. Real OS-enforced filesystem isolation
  (`IsolationStrength::OsEnforced`, `SetupKind::Bwrap`/`BwrapSeccomp`).
- **seccomp** — a network-only BPF filter, always stacked under the network predicate
  (`network == Blocked && no managed proxy`). Blocks network syscalls (`socket`/`connect`/`bind`/
  …) except `socket(AF_UNIX, …)`, installed via `PR_SET_NO_NEW_PRIVS` + `SECCOMP_SET_MODE_FILTER`
  in the child's `pre_exec` hook so it inherits across `execve`. `SECCOMP_RET_KILL_PROCESS`.
- **landlock** — the filesystem fallback **only when `bwrap` is unavailable** (containers without
  user namespaces), gated by `linux_allow_landlock_fallback`. Path-access control via a deny-default
  ruleset; never stacked with `bwrap` (both control the FS dimension).

`SetupKind::{BwrapSeccomp, BwrapLandlock}` and `SandboxIsolation::KernelFiltered` get their first
constructions here.

## Layering / cycle rule

`slab-sandboxing` depends on this crate (cfg-gated, downward only). **This crate MUST NOT depend on
`slab-sandboxing`.** The `SpawnedChild { child, kill_tree }` seam is the boundary; the shared
`wait_for_child` / `unix_kill_tree` / `command_env` stay in `slab-sandboxing` and are never forked
here.

## Fail-closed

seccomp compile failure, or landlock ABI < 1 when landlock is the opted-in fallback and `bwrap` is
absent, ⇒ no spawn ⇒ the driver reports `degraded`/`unavailable` ⇒ `available_sandbox_driver` blocks
the shell. Never silently degrade.

## Honest-capability notes

- The network dimension is reported `OsEnforced` only when the network predicate holds (no managed
  proxy). A managed proxy needs outbound, so the filter is skipped and the network dimension is
  honestly `Lexical` — this fixes the long-standing dishonesty in the pre-S4 bwrap driver.
- Under landlock, `.git`/`.slab`/`.agents` inside a writable root cannot be made write-protected
  (landlock is union/most-permissive with no deny-rules). The `bwrap` path honors protected metadata
  via read-only bind mounts; the lexical `validate_command` remains defense-in-depth in both paths.

## Local validation

```sh
cargo check -p slab-linux-sandbox
cargo clippy -p slab-linux-sandbox --all-targets -- -D warnings
cargo fmt --all --check
```

OS-level tests require Linux (kernel ≥ 5.13 for landlock) with `bwrap` installed and are gated behind
`SLAB_SANDBOX_LINUX=1` (self-skip otherwise):

```sh
SLAB_SANDBOX_LINUX=1 cargo test -p slab-linux-sandbox --test os_isolation -- --ignored --nocapture
```

## Hard boundaries

- No dependency on `slab-sandboxing` (cycle-free).
- No helper binary / daemon / IPC (single in-process crate; `pre_exec` is the only mechanism).
- Linux-only: everything is `#[cfg(target_os = "linux")]`.
