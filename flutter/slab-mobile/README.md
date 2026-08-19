# @slab/mobile — slab-mobile

Flutter mobile client for a running slab-server. A **pure network client** over the
existing `/v1` REST surface + the `/v1/agents/harness` JSON-RPC WebSocket — zero
backend changes, zero Rust on the device (phase 3 reserves that path, see below).

Phase 1 scope (it supersedes the removed `@slab/h5` mobile-web shell): connect
config → setup gate → conversation list → streaming chat with tool cards and
command/file-change approvals.

## Role

- **Architecture position**: the mobile sibling of the `slab-web` shell.
  It does NOT share `@slab/ui` code — visual unity comes from the
  one-way design-token pipeline below, not shared React components.
- **Flutter SDK**: 3.38.x stable (pinned by CI; Dart 3.10). Android-first —
  the `ios/` skeleton is committed but building it requires macOS.

## Layout

```
lib/
  main.dart / app.dart            entrypoint, MaterialApp.router, token themes
  theme/slab_tokens.g.dart        GENERATED — regenerate with `bun run gen:mobile`
  theme/slab_theme.dart           hand-written mapping of tokens → Material
  l10n/catalog.dart + mobile_strings.dart
  proto/  json_rpc / harness_methods / harness_types / harness_client (+reconnect)
  data/   rest_client / connection_config
  conversation/ turn_items (history+live projection) / conversation_controller
  routes/ app_router (connect → setup gate → sessions → chat)
  pages/ + widgets/
design/tokens.json                GENERATED — inspectable token intermediate
assets/i18n/{en-US,zh-CN}.json    GENERATED — flat catalogs from @slab/i18n
```

## Local validation

```sh
bun run gen:mobile       # regenerate tokens + locale assets (NO Flutter SDK needed)
bun run check:mobile     # flutter analyze
bun run test:mobile      # flutter test (52 unit tests; no device needed)
bun run dev:mobile       # flutter run; pass defines through:
bun run dev:mobile -- --dart-define=SLAB_API_BASE_URL=http://10.0.2.2:3000
```

- **Android emulator**: `10.0.2.2` is the host loopback — a desktop
  `slab-server` on `127.0.0.1:3000` is reachable as above.
- **Physical device**: start slab-server with `--gateway-bind 0.0.0.0:3000`
  (see `bin/slab-server/README.md`), allow it through the firewall, then
  connect to `http://<LAN-IP>:3000`.
- Plain-HTTP LAN access: Android allows cleartext via
  `android:usesCleartextTraffic` (already set); iOS has
  `NSAppTransportSecurity → NSAllowsLocalNetworking` (untested from Windows).

## Generated artifacts — do not hand-edit

`lib/theme/slab_tokens.g.dart`, `design/tokens.json`, `assets/i18n/*.json` are
produced by `bun run gen:mobile` (`scripts/design/export-tokens.ts` +
`scripts/i18n/export-mobile-locales.ts`) and drift-checked in CI with
`git diff --exit-code`. Colors in Dart app code must come from
`SlabTokens*`/`SlabExtras` — the design guard bans `Color(0x…)`/`Colors.*`
outside the generated file, exactly as it bans raw hex in TSX.

## Hard boundaries

- **No parallel API tree**: only the documented `/v1` + harness subset
  (health, setup/status, sessions CRUD; harness `initialize`/`thread/*`/
  `turn/*`/`approval/resolve`/`model/list`).
- **No code sharing with `@slab/ui`**; token values flow one way from
  `packages/slab-components/src/styles/globals.css`.
- **Harness constants** (`lib/proto/harness_methods.dart`) mirror
  `crates/slab-proto`; `bun run gen:harness` fails on drift.
- **Connection secrets**: stored in plain `shared_preferences` (LAN tool). The
  documented upgrade path is `flutter_secure_storage` once tokens protect
  anything sensitive.
- **On-device Rust (phase 3, not started)**: will land behind a reserved
  `crates/slab-mobile-ffi` facade (flutter_rust_bridge over a hand-picked
  subset of `slab-app-core`). Do NOT create that crate speculatively; the Dart
  seam already isolates it at `lib/proto/` + `lib/data/`.
