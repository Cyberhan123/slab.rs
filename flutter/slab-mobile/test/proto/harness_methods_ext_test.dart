/// Request-frame and codec tests for the extended harness methods
/// (fork/rollback/compact, model/list, command/list, turn overrides) —
/// the wrappers added for the assistant port (slice B1).
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/proto/harness_client.dart';
import 'package:slab_mobile/proto/harness_methods.dart';

import 'fake_slab_socket.dart';

const baseUrl = 'http://127.0.0.1:3000';

const _threadBody = {
  'thread': {
    'id': 't2',
    'preview': 'forked',
    'modelProvider': 'local',
    'createdAt': 1700000000000,
    'turns': [],
  },
};

HarnessClient _client(FakeSlabSocket socket) =>
    HarnessClient(baseUrl: Uri.parse(baseUrl), sessionId: 's1', socketFactory: FakeSocketFactory([socket]).call);

void main() {
  tearDown(() {
    FakeSlabSocket.instanceCount = 0;
  });

  test('threadFork sends threadId + modelOverride and decodes the child thread', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => _threadBody);
    final client = _client(socket);
    final thread = await client.threadFork(threadId: 't1', modelOverride: 'qwen');
    expect(thread.id, 't2');
    expect(thread.preview, 'forked');
    final frame = socket.requests.last;
    expect(frame['method'], HarnessMethod.threadFork);
    expect(frame['params'], {'threadId': 't1', 'modelOverride': 'qwen'});
    await client.close();
  });

  test('threadFork omits modelOverride when unset', () async {
    final socket = FakeSlabSocket(onRequest: (_, _) => _threadBody);
    final client = _client(socket);
    await client.threadFork(threadId: 't1');
    expect(socket.requests.last['params'], {'threadId': 't1'});
    await client.close();
  });

  test('threadRollback sends toTurnId and decodes the thread', () async {
    final socket = FakeSlabSocket(onRequest: (_, _) => _threadBody);
    final client = _client(socket);
    final thread = await client.threadRollback(threadId: 't1', toTurnId: '3');
    expect(thread.id, 't2');
    expect(socket.requests.last['method'], HarnessMethod.threadRollback);
    expect(socket.requests.last['params'], {'threadId': 't1', 'toTurnId': '3'});
    await client.close();
  });

  test('threadCompactStart decodes counters alongside the thread', () async {
    final socket = FakeSlabSocket(
      onRequest: (_, _) => {
        ..._threadBody,
        'removedMessages': 12,
        'outputTokens': 3400,
      },
    );
    final client = _client(socket);
    final result = await client.threadCompactStart(threadId: 't1');
    expect(result.thread.id, 't2');
    expect(result.removedMessages, 12);
    expect(result.outputTokens, 3400);
    expect(socket.requests.last['method'], HarnessMethod.threadCompactStart);
    await client.close();
  });

  test('modelList decodes efforts and the cursor', () async {
    final socket = FakeSlabSocket(
      onRequest: (_, _) => {
        'data': [
          {
            'id': 'qwen3.5-9b',
            'model': 'slab-llama',
            'displayName': 'Qwen3.5 9B',
            'description': 'local default',
            'supportedReasoningEfforts': [
              {'reasoningEffort': 'low', 'description': 'fast'},
              {'reasoningEffort': 'high', 'description': 'thorough'},
            ],
            'defaultReasoningEffort': 'high',
            'isDefault': true,
          },
        ],
        'nextCursor': 'page2',
      },
    );
    final client = _client(socket);
    final models = await client.modelList();
    expect(models.data, hasLength(1));
    final model = models.data.single;
    expect(model.displayName, 'Qwen3.5 9B');
    expect(model.isDefault, isTrue);
    expect(model.defaultReasoningEffort, 'high');
    expect(model.supportedReasoningEfforts.map((e) => e.reasoningEffort), ['low', 'high']);
    expect(models.nextCursor, 'page2');
    await client.close();
  });

  test('commandList decodes kinds, sources, aliases, and control actions', () async {
    final socket = FakeSlabSocket(
      onRequest: (_, _) => {
        'data': [
          {
            'name': 'compact',
            'aliases': ['summarize'],
            'description': 'Compact the thread',
            'kind': 'control',
            'source': 'builtin',
            'controlAction': 'compact',
          },
          {
            'name': 'plan',
            'aliases': <String>[],
            'description': 'Plan mode',
            'kind': 'prompt',
            'source': 'skill',
          },
        ],
      },
    );
    final client = _client(socket);
    final commands = await client.commandList();
    expect(commands, hasLength(2));
    final compact = commands.first;
    expect(compact.kind.wire, 'control');
    expect(compact.controlAction, 'compact');
    expect(compact.aliases, ['summarize']);
    final plan = commands.last;
    expect(plan.kind.wire, 'prompt');
    expect(plan.source.wire, 'skill');
    expect(plan.controlAction, isNull);
    await client.close();
  });

  test('turnStart forwards turn-level overrides and omits unset ones', () async {
    late Map<String, Object?>? seenParams;
    final socket = FakeSlabSocket(
      onRequest: (method, params) {
        if (method == HarnessMethod.turnStart) seenParams = params;
        return const {};
      },
    );
    final client = _client(socket);
    await client.turnStart(
      threadId: 't1',
      input: [
        {'type': 'text', 'text': 'hi', 'textElements': <Object?>[]},
      ],
      model: 'slab-llama',
      effort: 'high',
      permissionMode: 'approve_for_me',
      agentType: 'plan',
    );
    expect(seenParams, {
      'threadId': 't1',
      'input': [
        {'type': 'text', 'text': 'hi', 'textElements': <Object?>[]},
      ],
      'model': 'slab-llama',
      'effort': 'high',
      'permissionMode': 'approve_for_me',
      'agentType': 'plan',
    });

    await client.turnStart(
      threadId: 't1',
      input: const [],
      model: 'slab-llama',
    );
    expect(seenParams, {
      'threadId': 't1',
      'input': const [],
      'model': 'slab-llama',
    });
    await client.close();
  });
}
