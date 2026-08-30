import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/domain/conversation/conversation_controller.dart';
import 'package:slab_mobile/domain/conversation/turn_items.dart';
import 'package:slab_mobile/data/harness/harness_client.dart';
import 'package:slab_mobile/data/harness/harness_methods.dart';

import '../../data/harness/fake_slab_socket.dart';

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

  test('approval notifications queue, resolve, and mark denied on delivered=false', () async {
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

    // delivered=true → resolved and removed from pending.
    socket.onRequest = (method, params) => {'delivered': true};
    await controller.resolveApproval('c1', true);
    expect(controller.state.approvals, isEmpty);
    expect(controller.state.approvalStatusByItemId['c1']!.name, 'approved');

    // delivered=false → terminal denied (no pending entry exists server-side,
    // so a retry can never succeed) and the call never throws.
    socket.push(HarnessNotification.itemCommandExecutionRequestApproval, {
      'threadId': 't1',
      'turnId': '0',
      'itemId': 'c2',
      'command': 'cargo test',
      'cwd': '/repo',
      'allowedScopes': ['run_once'],
    });
    await pumpEventQueue();
    socket.onRequest = (method, params) => {'delivered': false, 'status': 'pending'};
    await controller.resolveApproval('c2', true);
    expect(controller.state.approvals, isEmpty);
    expect(controller.state.approvalStatusByItemId['c2']!.name, 'denied');
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

  test('restore exposes the user-message turn index for rollback affordances', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();
    expect(controller.state.userMessageTurnIndex, {'u1': 0});
    await controller.dispose();
  });

  test('restore fetches the command registry (fire-and-forget)', () async {
    final socket = FakeSlabSocket(
      onRequest: (method, params) {
        if (method == HarnessMethod.commandList) {
          return {
            'data': [
              {'name': 'compact', 'aliases': <String>[], 'description': '', 'kind': 'control', 'source': 'builtin', 'controlAction': 'compact'},
            ],
          };
        }
        return threadPayload();
      },
    );
    final controller = buildController(socket);
    await controller.start();
    await pumpEventQueue();
    expect(controller.state.commands.single.name, 'compact');
    await controller.dispose();
  });

  test('plan mode rides turn/start as agentType and clears on plan approval', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    controller.setPlanMode(true);
    expect(controller.state.planMode, isTrue);
    await controller.sendText('draft a plan');
    final turnStart = socket.requests.lastWhere((r) => r['method'] == HarnessMethod.turnStart);
    expect(turnStart['params'], containsPair('agentType', 'plan'));

    // A plan approval arrives; approving it clears plan mode.
    socket.push(HarnessNotification.itemCommandExecutionRequestApproval, {
      'threadId': 't1',
      'turnId': '1',
      'itemId': 'p1',
      'command': 'plan',
      'cwd': '/repo',
      'allowedScopes': ['run_once'],
      'planSnapshot': {'plan_id': 'x'},
    });
    await pumpEventQueue();
    expect(controller.state.approvals.single.kind.name, 'plan');

    await controller.resolveApproval('p1', true);
    await pumpEventQueue();
    expect(controller.state.planMode, isFalse);
    await controller.dispose();
  });

  test('send forwards images, effort, and permission mode', () async {
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

    await controller.send(
      text: 'look',
      imageUrls: ['data:image/png;base64,AAA'],
      effort: 'high',
      permissionMode: 'approve_for_me',
    );
    final turnStart = socket.requests.lastWhere((r) => r['method'] == HarnessMethod.turnStart);
    expect(turnStart['params'], {
      'threadId': 'tNew',
      'input': [
        {'type': 'text', 'text': 'look', 'textElements': []},
        {'type': 'image', 'imageUrl': 'data:image/png;base64,AAA', 'detail': 'auto'},
      ],
      'model': 'slab-llama',
      'effort': 'high',
      'permissionMode': 'approve_for_me',
    });
    // Optimistic user bubble carries the image part.
    expect(controller.state.messages.last.parts.whereType<ImageUiPart>(), isNotEmpty);
    await controller.dispose();
  });

  test('auto compaction markers: one compacting per thread, flip on done, drop on skipped', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => threadPayload());
    final controller = buildController(socket);
    await controller.start();

    socket.push(HarnessNotification.contextCompacting, {'threadId': 't1'});
    socket.push(HarnessNotification.contextCompacting, {'threadId': 't1'}); // dedup
    await pumpEventQueue();
    expect(controller.state.compactionMarkers, hasLength(1));
    expect(controller.state.compactionMarkers.single.phase, CompactionPhase.compacting);

    socket.push(HarnessNotification.contextCompacted, {'threadId': 't1', 'status': 'ok'});
    await pumpEventQueue();
    expect(controller.state.compactionMarkers.single.phase, CompactionPhase.compacted);

    // Next round: started but skipped → the in-progress marker disappears.
    socket.push(HarnessNotification.contextCompacting, {'threadId': 't1'});
    await pumpEventQueue();
    socket.push(HarnessNotification.contextCompacted, {'threadId': 't1', 'status': 'skipped'});
    await pumpEventQueue();
    expect(
      controller.state.compactionMarkers.where((m) => m.phase == CompactionPhase.compacting),
      isEmpty,
    );
    await controller.dispose();
  });

  test('compactThread flips the manual marker and rebinds the compacted history', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) {
      if (method == HarnessMethod.threadCompactStart) {
        return {'thread': threadPayload()['thread'], 'removedMessages': 3, 'outputTokens': 120};
      }
      return threadPayload();
    });
    final controller = buildController(socket);
    await controller.start();

    await controller.compactThread();
    expect(socket.countRequests(HarnessMethod.threadCompactStart), 1);
    expect(controller.state.compactionMarkers.single.mode, CompactionMode.manual);
    expect(controller.state.compactionMarkers.single.phase, CompactionPhase.compacted);
    expect(controller.state.isCompacting, isFalse);
    expect(controller.state.actionError, isNull);
    await controller.dispose();
  });

  test('forkThread rebinds to the child and bumps restoreVersion', () async {
    Map<String, Object?> childPayload() => {
          'thread': {
            'id': 'tChild',
            'preview': 'p',
            'modelProvider': 'llama',
            'createdAt': 2,
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
                ],
              },
            ],
          },
        };
    final socket = FakeSlabSocket(
      onRequest: (method, params) {
        if (method == HarnessMethod.threadFork) return childPayload();
        if (method == HarnessMethod.threadResume) return childPayload();
        return threadPayload();
      },
    );
    final controller = buildController(socket);
    await controller.start();
    expect(controller.state.restoreVersion, 0);

    await controller.forkThread();
    expect(controller.client.currentThreadId, 'tChild');
    expect(controller.state.threadId, 'tChild');
    expect(controller.state.restoreVersion, 1);
    expect(controller.state.isForking, isFalse);
    await controller.dispose();
  });

  test('rollbackFromTurn retracts the turn and later ones, then re-resumes', () async {
    // After rollback to turn 0 the thread payload loses turn 1.
    Map<String, Object?> rolledBackPayload() {
      final thread = threadPayload()['thread']! as Map<String, Object?>;
      final turns = thread['turns']! as List;
      return {
        'thread': {
          'id': 't1',
          'preview': 'p',
          'modelProvider': 'llama',
          'createdAt': 1,
          'turns': [turns[0]],
        },
      };
    }
    final socket = FakeSlabSocket(
      onRequest: (method, params) => method == HarnessMethod.threadResume ? rolledBackPayload() : threadPayload(),
    );
    final controller = buildController(socket);
    await controller.start();
    expect(controller.state.userMessageTurnIndex, {'u1': 0});

    await controller.rollbackFromTurn(1);
    final rollbackFrame = socket.requests.lastWhere((r) => r['method'] == HarnessMethod.threadRollback);
    expect(rollbackFrame['params'], {'threadId': 't1', 'toTurnId': '0'});
    expect(controller.state.messages, hasLength(2)); // turn 1's partial message is gone
    expect(controller.state.restoreVersion, 1);
    expect(controller.state.isRollingBack, isFalse);

    // Turn 0 is a no-op (never retract the very first turn).
    final requestsBefore = socket.requests.length;
    await controller.rollbackFromTurn(0);
    expect(socket.requests.length, requestsBefore);
    await controller.dispose();
  });
}
