---
name: ui-ux-pro-max
description: UI/UX design guidance and searchable design-system data for building or reviewing web and desktop interfaces.
---

# UI UX Pro Max

Use this skill for UI design, implementation, polish, or review tasks.

## When To Use It

- Designing a new page, flow, or visual system
- Refining an existing interface
- Reviewing UI quality, interaction polish, or information hierarchy
- Generating a design direction before coding

If the task is specifically a standards or accessibility review, also use `web-design-guidelines`.

## Repo-Specific Defaults

- In this repository, UI work usually targets `slab-app`.
- The existing app stack is React 19 + Vite + React Router 7 + Tauri 2.
- Styling uses Tailwind 4 and shared primitives under `slab-app/src/components/ui`.
- AI/chat surfaces also use Ant Design X and `antd`.
- When editing existing screens, preserve those patterns. Do not default to a standalone `html-tailwind` prototype unless the user explicitly wants a fresh mock or artifact outside `slab-app`.

## Running The Search Tool

Run the bundled script from the repo root and use paths relative to this skill directory.

### Windows PowerShell

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "query" --design-system -p "Project Name"
```

### POSIX shells

```bash
python3 .agents/skills/ui-ux-pro-max/scripts/search.py "query" --design-system -p "Project Name"
```

### Check Python

- Windows PowerShell: `python --version`
- POSIX shells: `python3 --version`

If Python is missing on Windows, install it with `winget install Python.Python.3.12`.

## Recommended Workflow

1. Identify the product area, audience, tone, and whether the task is greenfield or editing an existing screen.
2. For `slab-app`, default to `--stack react`. Add `--stack shadcn` when you are shaping shared UI primitives or Tailwind-driven component patterns.
3. Generate a design system first with `--design-system`.
4. Use domain searches such as `--domain ux`, `--domain style`, or `--domain typography` only when you need extra detail.
5. If you want files written to disk, pass `--persist` and set `--output-dir` explicitly.

## Common Commands

### Design system for this repo

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "desktop AI workspace calm utilitarian" --design-system -p "Slab"
```

### React implementation guidance

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "layout responsive form" --stack react
```

### Shared primitive guidance

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "drawer command palette settings form" --stack shadcn
```

### UX review support

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "loading accessibility empty states" --domain ux
```

### Persist a design system

```powershell
python .agents/skills/ui-ux-pro-max/scripts/search.py "desktop AI workspace calm utilitarian" --design-system --persist -p "Slab" --output-dir design-system
```

## Notes

- The old `skills/ui-ux-pro-max/...` examples are not valid in this repo; always use `.agents/skills/ui-ux-pro-max/...`.
- If the user asks for a review rather than a redesign, prefer `web-design-guidelines` for the review baseline and use this skill only to deepen design direction or remediation ideas.
- Keep results aligned with the actual app stack instead of replacing React Query, Zustand, Ant Design X, or the shared UI primitives without a clear reason.
