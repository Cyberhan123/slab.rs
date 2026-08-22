# core/ — feature-agnostic foundation

Infrastructure that does not know any feature exists.

## Admission rule

A file belongs in `core/` only if it could be dropped into a different app
that talks to the same server: **core must not import from `features/`,
`domain/`, or `data/`** (it may not even know their types). Allowed inputs are
Flutter/pub packages and sibling `core/` files.

## Residents

- `app/` — app-wide cubits (locale, connection)
- `di/` — get_it composition root (the one place allowed to know everything)
- `network/` — dio builder + auth/error interceptors + `SlabRestException`
- `theme/` — generated `slab_tokens.g.dart` (`bun run gen:mobile`) + TDesign
  theme wiring
- `l10n/` — locale catalog, mobile-only chrome strings, TDesign resource
  delegate
- `utils/` — pure helpers (ANSI SGR parser)
- `widgets/` — shared chrome widgets (health indicator)

`di/` is the sanctioned escape hatch: it wires core + data + features together,
so it imports across domains while nothing else in core does.
