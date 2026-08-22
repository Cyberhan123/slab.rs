/// Composer draft persistence: one draft per session (text + the turn
/// options the composer carries), written debounced from the composer.
library;

import 'package:drift/drift.dart';

import 'app_database.dart';

class DraftsDao {
  DraftsDao(this._db);

  final AppDatabase _db;

  Future<ComposerDraft?> load(String sessionId) async {
    final query = _db.select(_db.composerDrafts)..where((draft) => draft.sessionId.equals(sessionId));
    return query.getSingleOrNull();
  }

  Future<void> save({
    required String sessionId,
    required String content,
    required bool planMode,
    String? effort,
    String? permissionMode,
  }) async {
    await _db.into(_db.composerDrafts).insertOnConflictUpdate(
          ComposerDraftsCompanion.insert(
            sessionId: sessionId,
            content: content,
            planMode: Value(planMode),
            effort: Value(effort),
            permissionMode: Value(permissionMode),
            updatedAt: DateTime.now(),
          ),
        );
  }

  /// Clears once the draft is consumed (message sent).
  Future<void> clear(String sessionId) async {
    await (_db.delete(_db.composerDrafts)..where((draft) => draft.sessionId.equals(sessionId))).go();
  }
}
