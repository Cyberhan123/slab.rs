# data/ — outside-world adapters, one axis only

Everything that talks to something outside the Dart heap, classified strictly
by **persistence vs protocol** (the historical "proto" name is retired):

- `local/` — device-persisted state: drift store (`app_database` + DAOs) and
  `shared_preferences` connection config
- `rest/` — the HTTP surface: dio `rest_client` + wire types
  (`model_types`, `settings_types`)
- `harness/` — the `/v1/agents/harness` JSON-RPC WebSocket protocol: codecs
  (`json_rpc`, `harness_methods`, `harness_types`) + reconnecting client

## Rules

- Wire codecs are hand-written JSON — no codegen serializers (freezed ×
  json_serializable was evaluated and rejected on this repo's shapes).
- `data/` exposes plain types and futures/streams; it holds no widget or
  navigation logic. Cubits that orchestrate it live in `features/` or
  `domain/`.
- `harness_methods.dart` mirrors `crates/slab-proto` constants;
  `bun run gen:harness` fails on drift.
