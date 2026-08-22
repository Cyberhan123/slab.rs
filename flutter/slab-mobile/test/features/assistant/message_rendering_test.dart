/// Unit tests for the B3 rendering pieces: ANSI parser, diff classifier,
/// plan decode, SandboxedOutput split, live committed-file parse, and the
/// scroller-row ordering.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/domain/conversation/conversation_controller.dart';
import 'package:slab_mobile/domain/conversation/turn_items.dart';
import 'package:slab_mobile/core/utils/ansi.dart';
import 'package:slab_mobile/features/assistant/view/widgets/messages/diff_view.dart';
import 'package:slab_mobile/features/assistant/view/widgets/messages/file_change_card.dart';
import 'package:slab_mobile/features/assistant/view/widgets/messages/message_list.dart';
import 'package:slab_mobile/features/assistant/view/widgets/messages/plan_card.dart';
import 'package:slab_mobile/features/assistant/view/widgets/messages/terminal_card.dart';
import 'package:slab_mobile/domain/session_labels.dart';
import 'package:slab_mobile/core/network/slab_api_error.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/features/assistant/commands/command_registry.dart';
import 'package:slab_mobile/features/assistant/commands/request_errors.dart';
import 'package:slab_mobile/data/harness/harness_types.dart' as proto;

void main() {
  group('parseAnsi', () {
    test('plain text passes through as one span', () {
      final spans = parseAnsi('hello world');
      expect(spans, hasLength(1));
      expect(spans.single.text, 'hello world');
      expect(spans.single.style.foreground, isNull);
    });

    test('SGR colors split spans and reset restores defaults', () {
      final spans = parseAnsi('a\x1b[31mred\x1b[0mplain');
      expect(spans.map((s) => s.text), ['a', 'red', 'plain']);
      expect(spans[1].style.foreground, isNotNull);
      expect(spans[2].style.foreground, isNull);
    });

    test('bold/underline flags and 256-color / RGB extended colors', () {
      final spans = parseAnsi('\x1b[1;4mbold\x1b[38;5;208mcube\x1b[38;2;10;20;30mrgb');
      expect(spans[0].style.bold, isTrue);
      expect(spans[0].style.underline, isTrue);
      expect(spans[1].style.foreground, isNotNull);
      expect(spans[2].style.foreground, isNotNull);
    });

    test('carriage returns overwrite the line from column 0', () {
      expect(parseAnsi('abc\x1b[0K\rxyz').map((s) => s.text).join(), contains('xyz'));
      // Shorter overwrite keeps the tail of the previous content.
      final spans = parseAnsi('hello\rj');
      expect(spans.map((s) => s.text).join(), 'jello');
    });

    test('non-SGR CSI sequences are stripped', () {
      final joined = parseAnsi('\x1b[2J\x1b[?25lclean\x1b[1Gend').map((s) => s.text).join();
      expect(joined, 'cleanend');
    });
  });

  group('classifyDiffLine', () {
    test('apply_patch dialect', () {
      expect(classifyDiffLine('*** Begin Patch'), DiffLineKind.meta);
      expect(classifyDiffLine('*** Update File: a.rs'), DiffLineKind.meta);
      expect(classifyDiffLine('@@ -1,2 +1,3 @@'), DiffLineKind.hunk);
      expect(classifyDiffLine('+added'), DiffLineKind.add);
      expect(classifyDiffLine('-removed'), DiffLineKind.del);
      expect(classifyDiffLine(' context'), DiffLineKind.context);
    });

    test('unified headers are meta, heredoc wrappers are meta, rest plain', () {
      expect(classifyDiffLine('+++ b/x'), DiffLineKind.meta);
      expect(classifyDiffLine('--- a/x'), DiffLineKind.meta);
      expect(classifyDiffLine('EOF'), DiffLineKind.meta);
      expect(classifyDiffLine("apply_patch <<'EOF'"), DiffLineKind.meta);
      expect(classifyDiffLine('random text'), DiffLineKind.plain);
    });
  });

  group('decodePlan', () {
    test('decodes steps, counts, current step; tolerates junk', () {
      final plan = decodePlan({
        'plan_id': 'p1',
        'summary': 'Do things',
        'items': [
          {'step': 'first', 'status': 'completed'},
          {'step': 'second', 'status': 'in_progress', 'depends_on': ['first']},
          {'step': 'third', 'status': 'blocked', 'result_ref': 'r1'},
        ],
        'counts': {'pending': 0, 'in_progress': 1, 'completed': 1, 'blocked': 1},
        'current_step': 1,
      });
      expect(plan, isNotNull);
      expect(plan!.items, hasLength(3));
      expect(plan.items[1].status, PlanStepStatus.inProgress);
      expect(plan.counts.blocked, 1);
      expect(plan.currentStep, 1);
      expect(decodePlan('nope'), isNull);
      expect(decodePlan({'items': <Object?>[]}), isNull);
    });
  });

  group('decodeSandboxedOutput', () {
    test('splits stdout/stderr/exit and rejects non-envelope payloads', () {
      final sandboxed = decodeSandboxedOutput('{"stdout":"out","stderr":"err","exit_code":2,"timed_out":false}');
      expect(sandboxed, isNotNull);
      expect(sandboxed!.stdoutText, 'out');
      expect(sandboxed.stderrText, 'err');
      expect(sandboxed.exitCode, 2);
      expect(decodeSandboxedOutput('plain text'), isNull);
      expect(decodeSandboxedOutput('{"other":1}'), isNull);
    });
  });

  group('parseCommittedFiles', () {
    test('reads JSON lines and skips partials mid-delta', () {
      const live = '{"path":"a.rs","kind":"A"}\n{"path":"b.rs","kind":"M"}\n{"path":"c.r';
      final files = parseCommittedFiles(live);
      expect(files, hasLength(2));
      expect(files[0].path, 'a.rs');
      expect(files[1].kind, 'M');
    });
  });

  group('buildScrollerRows', () {
    final user = ChatMessage(id: 'u1', fromUser: true, parts: const [TextUiPart(text: 'hi')]);

    test('session-load marker only while restoring with no messages', () {
      final rows = buildScrollerRows(
        messages: const [],
        compactionMarkers: const [],
        historyCount: 0,
        sessionLoading: true,
        modelLoad: null,
      );
      expect(rows.single, isA<SessionLoadMarkerRow>());
    });

    test('history marker sits between restored and live messages', () {
      final live = ChatMessage(id: 'a1', fromUser: false, parts: const []);
      final rows = buildScrollerRows(
        messages: [user, live],
        compactionMarkers: const [],
        historyCount: 1,
        sessionLoading: false,
        modelLoad: null,
      );
      expect(rows.map((row) => row.runtimeType), [
        MessageRow,
        HistoryMarkerRow,
        MessageRow,
      ]);
    });

    test('compaction markers then model-load marker trail the timeline', () {
      final marker = CompactionMarker(
        id: 'm1',
        mode: CompactionMode.manual,
        phase: CompactionPhase.compacted,
        threadId: 't1',
      );
      final rows = buildScrollerRows(
        messages: [user],
        compactionMarkers: [marker],
        historyCount: 1,
        sessionLoading: false,
        modelLoad: const ModelLoadState(phase: 'loading'),
      );
      expect(rows.map((row) => row.runtimeType), [
        MessageRow,
        CompactMarkerRow,
        ModelLoadMarkerRow,
      ]);
    });
  });

  group('session labels', () {
    test('default family covers both locales plus legacy names', () {
      expect(isDefaultSessionLabel('New assistant'), isTrue);
      expect(isDefaultSessionLabel('新助手'), isTrue);
      expect(isDefaultSessionLabel('New Conversation'), isTrue);
      expect(isDefaultSessionLabel('新对话'), isTrue);
      expect(isDefaultSessionLabel(null), isTrue);
      expect(isDefaultSessionLabel(''), isTrue);
      expect(isDefaultSessionLabel('My custom title'), isFalse);
    });

    test('first-prompt label truncates at 42 chars', () {
      expect(createConversationLabel('  hello  ', 'fallback'), 'hello');
      expect(createConversationLabel('', 'fallback'), 'fallback');
      final long = 'a' * 50;
      expect(createConversationLabel(long, 'fallback'), '${'a' * 42}...');
    });
  });

  group('slash-command dispatch', () {
    final commands = [
      proto.CommandInfo(name: 'compact', aliases: const ['summarize'], description: '', kind: proto.CommandKind.control, source: proto.CommandSource.builtin, controlAction: 'compact'),
      proto.CommandInfo(name: 'fork', aliases: const [], description: '', kind: proto.CommandKind.control, source: proto.CommandSource.builtin, controlAction: 'fork'),
      proto.CommandInfo(name: 'plan', aliases: const [], description: '', kind: proto.CommandKind.prompt, source: proto.CommandSource.builtin),
      proto.CommandInfo(name: 'explain', aliases: const [], description: '', kind: proto.CommandKind.prompt, source: proto.CommandSource.skill),
    ];

    test('parses name and args; bare or non-slash input is not a command', () {
      expect(parseAssistantCommand('/compact')!.name, 'compact');
      expect(parseAssistantCommand('/review  this   file ')!.args, 'this file');
      expect(parseAssistantCommand('hello'), isNull);
      expect(parseAssistantCommand('/'), isNull);
      expect(parseAssistantCommand('  '), isNull);
    });

    test('control commands resolve to host actions and never send', () {
      expect(resolveCommandDispatch('/compact', commands), isA<ControlDispatch>()
          .having((d) => d.controlAction, 'action', 'compact'));
      // Alias lookup works too.
      expect(resolveCommandDispatch('/summarize', commands), isA<ControlDispatch>());
      expect(resolveCommandDispatch('/fork now', commands), isA<ControlDispatch>()
          .having((d) => d.controlAction, 'action', 'fork'));
    });

    test('/plan toggles plan mode client-side', () {
      expect(resolveCommandDispatch('/plan', commands), isA<TogglePlanDispatch>());
    });

    test('prompt commands and unknown text fall through to send', () {
      expect(resolveCommandDispatch('/explain rust', commands), isA<SendDispatch>());
      expect(resolveCommandDispatch('just chatting', commands), isA<SendDispatch>());
      expect(resolveCommandDispatch('/unknown', commands), isA<SendDispatch>());
    });
  });

  group('error envelope i18n', () {
    final catalog = SlabCatalog.fromJson('en-US',
        '{"server.errors.badRequest":"Bad request: {{detail}}"}');

    test('extraction pulls message + i18n ref from the envelope', () {
      final (message, key, params) = slabApiErrorWithI18n({
        'message': 'Bad request',
        'i18n': {
          'message': {
            'key': 'server.errors.badRequest',
            'params': {'detail': 'x'},
          },
        },
      });
      expect(message, 'Bad request');
      expect(key, 'server.errors.badRequest');
      expect(params, {'detail': 'x'});
      final (plain, noKey, _) = slabApiErrorWithI18n({'message': 'plain'});
      expect(plain, 'plain');
      expect(noKey, isNull);
    });

    test('describeRestError translates the ref and falls back to the message', () {
      final localized = describeRestError(
        const SlabRestException('Bad request', 400,
            i18nKey: 'server.errors.badRequest',
            i18nParams: {'detail': 'x'}),
        catalog,
      );
      expect(localized, 'Bad request: x');
      final fallback = describeRestError(
        const SlabRestException('Raw failure', null, i18nKey: 'server.errors.missing'),
        catalog,
      );
      expect(fallback, 'Raw failure');
      expect(describeRestError(const SlabRestException('no i18n', null), catalog), 'no i18n');
    });
  });
}
