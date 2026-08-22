/// App-wide drift database. Tests inject `AppDatabase(NativeDatabase.memory())`;
/// production opens lazily from the app-documents directory so registration
/// in the service locator stays synchronous.
library;

import 'dart:io';

import 'package:drift/drift.dart';
import 'package:drift/native.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import 'tables.dart';

part 'app_database.g.dart';

@DriftDatabase(tables: [AppKv, SessionLabels, ComposerDrafts])
class AppDatabase extends _$AppDatabase {
  AppDatabase(super.executor);

  @override
  int get schemaVersion => 1;
}

/// Production database: `<appDocuments>/slab-mobile.db`, opened on first use.
AppDatabase openAppDatabase() => AppDatabase(
      LazyDatabase(() async {
        final dir = await getApplicationDocumentsDirectory();
        return NativeDatabase(File(p.join(dir.path, 'slab-mobile.db')));
      }),
    );
