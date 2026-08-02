# slab-exec-policy

The unified permission decision engine for slab agent tools. This crate is the
single owner of every `Allow` / `RequireApproval` / `Deny` verdict.

## Role

`slab-exec-policy` demotes the shell policy, the kernel risk analyzer, and the
sandbox to inputs feeding one decision engine. It owns:

- The verdict vocabulary: [`ExecDecision`] (`Allow` / `RequireApproval` /
  `Deny`), the per-session [`PermissionMode`], the popup persistence scope
  [`ApprovalScope`], and the global `agent.permissions` baseline
  [`PermissionBaseline`].
- The decision engine itself: the [`ExecPolicyPort`] trait the agent kernel
  calls, its concrete [`ExecPolicyEngine`] implementation, and a permissive
  [`AllowAllExecPolicy`] stub for tests.
- A per-category rule system (migrated and generalized from
  `slab-shell-command`) and the rule persistence port with a filesystem default.
- Hard-deny safety checks for destructive shell commands and sensitive paths —
  these refuse unconditionally, even under `FullControl`.
- Progressive tool exposure: a bit-set of which tool categories the agent is
  allowed to *see* this turn, so read-only mode hides mutation tools from the
  tool list rather than merely blocking them after the fact.

Dependency position: `slab-agent → slab-exec-policy → slab-sandboxing`. The
baseline maps 1:1 onto [`slab_sandboxing::SandboxPolicy`].

## Decision model

The engine resolves each thread's effective behavior from its `PermissionMode`
(+ baseline for `Custom`), then evaluates an operation in this order:

1. **Hard-deny safety** — destructive shell patterns and sensitive paths are
   refused regardless of mode.
2. **`FullControl`** — allow everything that survived hard-deny.
3. **`StrictReadOnly`** — mutations denied; reads allowed.
4. **`RequestApproval`** — per-category default (`ReadOnly` → allow; shell /
   file-edit / network → prompt), then rules override.
5. **Rules** — first match wins: `Allow` short-circuits to allow, `Block` denies,
   `RequireApproval` prompts.

`RequestApproval` gates invocation via the approval popup, not tool *visibility*,
so every category stays exposed there. The sandbox's safe-auto-allow was removed,
so every shell command surfaces an approval decision unless a remembered `Allow`
rule matches.

## Exec rules

Tool operations can be refined with `.rule` / `.rules` files. The desktop/server
wiring loads these lazily — only `default.rules` (global) and the current
workspace's `hash-<workspace>.rules` (per-workspace), never the whole rules
directory. A DB-backed store in `slab-app-core` records the
`hash-<workspace>.rules → absolute workspace path` mapping.

Rule files are evaluated in file-name then line-number order. Blank lines and
lines starting with `#` are ignored. The first matching rule wins:

```txt
network allow prefix https
file_edit require_approval exact /workspace/secret.key
shell block contains Remove-Item
```

Each rule is `<category> <action> <matcher> <pattern>`. Categories are `shell`,
`file_edit`, `network`, `read_only`; actions are `allow`, `require_approval`,
`block`; matchers are `exact`, `prefix`, `contains`. The parser also accepts the
legacy 3-token `<action> <matcher> <pattern>` shell-only form, which loads with
category = `shell`.

`prefix` shell rules require a token boundary after the pattern and do not match
chained shell segments, so `allow prefix cargo check` matches
`cargo check -p slab-agent` but not `cargo checkout` or `cargo check && ...`.
File paths use `exact` so a persisted allow targets the specific path; commands
and queries use `prefix`.

## Boundaries

This crate owns the decision, not the execution. It does not:

- Execute commands or touch the filesystem beyond reading/writing rule files.
  Sandbox execution isolation lives in `slab-sandboxing`.
- Host approval transport — callers route the `RequireApproval` verdict through
  `slab-agent` / host ports.
- Persist rules to a database — the [`RuleStore`] port has an [`FsRuleStore`]
  default; the DB-backed implementation lives in `slab-app-core`.

A backward-compatibility [`ShellPolicy`] shim is kept so `slab-shell-command` and
`slab-agent-tools` compile during the migration; new code should use
`PermissionMode` + `PermissionBaseline`.

## Type

Rust library crate.

## Testing

```sh
cargo test -p slab-exec-policy
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
