/// Shell layout tests: the two-tab StatefulShellRoute (TTabBar), branch
/// switching, and the chat route staying outside the shell (no tab bar).
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_it/get_it.dart';
import 'package:go_router/go_router.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:slab_mobile/app.dart';
import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:drift/native.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/data/local/app_database.dart';
import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/data/rest/settings_types.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/routes/app_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

/// Inert REST surface (same pattern as the page smoke tests).
class FakeSlabRestClient extends SlabRestClient {
  FakeSlabRestClient({this.sessions = const []}) : super(baseUrl: Uri.parse('http://127.0.0.1:9'));

  final List<SessionRecord> sessions;

  @override
  Future<HealthStatus> probeHealth() async => const HealthStatus(ok: true, version: 'test');

  @override
  Future<SetupStatus> getSetupStatus() async => const SetupStatus(initialized: true);

  final List<SessionRecord> _created = [];
  int createCalls = 0;

  @override
  Future<List<SessionRecord>> listSessions() async => [...sessions, ..._created];

  @override
  Future<SettingsDocumentView> getSettingsDocument() async =>
      const SettingsDocumentView(schemaVersion: 1, settingsPath: '');

  @override
  Future<SessionRecord> createSession({String? name}) async {
    createCalls += 1;
    final record = SessionRecord(
      id: 'created-$createCalls',
      name: 'New assistant',
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
    );
    _created.add(record);
    return record;
  }

  @override
  void dispose() {}
}

const _session = SessionRecord(
  id: 's1',
  name: 'Session One',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-02T00:00:00Z',
);

const Catalogs _catalogs = defaultCatalogs;

TThemeData _tdTheme() {
  final json = File('assets/theme/tdesign-theme.json').readAsStringSync();
  return TThemeData.fromJson('slab', json, darkName: 'slabDark')!;
}

/// App-wide singletons plus a pre-configured connection (saved baseUrl in
/// mock prefs) so the router redirect lands on the shell instead of /connect.
Future<ConnectionCubit> _configure({SlabRestClient? client}) async {
  SharedPreferences.setMockInitialValues({'slab.mobile.connection.baseUrl': 'http://127.0.0.1:9'});
  GetIt.asNewInstance();
  final getIt = GetIt.I;
  getIt.registerSingleton<TThemeData>(_tdTheme());
  getIt.registerSingleton<LocaleCubit>(LocaleCubit(catalogs: _catalogs));
  final database = AppDatabase(NativeDatabase.memory());
  getIt.registerSingleton<AppDatabase>(database);
  getIt.registerSingleton<SessionMetaDao>(SessionMetaDao(database));
  addTearDown(database.close);
  final connection = ConnectionCubit(client: client ?? FakeSlabRestClient(sessions: const [_session]));
  getIt.registerSingleton<ConnectionCubit>(connection);
  addTearDown(GetIt.I.reset);
  await connection.load();
  return connection;
}

void main() {
  testWidgets('shell shows both tabs and switches branches', (tester) async {
    final connection = await _configure();
    await tester.pumpWidget(SlabApp(router: buildAppRouter(connection: connection)));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    // Sessions branch is live: tab label + navbar title (same copy) and one
    // session cell.
    expect(find.byType(TTabBar), findsOneWidget);
    expect(find.text('Conversations'), findsNWidgets(2));
    expect(find.text('Session One'), findsOneWidget);
    // Settings is only the tab label so far (its navbar is offstage).
    expect(find.text('Settings'), findsOneWidget);

    await tester.tap(find.text('Settings'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    // Sessions navbar went offstage (its tab label stays); the settings
    // navbar joined its tab label; the empty document renders TEmpty.
    expect(find.text('Conversations'), findsNWidgets(1));
    expect(find.text('Settings'), findsNWidgets(2));
    expect(find.byType(TEmpty), findsOneWidget);

    // Unmount inside the body so the shell's BlocProviders finish their
    // async close while the fake clock can still flush pending microtasks.
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 50));
  });

  testWidgets('zh preference renders Chinese chrome (locale plumbing e2e)', (tester) async {
    final connection = await _configure();
    // set() goes through the mocked shared_preferences (wire format 'zh-CN').
    await GetIt.I<LocaleCubit>().set(SlabLanguagePreference.zhCn);
    await tester.pumpWidget(SlabApp(router: buildAppRouter(connection: connection)));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    // Tab label + navbar title resolve the zh mobile-only strings; session
    // content is untouched by the locale switch.
    expect(find.text('会话'), findsNWidgets(2));
    expect(find.text('Session One'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 50));
  });

  testWidgets('shell body clears the status-bar inset (global SafeArea)', (tester) async {
    final connection = await _configure();
    tester.view
      ..devicePixelRatio = 1.0
      ..padding = FakeViewPadding(top: 44, bottom: 34, left: 0, right: 0);
    addTearDown(tester.view.reset);
    await tester.pumpWidget(SlabApp(router: buildAppRouter(connection: connection)));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    // The shell's SafeArea pushes the branch navbar below the 44px status-bar
    // inset (the bottom inset is TTabBar's own job, asserted via the tab bar
    // rendering below the body).
    final navbarTopLeft = tester.getTopLeft(find.byType(TNavBar).first);
    expect(navbarTopLeft.dy, greaterThanOrEqualTo(44));

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 50));
  });

  test('chat route stays outside the shell (full-screen, no tab bar)', () async {
    final connection = await _configure();
    final router = buildAppRouter(connection: connection);

    final shellRoute = router.configuration.routes.whereType<StatefulShellRoute>().single;
    final branchPaths = shellRoute.branches
        .map((branch) => (branch.routes.single as GoRoute).path)
        .toList(growable: false);
    expect(branchPaths, ['/sessions', '/settings']);

    final topLevelPaths = router.configuration.routes.whereType<GoRoute>().map((route) => route.path);
    expect(topLevelPaths, contains('/chat/:sessionId'));
  });
}
