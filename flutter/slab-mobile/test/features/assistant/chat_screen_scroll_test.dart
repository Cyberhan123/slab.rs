/// Chat screen scroll-to-end fab (the inverted `TBackTop`): hidden while
/// pinned to the bottom, revealed when the user scrolls away, tap returns to
/// the end AND re-engages streaming auto-follow.
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
import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/data/rest/model_types.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/features/assistant/models/model_cubit.dart';
import 'package:slab_mobile/features/assistant/models/model_repository.dart';
import 'package:slab_mobile/features/assistant/view/chat_screen.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/theme/td_theme.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../data/harness/fake_slab_socket.dart';

/// Inert REST surface for the model cubit (no transport touched).
class _FakeSlabRestClient extends SlabRestClient {
  _FakeSlabRestClient() : super(baseUrl: Uri.parse('http://127.0.0.1:9'));

  @override
  Future<List<AiModelRecord>> listModels() async => const [];

  @override
  void dispose() {}
}

/// A thread long enough to overflow the test surface, with a trailing
/// in-progress turn so live deltas still flow after restore.
Map<String, Object?> longThreadPayload({int turns = 30}) => {
      'thread': {
        'id': 't1',
        'preview': 'p',
        'modelProvider': 'llama',
        'createdAt': 1,
        'turns': [
          for (var i = 0; i < turns; i++)
            {
              'id': '$i',
              'status': 'completed',
              'items': [
                {
                  'type': 'userMessage',
                  'id': 'u$i',
                  'content': [
                    {'type': 'text', 'text': 'question number $i'},
                  ],
                },
                {
                  'type': 'agentMessage',
                  'id': 'a$i',
                  'text': 'answer number $i with enough body text to give the row height',
                },
              ],
            },
          {
            'id': '$turns',
            'status': 'inProgress',
            'items': [
              {'type': 'agentMessage', 'id': 'live', 'text': 'partial'},
            ],
          },
        ],
      },
    };

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

TThemeData _tdTheme() {
  final json = File('assets/theme/tdesign-theme.json').readAsStringSync();
  return TThemeData.fromJson('slab', json, darkName: 'slabDark')!;
}

const Catalogs _catalogs = defaultCatalogs;

class _Shell extends StatelessWidget {
  const _Shell({required this.home});

  final Widget home;

  @override
  Widget build(BuildContext context) {
    final getIt = GetIt.I;
    return MultiBlocProvider(
      providers: [
        BlocProvider.value(value: getIt<LocaleCubit>()),
        BlocProvider.value(value: getIt<ConnectionCubit>()),
      ],
      child: MaterialApp(
        theme: buildSlabTdTheme(getIt<TThemeData>(), Brightness.light),
        home: home,
      ),
    );
  }
}

double _fabOpacity(WidgetTester tester) => tester
    .widget<AnimatedOpacity>(find.ancestor(
      of: find.byType(TBackTop),
      matching: find.byType(AnimatedOpacity),
    ))
    .opacity;

/// Each pump delivers at most one ticker tick, and an animation's FIRST tick
/// always applies t=0 — so scroll animations need several small pumps to
/// progress, never one big jump.
Future<void> _pumpScrollAnimation(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

void main() {
  setUp(() {
    GetIt.asNewInstance();
    final getIt = GetIt.I;
    getIt.registerSingleton<TThemeData>(_tdTheme());
    getIt.registerSingleton<LocaleCubit>(LocaleCubit(catalogs: _catalogs));
    final database = AppDatabase(NativeDatabase.memory());
    getIt.registerSingleton<AppDatabase>(database);
    getIt.registerSingleton<SessionMetaDao>(SessionMetaDao(database));
    addTearDown(database.close);
    getIt.registerSingleton<ConnectionCubit>(ConnectionCubit());
    addTearDown(GetIt.I.reset);
    FakeSlabSocket.instanceCount = 0;
  });

  testWidgets('scroll-to-end fab hides at the bottom, shows when scrolled away, '
      'and re-engages auto-follow on tap', (tester) async {
    final socket = FakeSlabSocket(onRequest: (method, params) => longThreadPayload());
    final controller = buildController(socket);
    await controller.start();
    addTearDown(controller.dispose);

    final modelCubit = ModelCubit(repository: ModelRepository(client: _FakeSlabRestClient()));
    await tester.pumpWidget(
      _Shell(home: ChatScreen(sessionId: 's1', sessionName: 'S', controller: controller, modelCubit: modelCubit)),
    );
    // Initial frame + the 200ms auto-scroll animation to the bottom.
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final scrollable = find.byType(ListView);
    final scroll = tester.state<ScrollableState>(find.descendant(
      of: scrollable,
      matching: find.byType(Scrollable),
    ));
    // ListView.builder's maxScrollExtent is an estimate until every child has
    // been built, so "at the end" is asserted as distance-from-bottom (the
    // fab's own trigger metric), not pixel equality.
    double distanceFromEnd() =>
        scroll.position.maxScrollExtent - scroll.position.pixels;

    expect(distanceFromEnd(), lessThan(120),
        reason: 'initial auto-scroll pins to the bottom');
    expect(_fabOpacity(tester), 0.0, reason: 'fab hidden while at the bottom');

    // Scroll away from the bottom (finger drags down).
    await tester.drag(scrollable, const Offset(0, 400));
    await tester.pump();
    expect(distanceFromEnd(), greaterThan(120),
        reason: 'drag moved the viewport away from the end');
    expect(_fabOpacity(tester), 1.0, reason: 'fab revealed when distance >= 120');

    // Tap the fab: scrolls back to the end and hides again.
    await tester.tap(find.byType(TBackTop));
    await tester.pump();
    await _pumpScrollAnimation(tester);
    expect(distanceFromEnd(), lessThan(120), reason: 'fab tap scrolls to the end');
    expect(_fabOpacity(tester), 0.0, reason: 'fab hides once back at the end');

    // Auto-follow re-engaged: a live delta grows the list and the view follows.
    socket.push(HarnessNotification.itemAgentMessageDelta, {
      'threadId': 't1',
      'turnId': '30',
      'itemId': 'live',
      'delta': ' and a lot more streamed content that makes the row taller and the list longer',
    });
    await tester.pump();
    await _pumpScrollAnimation(tester);
    expect(distanceFromEnd(), lessThan(120),
        reason: 'streaming delta keeps following after the fab tap');
  });
}
