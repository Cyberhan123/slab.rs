---
name: slab-ui-primitives
description: Maintain shared UI primitives and design-system-level frontend building blocks in slab-app. Use for slab-app/src/components/ui, shared forms, theme variables, and global UI patterns.
---

# Slab UI Primitives

Use this skill for shared frontend building blocks, especially:

- `slab-app/src/components/ui/**`
- shared form patterns
- shared command, drawer, dialog, table, and navigation primitives
- theme variables and global style rules in `slab-app/src/styles/globals.css`

## Repo Defaults

- The repo already has a large shared primitive set under `src/components/ui`.
- `components.json` exists, so this is an existing shadcn-style setup, not a fresh install task.
- Ant Design X and `antd` are already present for AI-oriented surfaces; not every interaction belongs in `src/components/ui`.

## Workflow

1. Inspect existing primitives before adding a new one.
2. Decide whether the change belongs in:
   - a shared primitive under `src/components/ui`
   - a page-local component
   - an Ant Design X or `antd` surface already used by the page
3. If the task needs generic implementation patterns, open `../shadcn-ui/SKILL.md`.
4. Prefer adapting the current primitive style over re-running setup commands like `shadcn init`.
5. Keep APIs small, composable, and consistent with existing exports and utility patterns.
6. Verify theme variables and global styles still work across light, dark, and dense desktop layouts.

## Avoid

- Do not mass-import or regenerate a full component set.
- Do not add a new component family when an existing primitive can be extended.
- Do not blindly port Next.js-specific examples from generic shadcn guidance.
- Do not move AI/chat-specific presentation into shared primitives unless reuse is proven.

## Useful Files

- `slab-app/components.json`
- `slab-app/src/components/ui/**`
- `slab-app/src/styles/globals.css`
- `slab-app/src/lib/utils.ts`
- `slab-app/src/pages/chat/**`

## Done When

- The primitive matches the existing shared UI style.
- The change reduces duplication instead of creating another abstraction layer.
- Theme behavior and accessibility remain intact.
