---
name: slab-ui-review
description: Review slab-app UI for project-specific UX, accessibility, and desktop interaction issues. Use for page reviews, component reviews, regression checks, and pre-merge UI audits in this repository.
---

# Slab UI Review

Use this skill for reviewing `slab-app` UI code.

## Workflow

1. Read the requested files or UI area and the nearest route or layout.
2. Check project-specific concerns first:
   - desktop window constraints
   - long chat or task lists
   - loading, empty, and error states
   - keyboard and focus behavior
   - theme and contrast consistency
   - streaming Markdown, code blocks, and scroll anchoring on chat surfaces
3. If the review touches `slab-app/src/pages/chat/**`, also open `../use-x-chat/SKILL.md`, `../x-request/SKILL.md`, or `../x-markdown/SKILL.md` as needed.
4. If the user wants remediation ideas or a stronger design direction after the review, also open `../ui-ux-pro-max/SKILL.md`.
5. Keep React and Tauri feedback repo-compatible: prefer state locality, stable route behavior, and explicit desktop interactions over framework-agnostic advice.

## Output Style

- Prefer `path:line - issue`
- Lead with bugs, accessibility risks, regressions, and usability friction
- Keep styling opinions secondary unless they affect clarity or consistency

## Useful Files

- `slab-app/src/pages/**`
- `slab-app/src/components/**`
- `slab-app/src/layouts/**`
- `slab-app/src/styles/globals.css`
- `slab-app/src/routes/index.tsx`

## Done When

- The review reflects both generic UI quality and this repo's desktop AI workflow constraints.
- Findings are specific enough to act on directly.
