/// Drift store tests over an in-memory database — also the first gate that
/// `sqlite3` 3.x native assets work under `flutter test` on the host.
library;

import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/core/db/app_database.dart';
import 'package:slab_mobile/core/db/drafts_dao.dart';
import 'package:slab_mobile/core/db/session_meta_dao.dart';

void main() {
  late AppDatabase db;
  late SessionMetaDao meta;
  late DraftsDao drafts;

  setUp(() {
    db = AppDatabase(NativeDatabase.memory());
    meta = SessionMetaDao(db);
    drafts = DraftsDao(db);
  });

  tearDown(() async => db.close());

  test('current-session pointer round-trips and clears', () async {
    expect(await meta.getCurrentSessionId(), isNull);
    await meta.setCurrentSessionId('s1');
    expect(await meta.getCurrentSessionId(), 's1');
    // Overwrite, then clear.
    await meta.setCurrentSessionId('s2');
    expect(await meta.getCurrentSessionId(), 's2');
    await meta.setCurrentSessionId(null);
    expect(await meta.getCurrentSessionId(), isNull);
  });

  test('label map skips nulls and upsert keeps the latest value', () async {
    await meta.upsertLabel('s1', 'First prompt');
    await meta.upsertLabel('s2', null);
    expect(await meta.labels(), {'s1': 'First prompt'});
    await meta.upsertLabel('s1', 'Renamed');
    expect(await meta.labels(), {'s1': 'Renamed'});
  });

  test('retainOnly prunes labels of deleted sessions', () async {
    await meta.upsertLabel('s1', 'a');
    await meta.upsertLabel('s2', 'b');
    await meta.retainOnly({'s1'});
    expect(await meta.labels(), {'s1': 'a'});
  });

  test('composer draft saves, reloads, and clears', () async {
    expect(await drafts.load('s1'), isNull);
    await drafts.save(sessionId: 's1', content: 'hello', planMode: true, effort: 'high');
    final draft = await drafts.load('s1');
    expect(draft?.content, 'hello');
    expect(draft?.planMode, isTrue);
    expect(draft?.effort, 'high');
    expect(draft?.permissionMode, isNull);
    await drafts.save(sessionId: 's1', content: '', planMode: false);
    final cleared = await drafts.load('s1');
    expect(cleared?.content, '');
    expect(cleared?.planMode, isFalse);
    await drafts.clear('s1');
    expect(await drafts.load('s1'), isNull);
  });
}
