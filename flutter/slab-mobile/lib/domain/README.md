# domain/ — framework-free logic and state machines

## Admission rules

A file belongs in `domain/` when it is:

- **pure Dart testable** — no widget/UI imports; `Listenable` from
  `package:flutter/foundation.dart` is the one sanctioned Flutter edge
- **cross-feature or protocol-level** — it spans multiple features or owns a
  wire-level state machine, so it does not fit inside a single feature

## Residents

- `conversation/` — `conversation_controller` (turn lifecycle + listener set)
  and `turn_items` (history + live timeline projection over harness events)
- `session_labels.dart` — canonical session label set shared by assistant and
  sessions features

Dependencies point downward into `data/harness/` protocol types only; nothing
in domain knows about widgets, routes, or features.
