# @slab/mobile — slab-mobile

Flutter mobile client for a running slab-server. A **pure network client** over the
existing `/v1` REST surface + the `/v1/agents/harness` JSON-RPC WebSocket — zero
backend changes, zero Rust on the device (phase 3 reserves that path, see below).

Scope (it supersedes the removed `@slab/h5` mobile-web shell): the full
assistant surface ported from the desktop web UI — connect config → setup gate
→ two-tab shell (conversations / settings) → full-screen streaming chat with
terminal / file-change / plan tool cards, command-fileChange-plan approvals
with scopes, model lifecycle (list/download/load + switch dialog), plan mode,
`/compact`+`/fork`+rollback, slash-command menu, gallery image attachments,
token-usage indicator — plus the schema-driven settings module (per-field
debounced autosave, structured JSON editor, cloud-provider registry editor).

## Role

- **Architecture position**: the mobile sibling of the `slab-web` shell.
  It does NOT share `@slab/ui` code — visual unity comes from the
  one-way design-token pipeline below, not shared React components.
- **UI library**: [tdesign_flutter](https://tdesign.tencent.com/flutter) (exact
  pin `0.2.7` — 0.x alpha-stage lib; range re-resolution could pull breaking
  API churn, so upgrades are deliberate diffs). Pages are built from TDesign
  components (TDNavBar/TDCell/TDButton/TDInputDialog/TDSwipeCell/TDEasyRefresh
  …); chat bubbles / markdown / tool cards stay custom but source every color
  from `TDTheme.of(context)` or `SlabExtras`. `easy_refresh` (mirrored
  constraint) provides the pull-to-refresh shell around TDesign's
  `TDRefreshHeader`.
- **Flutter SDK**: 3.38.x stable (pinned by CI; Dart 3.10). Android-first —
  the `ios/` skeleton is committed but building it requires macOS.

## Layout

```
lib/
  main.dart / app.dart            entrypoint (ScreenUtilInit + get_it bootstrap), MaterialApp.router
  core/
    app/                          app-wide cubits: LocaleCubit, ConnectionCubit
    di/service_locator.dart       composition root (get_it)
    network/                      buildSlabDio + auth/error interceptors + SlabRestException
    db/                           drift store (kv / session labels / drafts) + DAOs
    utils/ansi.dart               ANSI SGR → TextSpan parser (terminal cards)
    widgets/                      shared chrome (health indicator)
  features/
    sessions/                     conversations tab (cubit + page)
    assistant/                    bootstrap (labels) · model (lifecycle) · commands
                                  (slash dispatch) · view (chat screen, messages/,
                                  composer/, approval)
    settings/                     settings_cubit · autosave/ · cloud/ (provider
                                  registry) · view (page + field cards +
                                  structured editor)
    connect/, setup/              gate screens
  l10n/                           catalog + mobile_strings + TDesign resource delegate
  proto/                          json_rpc / harness_methods / harness_types / harness_client (+reconnect)
  data/                           rest_client (dio) / model_types / settings_types / connection_config
  conversation/                   turn_items (history+live projection) / conversation_controller
  routes/                         app_router (shell + gates + full-screen chat) / home_shell (TTabBar)
design/tokens.json                GENERATED — inspectable token intermediate
assets/theme/tdesign-theme.json   GENERATED — tdesign_flutter theme (slab light + slabDark)
assets/i18n/{en-US,zh-CN}.json    GENERATED — flat catalogs from @slab/i18n (incl. the server ns)
lib/core/db/*.g.dart              GENERATED — drift (dart run build_runner build)
```

## Local validation

```sh
bun run gen:mobile       # regenerate tokens + locale assets (NO Flutter SDK needed)
bun run check:mobile     # flutter analyze
bun run test:mobile      # flutter test (136 unit/widget tests; no device needed)
bun run dev:server       # headless slab-server alone (mobile clients can target it)
bun run dev:mobile       # dev:server in the background + flutter run; pass defines through:
bun run dev:mobile -- --dart-define=SLAB_API_BASE_URL=http://10.0.2.2:3000
```

- **Android emulator**: `10.0.2.2` is the host loopback — the `dev:server` /
  `dev:mobile` `slab-server` on `127.0.0.1:3000` is reachable as above.
- **Physical device**: start slab-server with `--gateway-bind 0.0.0.0:3000`
  (see `bin/slab-server/README.md`), allow it through the firewall, then
  connect to `http://<LAN-IP>:3000`.
- Plain-HTTP LAN access: Android allows cleartext via
  `android:usesCleartextTraffic` (already set); iOS has
  `NSAppTransportSecurity → NSAllowsLocalNetworking` (untested from Windows).

## Generated artifacts — do not hand-edit

`lib/theme/slab_tokens.g.dart`, `design/tokens.json`,
`assets/theme/tdesign-theme.json`, `assets/i18n/*.json` are produced by
`bun run gen:mobile` (`scripts/design/export-tokens.ts` +
`scripts/i18n/export-mobile-locales.ts`) and drift-checked in CI with
`git diff --exit-code`. The TDesign theme JSON carries the COMPLETE palette
for both modes — tdesign_flutter falls back to its built-in light blue for
missing keys even in dark mode, so the exporter asserts key parity, scale
monotonicity, and anchor equality (e.g. light `brandColor7` == `--primary`).
Colors in Dart app code must come from `TDTheme.of(context)` / `SlabTokens*` /
`SlabExtras` — the design guard bans `Color(0x…)`/`Colors.*` (and, as a
substring side effect, `TDColors.*`) outside the generated file, exactly as it
bans raw hex in TSX.

## Hard boundaries

- **No parallel API tree**: only the documented `/v1` + harness subset
  (health, setup/status, sessions CRUD, models + tasks, settings document +
  per-pmid updates; harness `initialize`/`thread/*`/`turn/*`/
  `approval/resolve`/`model/list`/`command/list`).
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
