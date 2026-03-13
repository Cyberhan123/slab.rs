# Repo Web Review Checklist

Version: 2026-03-13

Use this file as the deterministic review baseline for `slab-app` and other web UI in this repository.

## Structure And Semantics

- Use semantic landmarks and heading order that matches the visible page structure.
- Use real buttons for actions and real links for navigation.
- Avoid clickable `div` or `span` elements unless there is a strong reason and keyboard support is preserved.

## Keyboard And Focus

- Every interactive control must be reachable with the keyboard.
- Focus states must be visible.
- Dialogs, drawers, menus, and command surfaces should trap and restore focus correctly.
- Keyboard shortcuts should not hide the underlying action from non-shortcut users.

## Forms And Validation

- Inputs need labels or an accessible name.
- Required, invalid, and help states should be communicated in text, not color alone.
- Async submit flows should show pending, success, and error states.

## Visual Clarity

- Text contrast should remain readable on all supported themes.
- Do not rely on color alone to communicate status.
- Interactive targets should be comfortably clickable, especially in dense desktop layouts.

## Motion

- Respect reduced-motion preferences for non-essential animation.
- Avoid hover effects that shift layout or make controls harder to target.
- Streaming or auto-updating surfaces should not steal focus unexpectedly.

## Responsive And Window Constraints

- The default desktop window is 1024x768, so layouts should remain usable at that size.
- Fixed headers, sidebars, and footers should not clip content.
- Narrow widths should degrade gracefully rather than overflow or hide key actions.

## Dense AI Workflows

- Loading, empty, and error states should exist for task lists, chat surfaces, and generated content.
- Long message lists, transcripts, or result panes should preserve scroll behavior and avoid jank.
- Very long lists should consider pagination, windowing, or another performance strategy.

## Tauri-Specific Checks

- Do not assume unrestricted browser APIs in desktop flows.
- External links or shell interactions should follow the approved Tauri patterns already used in the repo.
- Preserve CSP and capability boundaries unless the task explicitly requires a change.

## Output Format

- Prefer `path:line - issue` findings.
- Focus on bugs, accessibility risks, regressions, and user-facing friction before stylistic opinions.
