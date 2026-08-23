/// Deep-think / reasoning-effort menu: desktop sender semantics — the chip
/// opens a bottom sheet (switch + level picker), every change commits
/// immediately, and `turn/start` always carries an explicit `effort`
/// (`'off'` by default; the legacy null "Auto" is normalized on draft
/// restore).
library;

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get_it/get_it.dart';
import 'package:drift/native.dart';
import 'package:slab_mobile/domain/conversation/conversation_controller.dart';
import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/data/harness/harness_client.dart';
import 'package:slab_mobile/data/harness/harness_methods.dart';
import 'package:slab_mobile/data/local/app_database.dart';
import 'package:slab_mobile/data/local/drafts_dao.dart';
import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/data/rest/model_types.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/features/assistant/models/model_cubit.dart';
import 'package:slab_mobile/features/assistant/models/model_repository.dart';
import 'package:slab_mobile/features/assistant/view/widgets/composer/composer_bar.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/theme/td_theme.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../data/harness/fake_slab_socket.dart';

/// Inert REST surface for the model cubit.
class _FakeSlabRestClient extends SlabRestClient {
  _FakeSlabRestClient() : super(baseUrl: Uri.parse('http://127.0.0.1:9'));

  @override
  Future<List<AiModelRecord>> listModels() async => const [];

  @override
  void dispose() {}
}

TThemeData _tdTheme() {
  final json = File('assets/theme/tdesign-theme.json').readAsStringSync();
  return TThemeData.fromJson('slab', json, darkName: 'slabDark')!;
}

const Catalogs _catalogs = defaultCatalogs;

/// Fresh get_it with the composer's DAOs; returns the DraftsDao for seeding.
DraftsDao _configure() {
  GetIt.asNewInstance();
  final getIt = GetIt.I;
  getIt.registerSingleton<TThemeData>(_tdTheme());
  getIt.registerSingleton<LocaleCubit>(LocaleCubit(catalogs: _catalogs));
  final database = AppDatabase(NativeDatabase.memory());
  getIt.registerSingleton<AppDatabase>(database);
  getIt.registerSingleton<SessionMetaDao>(SessionMetaDao(database));
  final drafts = DraftsDao(database);
  getIt.registerSingleton<DraftsDao>(drafts);
  addTearDown(database.close);
  getIt.registerSingleton<ConnectionCubit>(ConnectionCubit());
  addTearDown(GetIt.I.reset);
  return drafts;
}

ConversationController buildController(FakeSlabSocket socket) {
  final client = HarnessClient(
    baseUrl: Uri.parse('http://127.0.0.1:3000'),
    sessionId: 's1',
    socketFactory: FakeSocketFactory([socket]).call,
    backoffBase: const Duration(milliseconds: 1),
  );
  return ConversationController(
      sessionId: 's1', baseUrl: Uri.parse('http://127.0.0.1:3000'), client: client);
}

/// Fresh-thread socket: resume fails ("no thread"), thread/start mints a new id.
FakeSlabSocket freshSocket() => FakeSlabSocket(
      onRequest: (method, params) {
        if (method == HarnessMethod.threadStart) {
          return {
            'thread': {
              'id': 'tNew',
              'preview': '',
              'modelProvider': '',
              'createdAt': 0,
              'turns': <Object?>[],
            },
          };
        }
        return const {};
      },
    )..failWith = (method) => method == HarnessMethod.threadResume ? 'no thread to resume' : null;

Widget _shell(
  WidgetTester tester,
  ConversationController controller,
  FakeSlabSocket socket, {
  String sessionId = 's1',
}) {
  final getIt = GetIt.I;
  return MultiBlocProvider(
    providers: [
      BlocProvider.value(value: getIt<LocaleCubit>()),
      BlocProvider.value(value: getIt<ConnectionCubit>()),
      BlocProvider<ModelCubit>.value(
        value: ModelCubit(repository: ModelRepository(client: _FakeSlabRestClient())),
      ),
    ],
    child: MaterialApp(
      theme: buildSlabTdTheme(getIt<TThemeData>(), Brightness.light),
      home: Scaffold(
        body: Align(
          alignment: Alignment.bottomCenter,
          // Keyed by session so a re-pumped shell with a different session
          // builds fresh state (initState re-runs the draft restore).
          child: ComposerBar(
            key: ValueKey(sessionId),
            controller: controller,
            sessionId: sessionId,
            locale: getIt<LocaleCubit>().resolvedTag,
            catalog: getIt<LocaleCubit>().catalog,
          ),
        ),
      ),
    ),
  );
}

/// Type text, hit send, and return the effort riding the emitted turn/start.
Future<Object?> _sendAndReadEffort(WidgetTester tester, FakeSlabSocket socket) async {
  await tester.enterText(find.byType(EditableText), 'hello');
  await tester.pump();
  await tester.tap(find.byIcon(TIcons.send));
  // The send chain (WS open → thread/start → turn/start) resolves across a
  // couple of microtask-draining pumps.
  await tester.pump(const Duration(milliseconds: 20));
  await tester.pump(const Duration(milliseconds: 20));
  final turnStart = socket.requests.lastWhere((r) => r['method'] == HarnessMethod.turnStart);
  return (turnStart['params'] as Map<String, Object?>)['effort'];
}

void main() {
  testWidgets('default send carries effort=off and the chip reads Deep think · Off',
      (tester) async {
    _configure();
    final socket = freshSocket();
    final controller = buildController(socket);
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(tester, controller, socket));
    await tester.pump();

    expect(find.text('Deep think · Off'), findsOneWidget);
    expect(await _sendAndReadEffort(tester, socket), 'off');
  });

  testWidgets('the menu commits switch + level and the chip reflects the level',
      (tester) async {
    _configure();
    final socket = freshSocket();
    final controller = buildController(socket);
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(tester, controller, socket));
    await tester.pump();

    // Second chip (plan, DEEP THINK, permission) opens the reasoning sheet.
    await tester.tap(find.byType(TTag).at(1));
    await tester.pump();
    // Let the modal sheet's entrance animation finish before hitting rows.
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(TSwitch), findsOneWidget);

    // Flipping the switch alone keeps the default (high) level.
    await tester.tap(find.byType(TSwitch));
    await tester.pump();
    expect(find.text('Deep think · High'), findsOneWidget);

    // Tapping a level selects it (and keeps deep think on).
    await tester.tap(find.text('Medium'));
    await tester.pump();

    // Dismiss the sheet by tapping the scrim; changes must survive.
    await tester.tapAt(const Offset(5, 5));
    await tester.pump();
    // Let the sheet's exit animation finish removing the route.
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(TSwitch), findsNothing);
    expect(find.text('Deep think · Medium'), findsOneWidget);

    expect(await _sendAndReadEffort(tester, socket), 'medium');
  });

  testWidgets('draft restore normalizes the legacy effort values', (tester) async {
    final drafts = _configure();

    // A stored explicit level re-enables deep think at that level.
    await drafts.save(
        sessionId: 's1', content: '', planMode: false, effort: 'medium', permissionMode: null);
    var socket = freshSocket();
    var controller = buildController(socket);
    addTearDown(controller.dispose);
    await tester.pumpWidget(_shell(tester, controller, socket));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(find.text('Deep think · Medium'), findsOneWidget);
    expect(await _sendAndReadEffort(tester, socket), 'medium');

    // A legacy null draft ("Auto") reads as off.
    await drafts.save(
        sessionId: 's2', content: '', planMode: false, effort: null, permissionMode: null);
    socket = freshSocket();
    controller = buildController(socket);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      _shell(tester, controller, socket, sessionId: 's2'),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 20));
    expect(find.text('Deep think · Off'), findsOneWidget);
    expect(await _sendAndReadEffort(tester, socket), 'off');
  });
}
