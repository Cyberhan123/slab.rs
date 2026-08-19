import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/conversation/conversation_controller.dart';
import 'package:slab_mobile/proto/harness_client.dart';
import 'package:slab_mobile/proto/harness_methods.dart';

import '../proto/fake_slab_socket.dart';

Map<String, Object?> threadPayload() => {
      'thread': {
        'id': 't1',
        'preview': 'p',
        'modelProvider': 'llama',
        'createdAt': 1,
        'turns': [
          {
            'id': '0',
            'status': 'completed',
            'items': [
              {
                'type': 'userMessage',
                'id': 'u1',
                'content': [
                  {'type': 'text', 'text': 'hello'},
                ],
              },
              {'type': 'agentMessage', 'id': 'a1', 'text': 'hi there'},
            ],
          },
          {
            'id': '1',
            'status': 'inProgress',
            'items': [
              {'type': 'agentMessage', 'id': 'a2', 'text': 'partial'},
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
  return ConversationController(sessionId: 's1', baseUrl: Uri.parse('http://127.0.0.1:3000'), client: client);
}

void main() {
  tearDown(() {
    FakeSlabSocket.instanceCount = 0;
  });

  test('restore: resume projects history and binds the thread', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    expect(controller.state.threadId, 't1');
    expect(controller.state.messages.length, 2);
    expect(controller.state.messages[0].fromUser, isTrue);
    expect(controller.client.currentThreadId, 't1');
    // Turn 1 is inProgress → excluded from the live threshold so its deltas flow.
    expect(controller.client.lastTurnIndex, 0);

    await controller.dispose();
  });

  test('restore: "no thread to resume" starts a fresh session (not an error)', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload())
      ..failWith = (method) => method == HarnessMethod.threadResume ? 'no thread to resume' : null;
    final controller = buildController(socket);
    await controller.start();

    expect(controller.state.error, isNull);
    expect(controller.state.messages, isEmpty);
    expect(controller.state.threadId, isNull);
    expect(controller.client.lastTurnIndex, -1);
    await controller.dispose();
  });

  test('restore: a real resume failure surfaces as state.error', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload())
      ..failWith = (method) => method == HarnessMethod.threadResume ? 'thread corrupted' : null;
    final controller = buildController(socket);
    await controller.start();

    expect(controller.state.error, contains('thread corrupted'));
    expect(controller.state.messages, isEmpty);
    await controller.dispose();
  });

  test('sendText lazily binds a thread and fires turn/start with text input', () async {
    final socket = FakeSlabSocket(
      onRequest: (method, params) {
        if (method == HarnessMethod.threadStart) {
          return {
            'thread': {'id': 'tNew', 'preview': '', 'modelProvider': '', 'createdAt': 0, 'turns': []},
          };
        }
        return const {};
      },
    )..failWith = (method) => method == HarnessMethod.threadResume ? 'no thread to resume' : null;
    final controller = buildController(socket);
    await controller.start();
    expect(controller.state.threadId, isNull);

    await controller.sendText('ping');

    expect(controller.client.currentThreadId, 'tNew');
    final turnStart = socket.requests.lastWhere((r) => r['method'] == HarnessMethod.turnStart);
    expect(turnStart['params'], {
      'threadId': 'tNew',
      'input': [
        {'type': 'text', 'text': 'ping', 'textElements': []},
      ],
      'model': 'slab-llama',
    });
    // Optimistic user bubble appended.
    expect(controller.state.messages.last.fromUser, isTrue);
    await controller.dispose();
  });

  test('approval notifications queue, resolve, and revert on delivered=false', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    socket.push(HarnessNotification.itemCommandExecutionRequestApproval, {
      'threadId': 't1',
      'turnId': '0',
      'itemId': 'c1',
      'command': 'cargo build',
      'cwd': '/repo',
      'allowedScopes': ['run_once'],
    });
    await pumpEventQueue();
    expect(controller.state.approvals.length, 1);
    expect(controller.state.approvals.single.command, 'cargo build');

    // delivered=false → revert to pending.
    socket.onRequest = (method, params) => {'delivered': false, 'status': 'pending'};
    await expectLater(controller.resolveApproval('c1', true), throwsStateError);
    expect(controller.state.approvals.single.status.name, 'pending');

    // delivered=true → resolved and removed from pending.
    socket.onRequest = (method, params) => {'delivered': true};
    await controller.resolveApproval('c1', true);
    expect(controller.state.approvals, isEmpty);
    expect(controller.state.approvalStatusByItemId['c1']!.name, 'approved');
    await controller.dispose();
  });

  test('file-change approvals decode changes and scopes', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    socket.push(HarnessNotification.itemFileChangeRequestApproval, {
      'threadId': 't1',
      'turnId': '0',
      'itemId': 'f1',
      'allowedScopes': ['run_once', 'always_in_workspace', 'unknown_scope'],
      'changes': [
        {'path': 'a.rs', 'type': 'edit', 'diff': '@@ -1 +1 @@'},
      ],
    });
    await pumpEventQueue();
    final approval = controller.state.approvals.single;
    expect(approval.itemId, 'f1');
    expect(approval.changes.single.path, 'a.rs');
    // Unknown wire scopes are dropped, known ones preserved in order.
    expect(approval.allowedScopes.map((s) => s.wire), ['run_once', 'always_in_workspace']);
    await controller.dispose();
  });

  test('model/load notifications drive the loading phase', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    socket.push(HarnessNotification.modelLoadDelta, {
      'threadId': 't1',
      'phase': 'downloading',
      'modelId': 'qwen',
      'downloadedBytes': 10,
      'totalBytes': 100,
    });
    await pumpEventQueue();
    expect(controller.state.turnPhase, TurnPhase.modelLoading);
    expect(controller.state.modelLoad?.downloadedBytes, 10);

    socket.push(HarnessNotification.modelLoadCompleted, {'threadId': 't1', 'modelId': 'qwen', 'status': 'ready'});
    await pumpEventQueue();
    expect(controller.state.modelLoad, isNull);
    await controller.dispose();
  });
}
