/// Composition root (get_it). Single file, sectioned registration:
/// app singletons (catalogs/theme) → app-wide cubits → infra factories.
///
/// Rules of the house:
/// - get_it holds infra + app-wide cubits only. Screen-level cubits are
///   constructed in `BlocProvider(create:)` inside the router builders and
///   take their repositories from get_it — no scattered service lookups in
///   widget trees.
/// - `getIt` is a getter (not a captured field) so `GetIt.asNewInstance()`
///   in tests is honored by already-imported libraries.
library;

import 'package:get_it/get_it.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart' show TThemeData;

import '../../l10n/catalog.dart';
import '../app/connection_cubit.dart';
import '../app/locale_cubit.dart';
import '../db/app_database.dart';
import '../db/drafts_dao.dart';
import '../db/session_meta_dao.dart';

GetIt get getIt => GetIt.instance;

/// Loads persisted preferences (language, connection) so the router redirect
/// runs against real state; call exactly once from `main()` before `runApp`.
Future<void> configureDependencies({required Catalogs catalogs, required TThemeData tdTheme}) async {
  getIt.registerSingleton<Catalogs>(catalogs);
  getIt.registerSingleton<TThemeData>(tdTheme);

  final locale = LocaleCubit(catalogs: catalogs);
  await locale.load();
  getIt.registerSingleton<LocaleCubit>(locale);

  final connection = ConnectionCubit();
  await connection.load();
  getIt.registerSingleton<ConnectionCubit>(connection);

  final db = openAppDatabase();
  getIt.registerSingleton<AppDatabase>(db);
  getIt.registerSingleton<SessionMetaDao>(SessionMetaDao(db));
  getIt.registerSingleton<DraftsDao>(DraftsDao(db));
}
