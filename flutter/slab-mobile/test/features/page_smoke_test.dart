/// Page smoke tests: each screen renders under the real TDesign theme with
/// faked transport. Assembly goes through a per-test get_it instance
/// (`asNewInstance`) — the same composition root `main()` uses in production.
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_it/get_it.dart';
import 'package:go_router/go_router.dart';
import 'package:slab_mobile/conversation/conversation_controller.dart';
import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/data/model_types.dart';
import 'package:slab_mobile/data/rest_client.dart';
import 'package:slab_mobile/features/assistant/model/model_cubit.dart';
import 'package:slab_mobile/features/assistant/model/model_repository.dart';
import 'package:slab_mobile/features/assistant/view/chat_screen.dart';
import 'package:slab_mobile/features/connect/connect_page.dart';
import 'package:slab_mobile/features/sessions/sessions_cubit.dart';
import 'package:slab_mobile/features/sessions/view/sessions_page.dart';
import 'package:slab_mobile/features/setup/setup_gate_page.dart';
import 'package:slab_mobile/l10n/catalog.dart';
import 'package:slab_mobile/theme/td_theme.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

/// Inert REST surface: no transport is ever touched (every method overridden).
class FakeSlabRestClient extends SlabRestClient {
  FakeSlabRestClient({this.setupInitialized = true, this.sessions = const []})
      : super(baseUrl: Uri.parse('http://127.0.0.1:9'));

  final bool setupInitialized;
  final List<SessionRecord> sessions;

  @override
  Future<HealthStatus> probeHealth() async => const HealthStatus(ok: true, version: 'test');

  @override
  Future<SetupStatus> getSetupStatus() async => SetupStatus(initialized: setupInitialized);

  @override
  Future<List<SessionRecord>> listSessions() async => sessions;

  @override
  Future<List<AiModelRecord>> listModels() async => const [];

  @override
  void dispose() {}
}

const _session = SessionRecord(
  id: 's1',
  name: 'Session One',
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-02T00:00:00Z',
);

TThemeData _tdTheme() {
  final json = File('assets/theme/tdesign-theme.json').readAsStringSync();
  return TThemeData.fromJson('slab', json, darkName: 'slabDark')!;
}

/// Loaded once in `setUpAll` — asset loads are real async I/O that can stall
/// under `testWidgets` FakeAsync, so tests only read the completed value.
late final Catalogs _catalogs;

/// Fresh get_it per test with the app-wide singletons; [client] doubles as
/// the transport fake seam (mirrors production wiring).
GetIt _configure({SlabRestClient? client}) {
  GetIt.asNewInstance();
  final getIt = GetIt.I;
  getIt.registerSingleton<Catalogs>(_catalogs);
  getIt.registerSingleton<TThemeData>(_tdTheme());
  getIt.registerSingleton<LocaleCubit>(LocaleCubit(catalogs: _catalogs));
  getIt.registerSingleton<ConnectionCubit>(ConnectionCubit(client: client));
  addTearDown(GetIt.I.reset);
  return getIt;
}

/// The pages under test navigate via go_router on user actions only, so a
/// minimal router keeps `context.go` functional even if a smoke test taps.
GoRouter _router(Widget home) => GoRouter(
      routes: [GoRoute(path: '/', builder: (_, _) => home)],
    );

/// Minimal shell: the production SlabApp wiring (theme + app-wide cubits)
/// around an arbitrary home page, minus the real router redirect ladder.
class SlabAppShell extends StatelessWidget {
  const SlabAppShell({super.key, required this.home});

  final Widget home;

  @override
  Widget build(BuildContext context) {
    final getIt = GetIt.I;
    return MultiBlocProvider(
      providers: [
        BlocProvider.value(value: getIt<LocaleCubit>()),
        BlocProvider.value(value: getIt<ConnectionCubit>()),
      ],
      child: MaterialApp.router(
        theme: buildSlabTdTheme(getIt<TThemeData>(), Brightness.light),
        darkTheme: buildSlabTdTheme(getIt<TThemeData>(), Brightness.dark),
        routerConfig: _router(home),
      ),
    );
  }
}

/// Fire chained zero-duration timers (EasyRefresh schedules a few) without
/// advancing into the pages' 3s/5s poll intervals. Never pumpAndSettle here —
/// the pages poll periodically and the chat controller reconnects.
Future<void> _drainTimers(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 60));
  }
}

void main() {
  setUpAll(() async {
    _catalogs = Catalogs(
      en: await SlabCatalog.loadDefault('en-US'),
      zh: await SlabCatalog.loadDefault('zh-CN'),
    );
  });

  testWidgets('connect page renders TDesign chrome', (tester) async {
    _configure();
    await tester.pumpWidget(const SlabAppShell(home: ConnectPage()));
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TInput), findsNWidgets(2));
    expect(find.byType(TButton), findsNWidgets(2));
    await _drainTimers(tester);
  });

  testWidgets('setup gate page renders TDesign chrome', (tester) async {
    _configure(client: FakeSlabRestClient(setupInitialized: false));
    await tester.pumpWidget(const SlabAppShell(home: SetupGatePage()));
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TLoading), findsOneWidget);
    expect(find.byType(TButton), findsOneWidget);
    await _drainTimers(tester);
  });

  testWidgets('sessions page renders cells and the swipe actions', (tester) async {
    _configure();
    final cubit = SessionsCubit(client: FakeSlabRestClient(sessions: const [_session]));
    await tester.pumpWidget(SlabAppShell(home: SessionsPage(cubit: cubit)));
    await tester.pump();
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TCell), findsOneWidget);
    expect(find.byType(TSwipeCell), findsOneWidget);
    expect(find.byType(TFab), findsOneWidget);
    expect(find.text('Session One'), findsOneWidget);
    await _drainTimers(tester);
    // Close inside the body (not addTearDown): the 5s poll timer must be
    // canceled before the FakeAsync pending-timer check, which runs before
    // registered teardowns.
    await cubit.close();
  });

  testWidgets('sessions page shows TEmpty for a fresh server', (tester) async {
    _configure();
    final cubit = SessionsCubit(client: FakeSlabRestClient(sessions: const []));
    await tester.pumpWidget(SlabAppShell(home: SessionsPage(cubit: cubit)));
    await tester.pump();
    await tester.pump();
    expect(find.byType(TEmpty), findsOneWidget);
    await _drainTimers(tester);
    await cubit.close();
  });

  testWidgets('chat screen renders navbar and composer', (tester) async {
    _configure();
    // An inert controller (never `start()`ed) keeps the WS stack out of the
    // test: the real page calls start(), whose `Future.timeout(5s)` timer
    // cannot be canceled from outside and would trip the binding's
    // pending-timer check under FakeAsync.
    final controller = ConversationController(
      sessionId: 's1',
      baseUrl: Uri.parse('http://127.0.0.1:9'),
    );
    addTearDown(controller.dispose);
    final modelCubit = ModelCubit(repository: ModelRepository(client: FakeSlabRestClient()));
    await tester.pumpWidget(
      SlabAppShell(
        home: ChatScreen(
          sessionId: 's1',
          sessionName: 'Session One',
          controller: controller,
          modelCubit: modelCubit,
        ),
      ),
    );
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TTextarea), findsOneWidget);
    expect(find.byType(TButton), findsOneWidget);
    expect(find.text('Session One'), findsOneWidget);
    await _drainTimers(tester);
  });
}
