---
name: slab-ui-review
description: Review slab-app UI for project-specific UX, accessibility, and desktop interaction issues. Use for page reviews, component reviews, regression checks, and pre-merge UI audits in this repository.
---

# Slab UI Review

Use this skill for reviewing `slab-app` UI code.

## Workflow

1. Read `../web-design-guidelines/REVIEW-CHECKLIST.md`.
2. Review the requested files or UI area.
3. Check project-specific concerns first:
   - desktop window constraints
   - long chat or task lists
   - loading, empty, and error states
   - keyboard and focus behavior
   - theme and contrast consistency
4. If the user explicitly asks for the latest upstream guidance, also open `../web-design-guidelines/SKILL.md` and include that comparison.
5. If runtime performance is part of the review, also open `../vercel-react-best-practices/SKILL.md` and apply only repo-compatible React rules.

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
