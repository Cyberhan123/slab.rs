---
name: web-design-guidelines
description: Review UI code for web interface quality, accessibility, and interaction issues using a deterministic local checklist, with optional upstream Vercel guidance when the user asks for the latest rules.
---

# Web Interface Guidelines

Use this skill when reviewing UI code, accessibility, UX quality, or interaction design.

## Default Review Baseline

Start with the local snapshot at [`REVIEW-CHECKLIST.md`](REVIEW-CHECKLIST.md). It is the deterministic baseline for this repository and should be used for normal reviews.

## Optional Upstream Check

If the user explicitly asks for the latest upstream Vercel guidance, fetch and compare against:

```text
https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md
```

When you do this, say clearly that the review includes live upstream guidance and may differ from the local snapshot.

## Review Workflow

1. Read `REVIEW-CHECKLIST.md`.
2. Read the requested files or the relevant UI surface.
3. Apply the checklist first.
4. Add upstream-only findings only if the user asked for the latest Vercel rules.
5. Output findings in terse `file:line - issue` format.

If the user does not specify files, ask which files or UI area to review.
