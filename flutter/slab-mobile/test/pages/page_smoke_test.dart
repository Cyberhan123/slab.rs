/// Page smoke tests: each screen renders under the real TDesign theme with
/// faked transport. These exist because the UI-library swap had no widget
/// safety net (all prior tests were pure unit tests).
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:slab_mobile/app_providers.dart';
import 'package:slab_mobile/conversation/conversation_controller.dart';
import 'package:slab_mobile/data/rest_client.dart';
import 'package:slab_mobile/l10n/catalog.dart';
import 'package:slab_mobile/pages/chat_page.dart';
import 'package:slab_mobile/pages/connect_page.dart';
import 'package:slab_mobile/pages/sessions_page.dart';
import 'package:slab_mobile/pages/setup_gate_page.dart';
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

ProviderContainer _container({SlabRestClient? restClient}) {
  return ProviderContainer(
    overrides: [
      catalogsProvider.overrideWithValue(_catalogs),
      slabTdThemeProvider.overrideWithValue(_tdTheme()),
      if (restClient != null) restClientProvider.overrideWithValue(restClient),
    ],
  );
}

/// The pages under test navigate via go_router on user actions only, so a
/// minimal router keeps `context.go` functional even if a smoke test taps.
GoRouter _router(Widget home) => GoRouter(
      routes: [GoRoute(path: '/', builder: (_, _) => home)],
    );

/// Minimal shell: the production SlabApp wiring (theme) around an arbitrary
/// home page, minus the real router redirect ladder.
class SlabAppShell extends StatelessWidget {
  const SlabAppShell({super.key, required this.home});

  final Widget home;

  @override
  Widget build(BuildContext context) {
    return Consumer(
      builder: (context, ref, _) {
        final td = ref.watch(slabTdThemeProvider);
        return MaterialApp.router(
          theme: buildSlabTdTheme(td, Brightness.light),
          darkTheme: buildSlabTdTheme(td, Brightness.dark),
          routerConfig: _router(home),
        );
      },
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
    final container = _container();
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: SlabAppShell(home: const ConnectPage())),
    );
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TInput), findsNWidgets(2));
    expect(find.byType(TButton), findsNWidgets(2));
    await _drainTimers(tester);
  });

  testWidgets('setup gate page renders TDesign chrome', (tester) async {
    final container = _container(restClient: FakeSlabRestClient(setupInitialized: false));
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: SlabAppShell(home: const SetupGatePage())),
    );
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TLoading), findsOneWidget);
    expect(find.byType(TButton), findsOneWidget);
    await _drainTimers(tester);
  });

  testWidgets('sessions page renders cells and the swipe actions', (tester) async {
    final container = _container(restClient: FakeSlabRestClient(sessions: const [_session]));
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: SlabAppShell(home: const SessionsPage())),
    );
    await tester.pump();
    await tester.pump();
    expect(find.byType(TNavBar), findsOneWidget);
    expect(find.byType(TCell), findsOneWidget);
    expect(find.byType(TSwipeCell), findsOneWidget);
    expect(find.byType(TFab), findsOneWidget);
    expect(find.text('Session One'), findsOneWidget);
    await _drainTimers(tester);
  });

  testWidgets('sessions page shows TEmpty for a fresh server', (tester) async {
    final container = _container(restClient: FakeSlabRestClient(sessions: const []));
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: SlabAppShell(home: const SessionsPage())),
    );
    await tester.pump();
    await tester.pump();
    expect(find.byType(TEmpty), findsOneWidget);
    await _drainTimers(tester);
  });

  testWidgets('chat page renders navbar and composer', (tester) async {
    // An inert controller (never `start()`ed) keeps the WS stack out of the
    // test: the real provider calls start(), whose `Future.timeout(5s)`
    // timer cannot be canceled from outside and would trip the binding's
    // pending-timer check under FakeAsync.
    final controller = ConversationController(
      sessionId: 's1',
      baseUrl: Uri.parse('http://127.0.0.1:9'),
    );
    addTearDown(controller.dispose);
    final container = ProviderContainer(
      overrides: [
        catalogsProvider.overrideWithValue(_catalogs),
        slabTdThemeProvider.overrideWithValue(_tdTheme()),
        conversationControllerProvider('s1').overrideWithValue(controller),
      ],
    );
    addTearDown(container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const SlabAppShell(home: ChatPage(sessionId: 's1', sessionName: 'Session One')),
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
