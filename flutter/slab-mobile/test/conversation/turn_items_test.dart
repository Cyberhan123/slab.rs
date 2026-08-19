import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/conversation/turn_items.dart';
import 'package:slab_mobile/proto/harness_methods.dart';
import 'package:slab_mobile/proto/harness_types.dart' as proto;
import 'package:slab_mobile/proto/json_rpc.dart';

const base = 'http://127.0.0.1:3000';
final baseUri = Uri.parse(base);

NotificationFrame frame(String method, [Map<String, Object?>? params]) =>
    NotificationFrame(method: method, params: params);

void main() {
  group('stripThinkBlocks', () {
    test('removes complete think blocks wherever embedded', () {
      const text = 'before <think foo="1">secret\nreasoning</think> after';
      expect(stripThinkBlocks(text), 'before  after');
    });

    test('leaves plain text untouched', () {
      expect(stripThinkBlocks('hello'), 'hello');
    });
  });

  group('toolItemFields', () {
    test('commandExecution: exitCode != 0 ⇒ failed with output as error', () {
      final fields = toolItemFields(proto.CommandExecutionItem(
        id: 'c1',
        command: 'cargo test',
        cwd: '/repo',
        status: 'completed',
        exitCode: 101,
        aggregatedOutput: 'error: test failed',
      ))!;
      expect(fields.failed, isTrue);
      expect(fields.errorText, 'error: test failed');
      expect(fields.output, isNull);
    });

    test('commandExecution: exitCode 0 ⇒ output, not failed', () {
      final fields = toolItemFields(proto.CommandExecutionItem(
        id: 'c2',
        command: 'ls',
        cwd: '/repo',
        status: 'completed',
        exitCode: 0,
        aggregatedOutput: 'file list',
      ))!;
      expect(fields.failed, isFalse);
      expect(fields.output, 'file list');
    });

    test('commandExecution: failure without output falls back to exit code text', () {
      final fields = toolItemFields(proto.CommandExecutionItem(
        id: 'c3',
        command: 'false',
        cwd: '/',
        status: 'completed',
        exitCode: 1,
      ))!;
      expect(fields.errorText, 'exit code 1');
    });

    test('mcpToolCall failed when error present', () {
      final fields = toolItemFields(proto.McpToolCallItem(
        id: 'm1',
        server: 'srv',
        tool: 'get_weather',
        arguments: {'city': 'X'},
        status: 'completed',
        error: 'boom',
      ))!;
      expect(fields.failed, isTrue);
      expect(fields.errorText, 'boom');
    });

    test('non-tool items return null', () {
      expect(toolItemFields(proto.AgentMessageItem(id: 'a', text: 'hi')), isNull);
    });
  });

  group('projectItems (history)', () {
    test('groups consecutive non-user items into one assistant message', () {
      final messages = projectItems([
        proto.TurnItem.fromJson({
          'type': 'userMessage',
          'id': 'u1',
          'content': [
            {'type': 'text', 'text': 'please run ls'},
          ],
        }),
        proto.TurnItem.fromJson({
          'type': 'commandExecution',
          'id': 'c1',
          'command': 'ls',
          'cwd': '/repo',
          'status': 'completed',
          'exitCode': 0,
          'aggregatedOutput': 'a b',
        }),
        proto.TurnItem.fromJson({
          'type': 'agentMessage',
          'id': 'a1',
          'text': 'done <think>secret</think> really',
        }),
      ], baseUri);

      expect(messages.length, 2);
      expect(messages[0].fromUser, isTrue);
      expect((messages[0].parts.single as TextUiPart).text, 'please run ls');
      // Assistant group id = first item id; think block stripped.
      expect(messages[1].id, 'c1');
      expect(messages[1].parts[0], isA<ToolUiPart>());
      expect((messages[1].parts[1] as TextUiPart).text, 'done  really');
    });

    test('image content: data URL passes, /v1/ resolves against base, bare path drops', () {
      final messages = projectItems([
        proto.TurnItem.fromJson({
          'type': 'userMessage',
          'id': 'u1',
          'content': [
            {'type': 'text', 'text': 'look'},
            {'type': 'image', 'image_url': 'data:image/png;base64,AAA'},
            {'type': 'image', 'image_url': '/v1/images/xyz.png'},
            {'type': 'image', 'image_url': 'C:\\raw\\path.png'},
          ],
        }),
      ], baseUri);
      final parts = messages.single.parts;
      expect(parts[0], isA<TextUiPart>());
      expect((parts[1] as ImageUiPart).url, 'data:image/png;base64,AAA');
      expect((parts[2] as ImageUiPart).url, '$base/v1/images/xyz.png');
      expect(parts.length, 3); // bare local path dropped
    });

    test('empty groups produce no message', () {
      expect(projectItems(const [], baseUri), isEmpty);
      expect(
        projectItems([proto.UnknownItem(id: 'x')], baseUri),
        isEmpty,
      );
    });
  });

  group('LiveTurnProjector', () {
    LiveTurnProjector projector({int threshold = -1, String thread = 't1'}) =>
        LiveTurnProjector(baseUrl: baseUri, boundThreadId: thread, threshold: threshold);

    test('agent message: deltas accumulate, completion finalizes authoritative text', () {
      final p = projector();
      const itemId = 'a1';
      p.feed(frame(HarnessNotification.itemStarted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'agentMessage', 'id': itemId, 'text': ''},
      }));
      p.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 't1', 'turnId': '0', 'itemId': itemId, 'delta': 'Hel'}));
      p.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 't1', 'turnId': '0', 'itemId': itemId, 'delta': 'lo'}));

      var text = (p.messages.single.parts.single as TextUiPart);
      expect(text.text, 'Hello');
      expect(text.streaming, isTrue);

      final terminated = p.feed(frame(HarnessNotification.itemCompleted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'agentMessage', 'id': itemId, 'text': 'Hello <think>x</think>'},
      }));
      expect(terminated, isFalse);
      text = (p.messages.single.parts.single as TextUiPart);
      expect(text.text, 'Hello');
      expect(text.streaming, isFalse);

      expect(p.feed(frame(HarnessNotification.turnCompleted, {'threadId': 't1', 'turn': {'id': '0', 'items': [], 'status': 'completed'}})), isTrue);
      expect(p.finished, isTrue);
    });

    test('replay guard drops non-terminal events at or below threshold', () {
      final p = projector(threshold: 3);
      p.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 't1', 'turnId': '3', 'itemId': 'old', 'delta': 'replayed'}));
      expect(p.messages, isEmpty);
      p.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 't1', 'turnId': '4', 'itemId': 'new', 'delta': 'live'}));
      expect(p.messages, isNotEmpty);
    });

    test('terminal events pass the replay guard; other-thread events drop', () {
      final p = projector(threshold: 5);
      expect(
        p.feed(frame(HarnessNotification.turnCompleted, {'threadId': 't1', 'turn': {'id': '2', 'items': [], 'status': 'completed'}})),
        isTrue,
      );
      final q = projector();
      q.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 'other', 'turnId': '9', 'itemId': 'x', 'delta': 'd'}));
      expect(q.messages, isEmpty);
    });

    test('command tool card: created on start, streams output, finalizes with failure state', () {
      final p = projector();
      const itemId = 'c1';
      p.feed(frame(HarnessNotification.itemStarted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'commandExecution', 'id': itemId, 'command': 'ls', 'cwd': '/', 'status': 'inProgress'},
      }));
      p.feed(frame(HarnessNotification.itemCommandExecutionOutputDelta, {'threadId': 't1', 'turnId': '0', 'itemId': itemId, 'delta': 'stdout…'}));
      var tool = (p.messages.single.parts.single as ToolUiPart);
      expect(tool.phase, ToolPhase.running);
      expect(tool.liveOutput, 'stdout…');

      p.feed(frame(HarnessNotification.itemCompleted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'commandExecution', 'id': itemId, 'command': 'ls', 'cwd': '/', 'status': 'completed', 'exitCode': 0, 'aggregatedOutput': 'stdout…'},
      }));
      tool = (p.messages.single.parts.single as ToolUiPart);
      expect(tool.phase, ToolPhase.outputAvailable);
      expect(tool.output, 'stdout…');
      expect(tool.liveOutput, isEmpty);
    });

    test('approval request surfaces an awaiting-approval card even before item/started', () {
      final p = projector();
      p.feed(frame(HarnessNotification.itemCommandExecutionRequestApproval, {
        'threadId': 't1',
        'turnId': '0',
        'itemId': 'c9',
        'command': 'rm -rf /',
        'cwd': '/tmp',
        'allowedScopes': ['run_once', 'always'],
      }));
      final tool = (p.messages.single.parts.single as ToolUiPart);
      expect(tool.phase, ToolPhase.awaitingApproval);
      expect(tool.toolName, 'commandExecution');
      expect(tool.input, {'command': 'rm -rf /', 'cwd': '/tmp'});
    });

    test('error notification is turn-terminal and appends an error part', () {
      final p = projector();
      final terminated = p.feed(frame(HarnessNotification.error, {'threadId': 't1', 'code': 'E', 'message': 'model load failed'}));
      expect(terminated, isTrue);
      expect(p.messages.single.parts.single, isA<ErrorUiPart>());
    });

    test('live and history produce equivalent finals (the TS core invariant)', () {
      final items = [
        proto.TurnItem.fromJson({
          'type': 'userMessage',
          'id': 'u1',
          'content': [
            {'type': 'text', 'text': 'hi'},
          ],
        }),
        proto.TurnItem.fromJson({'type': 'reasoning', 'id': 'r1', 'summary': 's', 'content': 'thinking…'}),
        proto.TurnItem.fromJson({'type': 'agentMessage', 'id': 'a1', 'text': 'answer'}),
      ];
      final history = projectItems(items, baseUri);
      expect(history.length, 2);

      final p = projector();
      p.feed(frame(HarnessNotification.itemStarted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'reasoning', 'id': 'r1', 'summary': 's', 'content': ''},
      }));
      p.feed(frame(HarnessNotification.itemReasoningTextDelta, {'threadId': 't1', 'turnId': '0', 'itemId': 'r1', 'delta': 'thinking…'}));
      p.feed(frame(HarnessNotification.itemCompleted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'reasoning', 'id': 'r1', 'summary': 's', 'content': 'thinking…'},
      }));
      p.feed(frame(HarnessNotification.itemAgentMessageDelta, {'threadId': 't1', 'turnId': '0', 'itemId': 'a1', 'delta': 'answer'}));
      p.feed(frame(HarnessNotification.itemCompleted, {
        'threadId': 't1',
        'turnId': '0',
        'item': {'type': 'agentMessage', 'id': 'a1', 'text': 'answer'},
      }));
      p.feed(frame(HarnessNotification.turnCompleted, {'threadId': 't1', 'turn': {'id': '0', 'items': [], 'status': 'completed'}}));

      final liveAssistant = p.messages.single;
      final historyAssistant = history[1];
      expect(liveAssistant.id, historyAssistant.id);
      expect(
        (liveAssistant.parts[0] as ReasoningUiPart).text,
        (historyAssistant.parts[0] as ReasoningUiPart).text,
      );
      expect(
        (liveAssistant.parts[1] as TextUiPart).text,
        (historyAssistant.parts[1] as TextUiPart).text,
      );
    });
  });

  group('resolveImageUrl', () {
    test('data and absolute http(s) pass through; // upgrades to https', () {
      expect(resolveImageUrl('data:image/png;base64,A', baseUri), startsWith('data:'));
      expect(resolveImageUrl('http://x/y.png', baseUri), 'http://x/y.png');
      expect(resolveImageUrl('//cdn/x.png', baseUri), 'https://cdn/x.png');
    });
    test('bare filesystem paths cannot be fetched', () {
      expect(resolveImageUrl('/Users/me/pic.png', baseUri), isNull);
      expect(resolveImageUrl('', baseUri), isNull);
    });
  });
}
