/// Drift table definitions for the mobile-local store.
///
/// Scope: data the server does NOT own — the current-session pointer,
/// per-session display labels (desktop keeps these in the persisted zustand
/// assistant store), and composer drafts. Server-owned records (sessions,
/// threads, settings) are always fetched, never cached here.
library;

import 'package:drift/drift.dart';

/// Generic string KV (`currentSessionId` today).
class AppKv extends Table {
  TextColumn get key => text()();
  TextColumn get value => text()();

  @override
  Set<Column> get primaryKey => {key};
}

/// Display label overrides per slab session (null = use the server name).
class SessionLabels extends Table {
  TextColumn get sessionId => text()();
  TextColumn get label => text().nullable()();
  DateTimeColumn get updatedAt => dateTime()();

  @override
  Set<Column> get primaryKey => {sessionId};
}

/// Composer draft per session: text plus the turn options that ride the
/// composer (plan mode, reasoning effort, permission mode). The text column
/// is `content` — `text` collides with drift's column-builder DSL.
class ComposerDrafts extends Table {
  TextColumn get sessionId => text()();
  TextColumn get content => text()();
  BoolColumn get planMode => boolean().withDefault(const Constant(false))();
  TextColumn get effort => text().nullable()();
  TextColumn get permissionMode => text().nullable()();
  DateTimeColumn get updatedAt => dateTime()();

  @override
  Set<Column> get primaryKey => {sessionId};
}
