/// Framework-free conversation controller for the harness control plane.
///
/// Dart port of `packages/slab-core/src/harness/conversation-controller.ts`
/// (phase-1 scope): session restore (open retry + `thread/resume`
/// projection), the out-of-band notification projections (approval queue,
/// model-load indicator, turn usage), and the user actions (`send`, `interrupt`,
/// `approval/resolve`). Live turn projection is delegated to
/// [LiveTurnProjector]; reconnect is delegated to the client (transport) with
/// re-bind handled here on every transition back to `ready`.
///
/// Deliberate delta vs TS: approvals are CLEARED on a reconnect-resume — the
/// server re-requests still-pending approvals, and ones resolved while the
/// client was offline would otherwise stick as "pending" forever.
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show Listenable;

import '../proto/harness_client.dart';
import '../proto/harness_methods.dart';
import '../proto/harness_types.dart' as proto;
import '../proto/json_rpc.dart';
import 'turn_items.dart';

typedef Listener = void Function();

/// How many times the harness `open()` is retried before a restore fails
/// (the server may still be starting); mirrors the TS constants.
const maxRestoreAttempts = 3;
const restoreBackoff = Duration(milliseconds: 400);

enum ConnectionPhase { idle, connecting, ready, reconnecting }

enum TurnPhase { idle, running, modelLoading }

/// Transient model-load indicator state (`model/load/delta` + `completed`).
class ModelLoadState {
  const ModelLoadState({required this.phase, this.modelId, this.downloadedBytes, this.totalBytes});
  final String phase; // "downloading" | "loading"
  final String? modelId;
  final int? downloadedBytes;
  final int? totalBytes;
}

/// Immutable snapshot exposed via [ConversationController.state].
class ConversationState {
  const ConversationState({
    this.messages = const [],
    this.threadId,
    this.isHistoryLoading = false,
    this.error,
    this.approvals = const [],
    this.approvalStatusByItemId = const {},
    this.modelLoad,
    this.turnUsage,
    this.turnPhase = TurnPhase.idle,
    this.connection = ConnectionPhase.idle,
  });

  final List<ChatMessage> messages;
  final String? threadId;
  final bool isHistoryLoading;
  final String? error;
  final List<proto.ApprovalRequest> approvals; // pending only
  final Map<String, proto.ApprovalStatus> approvalStatusByItemId;
  final ModelLoadState? modelLoad;
  final proto.TurnUsage? turnUsage;
  final TurnPhase turnPhase;
  final ConnectionPhase connection;

  ConversationState copyWith({
    List<ChatMessage>? messages,
    String? threadId,
    bool? isHistoryLoading,
    String? error,
    bool clearError = false,
    List<proto.ApprovalRequest>? approvals,
    Map<String, proto.ApprovalStatus>? approvalStatusByItemId,
    ModelLoadState? modelLoad,
    bool clearModelLoad = false,
    proto.TurnUsage? turnUsage,
    TurnPhase? turnPhase,
    ConnectionPhase? connection,
  }) =>
      ConversationState(
        messages: messages ?? this.messages,
        threadId: threadId ?? this.threadId,
        isHistoryLoading: isHistoryLoading ?? this.isHistoryLoading,
        error: clearError ? null : (error ?? this.error),
        approvals: approvals ?? this.approvals,
        approvalStatusByItemId: approvalStatusByItemId ?? this.approvalStatusByItemId,
        modelLoad: clearModelLoad ? null : (modelLoad ?? this.modelLoad),
        turnUsage: turnUsage ?? this.turnUsage,
        turnPhase: turnPhase ?? this.turnPhase,
        connection: connection ?? this.connection,
      );
}

/// Highest numeric turn id in a thread (-1 when there are no turns); turns
/// with a non-terminal status (`inProgress`) are excluded so their live events
/// keep flowing after a reconnect-resume (see the doc comment above).
int computeLastTurnIndex(proto.Thread thread) {
  var max = -1;
  for (final turn in thread.turns) {
    final isTerminal = turn.status == 'completed' || turn.status == 'interrupted' || turn.status == 'failed';
    if (!isTerminal) continue;
    final index = turn.numericId;
    if (index != null && index > max) max = index;
  }
  return max;
}

/// `Listenable` so widgets bind via `ListenableBuilder`; the state machine
/// itself stays framework-free (foundation only).
class ConversationController implements Listenable {
  ConversationController({
    required this.sessionId,
    required this.baseUrl,
    HarnessClient? client,
    this.model = 'slab-llama',
  }) : client = client ?? HarnessClient(baseUrl: baseUrl, sessionId: sessionId);

  /// Slab session id; also the harness WS `?token=`.
  final String sessionId;
  final String model;
  final HarnessClient client;
  final Uri baseUrl;

  ConversationState _state = const ConversationState();
  final Set<Listener> _listeners = {};
  StreamSubscription<NotificationFrame>? _notificationSub;
  StreamSubscription<HarnessStatus>? _statusSub;
  LiveTurnProjector? _projector;
  List<ChatMessage> _history = const [];
  int _generation = 0;
  bool _sawDrop = false;

  ConversationState get state => _state;

  @override
  void addListener(Listener listener) => _listeners.add(listener);
  @override
  void removeListener(Listener listener) => _listeners.remove(listener);

  void _emit(ConversationState next) {
    _state = next;
    for (final listener in _listeners.toList()) {
      listener();
    }
  }

  /// Kick off the restore machine and subscribe to transport status.
  Future<void> start() {
    _notificationSub ??= client.notifications.listen(_handleNotification);
    _statusSub ??= client.statusStream.listen(_handleStatus);
    return reconnect();
  }

  /// Transport status. Only a `ready` seen AFTER a drop re-binds the thread
  /// (the initial restore is owned by [reconnect] — status events are
  /// delivered asynchronously, so keying on "first ready" here would race the
  /// restore machine and double-resume).
  void _handleStatus(HarnessStatus status) {
    switch (status) {
      case HarnessStatus.opening:
        _emit(_state.copyWith(connection: ConnectionPhase.connecting));
      case HarnessStatus.reconnecting:
        _sawDrop = true;
        _emit(_state.copyWith(connection: ConnectionPhase.reconnecting));
      case HarnessStatus.ready:
        if (_sawDrop) {
          _sawDrop = false;
          // Reconnect-resume: approvals cleared (server re-requests the ones
          // still pending); the live projector resets against fresh history.
          unawaited(_resumeAndProject());
        }
      case HarnessStatus.closed:
        _emit(_state.copyWith(connection: ConnectionPhase.idle));
      case HarnessStatus.idle:
        break;
    }
  }

  /// (Re)run the restore machine: `open()` with backed-off retries →
  /// `thread/resume` projection. A newer run (or `dispose()`) invalidates an
  /// in-flight one. Mirrors the TS `ConversationController.reconnect`.
  Future<void> reconnect() async {
    final generation = ++_generation;
    bool isCurrent() => _generation == generation;

    _emit(_state.copyWith(isHistoryLoading: true, clearError: true, connection: ConnectionPhase.connecting));

    try {
      for (var attempt = 1; attempt <= maxRestoreAttempts; attempt++) {
        if (!isCurrent()) return;
        try {
          await client.open();
          break;
        } catch (openError) {
          if (!isCurrent()) return;
          if (attempt == maxRestoreAttempts) rethrow;
          await Future<void>.delayed(restoreBackoff * attempt);
        }
      }
      if (!isCurrent()) return;
      await _resumeAndProject(generation: generation);
    } catch (restoreError) {
      if (!isCurrent()) return;
      _emit(_state.copyWith(
        error: restoreError.toString(),
        isHistoryLoading: false,
        connection: ConnectionPhase.idle,
      ));
      return;
    }
    if (isCurrent()) {
      _emit(_state.copyWith(isHistoryLoading: false, connection: ConnectionPhase.ready));
    }
  }

  Future<void> _resumeAndProject({int? generation}) async {
    final gen = generation ?? ++_generation;
    bool isCurrent() => _generation == gen;
    try {
      final thread = await client.threadResume();
      if (!isCurrent()) return;
      _bindThread(thread);
      _emit(_state.copyWith(
        threadId: thread.id,
        messages: List.unmodifiable(_history),
        approvals: const [],
        approvalStatusByItemId: const {},
        connection: ConnectionPhase.ready,
      ));
    } catch (resumeError) {
      final message = resumeError.toString();
      // A fresh session has no thread to resume — start empty and let the
      // first turn lazily create the thread.
      if (!RegExp('no thread to resume', caseSensitive: false).hasMatch(message)) {
        if (isCurrent()) {
          _emit(_state.copyWith(error: message, isHistoryLoading: false));
        }
        return;
      }
      client.currentThreadId = null;
      client.lastTurnIndex = -1;
      if (!isCurrent()) return;
      _history = const [];
      _projector = null;
      _emit(_state.copyWith(
        messages: const [],
        threadId: null,
        approvals: const [],
        approvalStatusByItemId: const {},
        connection: ConnectionPhase.ready,
      ));
    }
  }

  void _bindThread(proto.Thread thread) {
    client.currentThreadId = thread.id;
    client.lastTurnIndex = computeLastTurnIndex(thread);
    _history = projectItems(thread.turns.expand((turn) => turn.items), baseUrl);
    _projector = null;
  }

  /// Programmatically start a turn for the given user text: ensures the socket
  /// is open, lazily binds a thread (`thread/start` on a fresh session), and
  /// fires `turn/start` with the text input mapping.
  Future<void> sendText(String text, {String? modelId}) async {
    final effectiveModel = modelId ?? model;
    await client.open();

    // Bind a thread if none is bound yet (fresh session, no prior resume).
    if (client.currentThreadId == null) {
      final started = await client.threadStart(model: effectiveModel);
      client.currentThreadId = started.id;
    }
    final threadId = client.currentThreadId;
    if (threadId == null) throw StateError('no harness thread bound');

    // The optimistic user bubble (the live userMessage item is a no-op in the
    // projector, mirroring the TS stream machine).
    final userMessage = ChatMessage(
      id: 'local-${DateTime.now().microsecondsSinceEpoch}',
      fromUser: true,
      parts: [TextUiPart(text: text)],
    );
    final history = [..._history, ...?_projector?.messages, userMessage];
    _projector = LiveTurnProjector(
      baseUrl: baseUrl,
      boundThreadId: threadId,
      threshold: client.lastTurnIndex,
    );
    _emit(_state.copyWith(messages: List.unmodifiable(history), turnPhase: TurnPhase.running, clearError: true));

    try {
      await client.turnStart(
        threadId: threadId,
        input: [proto.textUserInput(text)],
        model: effectiveModel,
      );
    } catch (turnError) {
      _emit(_state.copyWith(error: turnError.toString(), turnPhase: TurnPhase.idle));
    }
  }

  /// Interrupt the live turn on the bound thread (best-effort).
  Future<void> interrupt() async {
    final threadId = client.currentThreadId;
    if (threadId == null) return;
    try {
      await client.turnInterrupt(threadId: threadId);
    } catch (_) {
      // Best-effort: the turn ends server-side regardless.
    }
  }

  /// Resolve a pending approval via `approval/resolve`; optimistic with
  /// revert-to-pending when the server could not deliver the decision.
  Future<void> resolveApproval(String itemId, bool approved) async {
    final entry = state.approvals.where((a) => a.itemId == itemId).firstOrNull;
    if (entry == null) return;
    entry.status = approved ? proto.ApprovalStatus.approved : proto.ApprovalStatus.denied;
    _commitApprovals();

    final scope = entry.allowedScopes.isNotEmpty ? entry.allowedScopes.first : proto.ApprovalScope.runOnce;
    try {
      final result = await client.approvalResolve(
        threadId: entry.threadId,
        itemId: itemId,
        approved: approved,
        scope: scope,
      );
      if (result.delivered == false) {
        entry.status = proto.ApprovalStatus.pending;
        _commitApprovals();
        throw StateError('approval not delivered');
      }
    } catch (_) {
      entry.status = proto.ApprovalStatus.pending;
      _commitApprovals();
      rethrow;
    }
  }

  void _commitApprovals() {
    final pending = _allApprovals.where((a) => a.status == proto.ApprovalStatus.pending).toList(growable: false);
    final statusMap = <String, proto.ApprovalStatus>{};
    for (final approval in _allApprovals) {
      statusMap[approval.itemId] = approval.status;
    }
    _emit(_state.copyWith(approvals: pending, approvalStatusByItemId: statusMap));
  }

  final List<proto.ApprovalRequest> _allApprovals = [];

  /// Cancel in-flight restore work, drop listeners, and close the client.
  Future<void> dispose() async {
    _generation += 1;
    await _notificationSub?.cancel();
    await _statusSub?.cancel();
    _notificationSub = null;
    _statusSub = null;
    _listeners.clear();
    await client.close();
  }

  // ── notification projection ───────────────────────────────────────────────

  void _handleNotification(NotificationFrame notification) {
    final params = notification.params ?? const <String, Object?>{};

    // Approvals, model-load, usage: out-of-band state (TS parity).
    switch (notification.method) {
      case HarnessNotification.itemCommandExecutionRequestApproval:
        _onApprovalNotification(params, isCommand: true);
        return;
      case HarnessNotification.itemFileChangeRequestApproval:
        _onApprovalNotification(params, isCommand: false);
        return;
      case HarnessNotification.modelLoadDelta:
        _emit(_state.copyWith(
          modelLoad: ModelLoadState(
            phase: _string(params['phase']),
            modelId: _optString(params['modelId']),
            downloadedBytes: _int(params['downloadedBytes']),
            totalBytes: _int(params['totalBytes']),
          ),
          turnPhase: TurnPhase.modelLoading,
        ));
        return;
      case HarnessNotification.modelLoadCompleted:
        _emit(_state.copyWith(clearModelLoad: true, turnPhase: TurnPhase.running));
        return;
      case HarnessNotification.turnCompleted:
        final usage = proto.TurnUsage.fromJson(params['usage']);
        _emit(_state.copyWith(turnUsage: usage, turnPhase: TurnPhase.idle));
        // Fall through to the projector so terminal parts close.
        break;
      case HarnessNotification.error:
        _emit(_state.copyWith(turnPhase: TurnPhase.idle));
        break;
      default:
        break;
    }

    // Live turn projection.
    final projector = _projector;
    if (projector == null) return;
    final terminated = projector.feed(notification);
    _emit(_state.copyWith(messages: List.unmodifiable([..._history, ...projector.messages])));
    if (terminated) {
      _projector = projector.finished && projector.messages.isEmpty ? null : projector;
    }
  }

  void _onApprovalNotification(Map<String, Object?> params, {required bool isCommand}) {
    final threadId = _string(params['threadId']);
    if (threadId != client.currentThreadId) return; // only the bound thread
    final itemId = _string(params['itemId']);
    if (_allApprovals.any((a) => a.itemId == itemId)) return;

    proto.ApprovalRequest entry;
    if (isCommand) {
      final planSnapshot = params['planSnapshot'];
      entry = proto.ApprovalRequest(
        itemId: itemId,
        threadId: threadId,
        kind: planSnapshot != null ? proto.ApprovalKind.plan : proto.ApprovalKind.command,
        command: _optString(params['command']),
        cwd: _optString(params['cwd']),
        reason: _optString(params['reason']),
        category: _optString(params['category']),
        allowedScopes: _scopes(params['allowedScopes']),
        planSnapshot: planSnapshot,
      );
    } else {
      final changes = (params['changes'] is List ? params['changes']! as List : const [])
          .whereType<Map<String, Object?>>()
          .map(
            (change) => proto.FileChangeApprovalChange(
              path: _string(change['path']),
              type: _string(change['type']),
              diff: _optString(change['diff']),
            ),
          )
          .toList(growable: false);
      entry = proto.ApprovalRequest(
        itemId: itemId,
        threadId: threadId,
        kind: proto.ApprovalKind.fileChange,
        changes: changes,
        allowedScopes: _scopes(params['allowedScopes']),
      );
    }
    _allApprovals.add(entry);
    _commitApprovals();
  }
}

String _string(Object? value) => value is String ? value : '';
String? _optString(Object? value) => value is String ? value : null;
int? _int(Object? value) => value is int ? value : null;

List<proto.ApprovalScope> _scopes(Object? value) => value is List
    ? value.whereType<String>().map(proto.ApprovalScope.fromWire).whereType<proto.ApprovalScope>().toList(growable: false)
    : const [];
