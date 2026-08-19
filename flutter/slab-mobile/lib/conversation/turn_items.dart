/// Shared `TurnItem` → UI mapping for the harness protocol.
///
/// Dart port of `packages/slab-core/src/harness/turn-items.ts` (+ the live
/// chunk machine from `stream.ts`, folded into [LiveTurnProjector]). History
/// ([projectItems]) and live ([LiveTurnProjector]) share the same tool-field
/// extraction and `<think>` stripping so the two paths cannot drift on how an
/// item maps to UI parts — the TS repo's core invariant.
library;

import '../proto/harness_methods.dart';
import '../proto/harness_types.dart' as proto;
import '../proto/json_rpc.dart';

// ── UI model ────────────────────────────────────────────────────────────────

enum ToolPhase { running, awaitingApproval, outputAvailable, outputError }

sealed class UiPart {
  const UiPart();
}

final class TextUiPart extends UiPart {
  const TextUiPart({required this.text, this.streaming = false});
  final String text;
  final bool streaming;
}

final class ReasoningUiPart extends UiPart {
  const ReasoningUiPart({required this.text, this.streaming = false});
  final String text;
  final bool streaming;
}

final class ToolUiPart extends UiPart {
  const ToolUiPart({
    required this.itemId,
    required this.toolName,
    required this.input,
    this.output,
    this.errorText,
    this.failed = false,
    this.phase = ToolPhase.running,
    this.liveOutput = '',
  });
  final String itemId;
  final String toolName;
  final Object? input;
  final Object? output;
  final String? errorText;
  final bool failed;
  final ToolPhase phase;
  /// Accumulated live stdout/stderr (before `item/completed` finalizes).
  final String liveOutput;

  ToolUiPart copyWith({Object? output, String? errorText, bool? failed, ToolPhase? phase, String? liveOutput}) =>
      ToolUiPart(
        itemId: itemId,
        toolName: toolName,
        input: input,
        output: output ?? this.output,
        errorText: errorText ?? this.errorText,
        failed: failed ?? this.failed,
        phase: phase ?? this.phase,
        liveOutput: liveOutput ?? this.liveOutput,
      );
}

final class ImageUiPart extends UiPart {
  const ImageUiPart({required this.url});
  final String url;
}

final class ErrorUiPart extends UiPart {
  const ErrorUiPart({required this.text});
  final String text;
}

class ChatMessage {
  const ChatMessage({required this.id, required this.fromUser, required this.parts});
  final String id;
  final bool fromUser;
  final List<UiPart> parts;

  ChatMessage copyWith({List<UiPart>? parts}) =>
      ChatMessage(id: id, fromUser: fromUser, parts: parts ?? this.parts);
}

// ── Tool-field extraction (shared by history and live) ──────────────────────

/// Tool-shaped fields extracted from a finalized tool-like `TurnItem`.
class ToolFields {
  const ToolFields({required this.toolName, required this.input, this.output, this.errorText, required this.failed});
  final String toolName;
  final Object? input;
  final Object? output;
  final String? errorText;
  final bool failed;
}

/// Extract the tool fields from a finalized tool-like item (`commandExecution`,
/// `mcpToolCall`, `fileChange`, `webSearch`, `plan`). `null` for non-tool items.
ToolFields? toolItemFields(proto.TurnItem item) {
  switch (item) {
    case proto.CommandExecutionItem():
      final failed = item.exitCode != null && item.exitCode != 0;
      return ToolFields(
        toolName: 'commandExecution',
        input: {'command': item.command, 'cwd': item.cwd},
        output: !failed && item.aggregatedOutput != null && item.aggregatedOutput!.isNotEmpty ? item.aggregatedOutput : null,
        errorText: failed ? (item.aggregatedOutput ?? 'exit code ${item.exitCode}') : null,
        failed: failed,
      );
    case proto.McpToolCallItem():
      final failed = item.error != null;
      return ToolFields(
        toolName: item.tool,
        input: item.arguments,
        output: !failed && item.result != null ? item.result : null,
        errorText: failed ? stringifyToolValue(item.error) : null,
        failed: failed,
      );
    case proto.FileChangeItem():
      return ToolFields(
        toolName: 'fileChange',
        input: {'changes': item.changes},
        output: {'status': item.status},
        failed: false,
      );
    case proto.WebSearchItem():
      return ToolFields(toolName: 'webSearch', input: {'query': item.query}, failed: false);
    case proto.PlanTurnItem():
      return ToolFields(toolName: 'plan', input: item.plan, output: item.plan, failed: false);
    default:
      return null;
  }
}

/// Stringify a tool error/result value of unknown shape for display.
String stringifyToolValue(Object? value) {
  if (value == null) return '';
  if (value is String) return value;
  return value.toString();
}

// ── History projection ──────────────────────────────────────────────────────

/// Complete `<think …>…</think>` blocks the server used to embed into the
/// persisted agentMessage text (LLM-context form). Mirrors the server-side
/// `strip_think_blocks` emission guard and the TS `THINK_BLOCK_PATTERN`.
final RegExp thinkBlockPattern = RegExp(r'<think\b[^>]*>[\s\S]*?</think>', caseSensitive: false);

String stripThinkBlocks(String text) => text.replaceAll(thinkBlockPattern, '').trim();

/// Resolve an image reference into a URL the app can fetch: `data:` URIs and
/// absolute http(s) URLs pass through; slab-server artifact paths (`/v1/...`)
/// resolve against the API base; bare filesystem paths cannot be fetched by a
/// network client → `null`.
String? resolveImageUrl(String pathOrUrl, Uri baseUrl) {
  if (pathOrUrl.isEmpty) return null;
  if (pathOrUrl.startsWith('data:') || RegExp('^(https?:)?//', caseSensitive: false).hasMatch(pathOrUrl)) {
    return pathOrUrl.startsWith('//') ? 'https:$pathOrUrl' : pathOrUrl;
  }
  if (pathOrUrl.startsWith('/v1/')) {
    // Build a fresh URI (replace with empty query/fragment would render "?#").
    return Uri(scheme: baseUrl.scheme, userInfo: baseUrl.userInfo, host: baseUrl.host, port: baseUrl.port, path: pathOrUrl).toString();
  }
  return null;
}

List<UiPart> _turnItemToUiParts(proto.TurnItem item, Uri baseUrl) {
  switch (item) {
    case proto.AgentMessageItem():
      final text = stripThinkBlocks(item.text);
      return text.isEmpty ? const [] : [TextUiPart(text: text)];
    case proto.ReasoningItem():
      return item.content.isEmpty ? const [] : [ReasoningUiPart(text: item.content)];
    case proto.ImageViewItem():
      final url = resolveImageUrl(item.path, baseUrl);
      return url == null ? const [] : [ImageUiPart(url: url)];
    case proto.UserMessageItem() || proto.UnknownItem():
      return const [];
    default:
      final fields = toolItemFields(item);
      if (fields == null) return const [];
      return [
        ToolUiPart(
          itemId: item.id,
          toolName: fields.toolName,
          input: fields.input,
          output: fields.output,
          errorText: fields.errorText,
          failed: fields.failed,
          phase: fields.failed ? ToolPhase.outputError : ToolPhase.outputAvailable,
        ),
      ];
  }
}

/// Project a flat, ordered list of finalized items into chat messages.
///
/// Grouping mirrors the live stream: a `userMessage` item starts a user
/// message and flushes any in-flight assistant group; consecutive non-user
/// items fold into one assistant message whose id is the first item's id.
List<ChatMessage> projectItems(Iterable<proto.TurnItem> items, Uri baseUrl) {
  final messages = <ChatMessage>[];
  String? pendingAssistantId;
  var pendingParts = <UiPart>[];

  void flushAssistant() {
    if (pendingAssistantId != null && pendingParts.isNotEmpty) {
      messages.add(ChatMessage(id: pendingAssistantId!, fromUser: false, parts: List.unmodifiable(pendingParts)));
    }
    pendingAssistantId = null;
    pendingParts = <UiPart>[];
  }

  for (final item in items) {
    if (item is proto.UserMessageItem) {
      flushAssistant();
      final parts = <UiPart>[];
      for (final content in item.content) {
        if (content is proto.TextContent) {
          if (content.text.isNotEmpty) parts.add(TextUiPart(text: content.text));
        } else if (content is proto.ImageContent) {
          final source = content.imageUrl ??
              (content.base64 != null ? 'data:${content.mimeType ?? 'image/png'};base64,${content.base64}' : null);
          final url = source == null ? null : resolveImageUrl(source, baseUrl);
          if (url != null) parts.add(ImageUiPart(url: url));
        }
      }
      if (parts.isNotEmpty) messages.add(ChatMessage(id: item.id, fromUser: true, parts: List.unmodifiable(parts)));
      continue;
    }
    pendingAssistantId ??= item.id;
    pendingParts.addAll(_turnItemToUiParts(item, baseUrl));
  }
  flushAssistant();
  return messages;
}

// ── Live projector ──────────────────────────────────────────────────────────

/// Mutable projection of one live turn's notifications into [ChatMessage]s.
///
/// Ports the `stream.ts` chunk machine + the `harness-transport.ts` replay
/// guard: notifications for another thread are dropped, and non-terminal
/// notifications whose numeric `turnId` is `<= threshold` (captured at turn
/// start) are replayed history and dropped. Both `turn/completed` and `error`
/// are turn-terminal.
class LiveTurnProjector {
  LiveTurnProjector({required this.baseUrl, required this.boundThreadId, required this.threshold});

  final Uri baseUrl;
  final String boundThreadId;
  final int threshold;

  final List<ChatMessage> messages = [];
  final Set<String> openText = {};
  final Set<String> openReasoning = {};
  bool finished = false;

  // itemId → (message index, part index) for live part mutation.
  final Map<String, int> _partIndexByItemId = {};
  final Map<String, int> _messageIndexByItemId = {};

  /// Feed one notification; returns whether the turn terminated.
  bool feed(NotificationFrame notification) {
    if (finished) return false;
    final params = notification.params ?? const <String, Object?>{};
    final threadId = params['threadId'];
    // Ignore notifications for a different thread on the shared socket.
    if (threadId is String && threadId != boundThreadId) return false;

    final terminal = notification.method == HarnessNotification.turnCompleted ||
        notification.method == HarnessNotification.error;

    // Drop replayed history (turnId at or below the threshold) unless terminal
    // or carrying a non-numeric turnId.
    if (!terminal) {
      final turnId = params['turnId'];
      final turnNum = turnId is String ? int.tryParse(turnId) : null;
      if (turnNum != null && turnNum <= threshold) return false;
    }

    switch (notification.method) {
      case HarnessNotification.itemStarted:
        _onItemStarted(proto.TurnItem.fromJson(_expectMap(params['item'])));
      case HarnessNotification.itemCompleted:
        _onItemCompleted(proto.TurnItem.fromJson(_expectMap(params['item'])));
      case HarnessNotification.itemAgentMessageDelta:
        _appendTextDelta(params);
      case HarnessNotification.itemReasoningTextDelta:
      case HarnessNotification.itemReasoningSummaryTextDelta:
        _appendReasoningDelta(params);
      case HarnessNotification.itemCommandExecutionOutputDelta:
      case HarnessNotification.itemFileChangeOutputDelta:
        _appendLiveOutput(params);
      case HarnessNotification.itemCommandExecutionRequestApproval:
        _onApprovalRequested(
          itemId: _string(params['itemId']),
          toolName: 'commandExecution',
          input: {'command': _string(params['command']), 'cwd': _string(params['cwd'])},
        );
      case HarnessNotification.itemFileChangeRequestApproval:
        _onApprovalRequested(
          itemId: _string(params['itemId']),
          toolName: 'fileChange',
          input: {
            'changes': (params['changes'] is List ? params['changes']! as List : const []).toList(),
          },
        );
      case HarnessNotification.turnCompleted:
        _finish();
        return true;
      case HarnessNotification.error:
        _onError(params);
        _finish();
        return true;
      default:
        break; // thread/started, turn/started, model/load/*, context/* — out-of-band
    }
    return false;
  }

  // ── handlers ──────────────────────────────────────────────────────────────

  void _onItemStarted(proto.TurnItem item) {
    switch (item) {
      case proto.AgentMessageItem():
        // Close any reasoning part still open so its indicator stops, even
        // when the server jumps straight to the agent message (TS parity).
        _closeOpenSet(openReasoning, (ReasoningUiPart part) => ReasoningUiPart(text: part.text, streaming: false));
        _openPart(item.id, () => const TextUiPart(text: '', streaming: true), openText);
      case proto.ReasoningItem():
        _openPart(item.id, () => const ReasoningUiPart(text: '', streaming: true), openReasoning);
      case proto.UserMessageItem() || proto.UnknownItem():
        break;
      default:
        // Tool-like items: create the card immediately so commands are visible
        // (and stream live output) while they execute.
        final fields = toolItemFields(item);
        if (fields == null) return;
        _openPart(item.id, () => ToolUiPart(itemId: item.id, toolName: fields.toolName, input: fields.input), null);
    }
  }

  void _onItemCompleted(proto.TurnItem item) {
    switch (item) {
      case proto.AgentMessageItem():
        // The finalized item text is authoritative — prefer it over the
        // accumulated deltas (they are identical on a clean stream, and this
        // self-heals a bubble whose deltas were missed across a reconnect).
        final found = _findPart(item.id);
        final accumulated = found != null && found.$3 is TextUiPart ? (found.$3 as TextUiPart).text : '';
        final stripped = stripThinkBlocks(item.text);
        final finalText = stripped.isNotEmpty ? stripped : accumulated;
        if (found != null) {
          _replacePartAt(found.$1, found.$2, TextUiPart(text: finalText));
        } else if (finalText.isNotEmpty) {
          _openPart(item.id, () => TextUiPart(text: finalText), openText);
        }
        openText.remove(item.id);
      case proto.ReasoningItem():
        _closeOpenPart(item.id, openReasoning, (ReasoningUiPart part) => ReasoningUiPart(text: part.text, streaming: false));
      case proto.UserMessageItem() || proto.UnknownItem():
        break;
      default:
        final fields = toolItemFields(item);
        if (fields == null) return;
        final found = _findPart(item.id);
        if (found != null && found.$3 is ToolUiPart) {
          _replacePartAt(
            found.$1,
            found.$2,
            (found.$3 as ToolUiPart).copyWith(
              output: fields.output,
              errorText: fields.errorText,
              failed: fields.failed,
              phase: fields.failed ? ToolPhase.outputError : ToolPhase.outputAvailable,
              liveOutput: '',
            ),
          );
        } else {
          _openPart(
            item.id,
            () => ToolUiPart(
              itemId: item.id,
              toolName: fields.toolName,
              input: fields.input,
              output: fields.output,
              errorText: fields.errorText,
              failed: fields.failed,
              phase: fields.failed ? ToolPhase.outputError : ToolPhase.outputAvailable,
            ),
            null,
          );
        }
    }
  }

  /// An approval request also surfaces the pending tool card (the server sends
  /// it with the same itemId as the running item).
  void _onApprovalRequested({required String itemId, required String toolName, required Map<String, Object?> input}) {
    final found = _findPart(itemId);
    if (found != null && found.$3 is ToolUiPart) {
      _replacePartAt(found.$1, found.$2, (found.$3 as ToolUiPart).copyWith(phase: ToolPhase.awaitingApproval));
    } else {
      _openPart(itemId, () => ToolUiPart(itemId: itemId, toolName: toolName, input: input, phase: ToolPhase.awaitingApproval), null);
    }
  }

  void _appendLiveOutput(Map<String, Object?> params) {
    final itemId = _string(params['itemId']);
    final found = _findPart(itemId);
    if (found == null) return;
    final part = found.$3;
    if (part is! ToolUiPart) return;
    final delta = _string(params['delta']);
    // Bound per-item accumulation so a runaway command can't exhaust memory.
    if (part.liveOutput.length + delta.length > 256 * 1024) return;
    _replacePartAt(found.$1, found.$2, part.copyWith(liveOutput: part.liveOutput + delta));
  }

  void _appendTextDelta(Map<String, Object?> params) {
    final itemId = _string(params['itemId']);
    var found = _findPart(itemId);
    if (found == null) {
      _openPart(itemId, () => const TextUiPart(text: '', streaming: true), openText);
      found = _findPart(itemId);
      if (found == null) return;
    }
    final part = found.$3;
    if (part is TextUiPart) {
      _replacePartAt(found.$1, found.$2, TextUiPart(text: part.text + _string(params['delta']), streaming: true));
      openText.add(itemId);
    }
  }

  void _appendReasoningDelta(Map<String, Object?> params) {
    final itemId = _string(params['itemId']);
    var found = _findPart(itemId);
    if (found == null) {
      _openPart(itemId, () => const ReasoningUiPart(text: '', streaming: true), openReasoning);
      found = _findPart(itemId);
      if (found == null) return;
    }
    final part = found.$3;
    if (part is ReasoningUiPart) {
      _replacePartAt(found.$1, found.$2, ReasoningUiPart(text: part.text + _string(params['delta']), streaming: true));
      openReasoning.add(itemId);
    }
  }

  void _onError(Map<String, Object?> params) {
    final message = _string(params['message']);
    final assistant = _ensureAssistant('turn-error');
    final parts = [...assistant.parts, ErrorUiPart(text: message.isEmpty ? 'harness error' : message)];
    _replaceMessage(_messageIndexOf(assistant.id), ChatMessage(id: assistant.id, fromUser: false, parts: parts));
  }

  void _finish() {
    finished = true;
    _closeOpenSet(openReasoning, (ReasoningUiPart part) => ReasoningUiPart(text: part.text, streaming: false));
    _closeOpenSet(openText, (TextUiPart part) => TextUiPart(text: part.text));
  }

  // ── part plumbing ─────────────────────────────────────────────────────────

  void _openPart(String itemId, UiPart Function() build, Set<String>? openSet) {
    if (_findPart(itemId) != null) return;
    openSet?.add(itemId);
    final assistant = _ensureAssistant(itemId);
    final parts = [...assistant.parts, build()];
    _messageIndexByItemId[itemId] = _messageIndexOf(assistant.id);
    _partIndexByItemId[itemId] = parts.length - 1;
    _replaceMessage(
      _messageIndexOf(assistant.id),
      ChatMessage(id: assistant.id, fromUser: false, parts: parts),
    );
  }

  /// Ensure an assistant message exists for the current run of non-user items;
  /// consecutive items of one turn fold into one message whose id is the FIRST
  /// item's id (mirrors history grouping; the projector only ever holds the
  /// live turn's messages).
  ChatMessage _ensureAssistant(String itemId) {
    if (messages.isNotEmpty && !messages.last.fromUser) return messages.last;
    messages.add(ChatMessage(id: itemId, fromUser: false, parts: const []));
    return messages.last;
  }

  (int, int, UiPart)? _findPart(String itemId) {
    final messageIndex = _messageIndexByItemId[itemId];
    final partIndex = _partIndexByItemId[itemId];
    if (messageIndex == null || partIndex == null) return null;
    if (messageIndex >= messages.length) return null;
    final message = messages[messageIndex];
    if (partIndex >= message.parts.length) return null;
    return (messageIndex, partIndex, message.parts[partIndex]);
  }

  void _replacePartAt(int messageIndex, int partIndex, UiPart part) {
    final message = messages[messageIndex];
    final parts = [...message.parts]..[partIndex] = part;
    _replaceMessage(messageIndex, ChatMessage(id: message.id, fromUser: message.fromUser, parts: parts));
  }

  void _replaceMessage(int index, ChatMessage message) {
    messages[index] = message;
  }

  int _messageIndexOf(String id) => messages.indexWhere((m) => m.id == id);

  /// Close every open part of type `T` (an open set only ever holds one kind).
  void _closeOpenSet<T extends UiPart>(Set<String> openSet, T Function(T) close) {
    for (final itemId in openSet.toList()) {
      _closeOpenPart(itemId, openSet, close);
    }
  }

  void _closeOpenPart<T extends UiPart>(String itemId, Set<String> openSet, T Function(T) close) {
    openSet.remove(itemId);
    final found = _findPart(itemId);
    if (found == null || found.$3 is! T) return;
    _replacePartAt(found.$1, found.$2, close(found.$3 as T));
  }
}

Map<String, Object?> _expectMap(Object? value) =>
    value is Map<String, Object?> ? value : const <String, Object?>{};

String _string(Object? value) => value is String ? value : '';
