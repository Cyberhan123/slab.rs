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
(+ the global baseline for `Custom` and `ApproveForMe`), then evaluates an
operation in this order:

1. **Hard-deny safety** — destructive shell patterns and sensitive paths are
   refused regardless of mode.
2. **Policy/enterprise rules** — the immutable `policy.rules` partition is
   consulted first; a matching rule is authoritative. A policy `Block` denies
   unconditionally (even under `FullControl`) and cannot be overridden.
3. **`FullControl`** — allow everything that survived hard-deny + policy.
4. **`StrictReadOnly`** (`Custom` + `ReadOnly` baseline) — mutations denied;
   reads allowed.
5. **`RequestApproval` / `AcceptEdits`** — per-category base, then rules
   override. `RequestApproval` defaults every mutation to prompt. `AcceptEdits`
   (`ApproveForMe`) elevates the base to *allow* for operations the active
   baseline already permits: reads under `ReadOnly`; reads + in-workspace file
   edits + non-destructive, non-network shell under `WorkspaceWrite`; everything
   under `FullAccess`. The rest still prompt.
6. **Rules** — first match wins: `Allow` short-circuits to allow, `Block` denies,
   `RequireApproval` prompts. This runs for both `RequestApproval` and
   `AcceptEdits`, so a `Block` rule still denies an in-envelope op and an `Allow`
   rule can permit an out-of-envelope one.

`RequestApproval` and `AcceptEdits` gate invocation via the approval popup (or the
acceptEdits auto-allow), not tool *visibility*, so every category stays exposed.
Under `AcceptEdits` the scoped shell auto-allow (non-destructive, non-network) is
intentionally re-introduced — the blanket safe-auto-allow was removed so every
shell command surfaced an approval decision; `AcceptEdits` scopes it to the
baseline envelope, and hard-deny safety + `Block` rules still apply.

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

### Rule sources and precedence

Rules load from three scopes, evaluated in this priority order (first match
wins, and the engine consults each in the same order as the decision model
above):

1. **Policy** — `<rules_dir>/policy.rules`, a reserved, read-only file deployed
   by an administrator. Held in a separate immutable partition and consulted
   *first*, so a policy `Block` cannot be overridden by user, workspace, or
   global rules. Never written by the store; absent ⇒ no policy rules.
2. **Workspace** — `<rules_dir>/hash-<workspace>.rules` (`AlwaysInWorkspace`
   approvals).
3. **Global** — `<rules_dir>/default.rules` (`Always` approvals).

An optional leading `tool=<pattern>` token scopes a rule to a tool name (a
trailing `*` globs namespaced tools, e.g. `tool=mcp__github__*`), and the `glob`
matcher supports `*`-patterns. Both are backward compatible: lines without them
keep their existing meaning.

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
