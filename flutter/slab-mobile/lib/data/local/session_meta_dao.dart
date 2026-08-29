/// Session meta access: the current-session pointer (app KV) and per-session
/// label overrides. Mirrors what the desktop persists in the zustand
/// assistant store (currentSessionId + sessionLabels).
library;

import 'package:drift/drift.dart';

import 'app_database.dart';

class SessionMetaDao {
  SessionMetaDao(this._db);

  final AppDatabase _db;

  static const _currentSessionKey = 'currentSessionId';

  Future<String?> getCurrentSessionId() async {
    final row = await (_db.select(_db.appKv)..where((kv) => kv.key.equals(_currentSessionKey))).getSingleOrNull();
    return row?.value;
  }

  /// Null clears the pointer (e.g. the session was deleted).
  Future<void> setCurrentSessionId(String? sessionId) async {
    if (sessionId == null) {
      await (_db.delete(_db.appKv)..where((kv) => kv.key.equals(_currentSessionKey))).go();
      return;
    }
    await _db.into(_db.appKv).insertOnConflictUpdate(
          AppKvCompanion.insert(key: _currentSessionKey, value: sessionId),
        );
  }

  /// All label overrides keyed by session id (null labels are skipped —
  /// callers fall back to the server name).
  Future<Map<String, String>> labels() async {
    final rows = await (_db.select(_db.sessionLabels)
          ..where((label) => label.label.isNotNull())
          ..orderBy([(label) => OrderingTerm(expression: label.updatedAt)]))
        .get();
    return {for (final row in rows) row.sessionId: row.label!};
  }

  Future<void> upsertLabel(String sessionId, String? label) async {
    await _db.into(_db.sessionLabels).insertOnConflictUpdate(
          SessionLabelsCompanion.insert(
            sessionId: sessionId,
            label: Value(label),
            updatedAt: DateTime.now(),
          ),
        );
  }

  /// Drop labels for sessions that no longer exist server-side.
  Future<void> retainOnly(Set<String> liveSessionIds) async {
    await (_db.delete(_db.sessionLabels)
          ..where((label) => label.sessionId.isNotIn(liveSessionIds)))
        .go();
  }
}
