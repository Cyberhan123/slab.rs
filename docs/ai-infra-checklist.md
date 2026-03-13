# AI Infrastructure Checklist

Use this checklist whenever the repo architecture, frontend stack, review workflow, or local skills change.

## Keep In Sync

- `AGENTS.md`
- `CLAUDE.md`
- `.github/copilot-instructions.md`
- Relevant `.agents/skills/*/SKILL.md`
- Any local review or reference files used by those skills

## What To Verify

- Documentation claims still match the current code layout.
- Example commands work from the repo root on the shell they claim to support.
- Skill paths resolve correctly relative to the repo and the skill directory.
- Skill trigger text matches the actual stack and does not over-trigger on unrelated tasks.
- Remote guidance is optional or versioned; routine review flows should have a local baseline.
- Security notes match current config, especially Tauri CSP, capabilities, auth, and transport boundaries.

## Quick Validation Ideas

- Search for stale architecture claims after major refactors.
- Run the example scripts in local skills with `--help` or a small sample query.
- Confirm the frontend stack listed in docs still matches `slab-app/package.json`.
- Confirm Tauri security notes still match `slab-app/src-tauri/tauri.conf.json`.
