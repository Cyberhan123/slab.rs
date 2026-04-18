# @slab/components

Shared UI component library for Slab.

## Role

`@slab/components` provides the shared React component primitives used by `@slab/desktop` and any future frontend packages. It is built on:

- [shadcn/ui](https://ui.shadcn.com/) component patterns.
- [Radix UI](https://www.radix-ui.com/) for accessible primitives.
- [Tailwind CSS 4](https://tailwindcss.com/) for styling.

The library exposes components via subpath exports (e.g., `@slab/components/*`) and includes a shared `globals.css` with CSS custom property tokens for theming.

## Testing

- `tests/browser/components/*.browser.test.tsx`: browser-mode component tests built on `vitest-browser-react`.
- Use `renderComponentScene` from `tests/browser/test-utils.tsx` to keep fixtures visually stable and screenshot-friendly.
- Run component browser tests with `bun run test:run`.
- Refresh screenshot baselines with `bun run test:update`.

## Type

Bun-managed frontend package.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
