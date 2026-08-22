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

/// Whether a compaction marker represents an automatic run or a manual
/// `/compact` invocation.
enum CompactionMode { auto, manual }

enum CompactionPhase { compacting, compacted }

/// A session-scoped compaction divider rendered in the message stream.
/// Survives reconnects (state lives in the controller, mirroring TS).
class CompactionMarker {
  const CompactionMarker({required this.id, required this.mode, required this.phase, required this.threadId});
  final String id;
  final CompactionMode mode;
  final CompactionPhase phase;
  final String threadId;

  CompactionMarker copyWith({CompactionPhase? phase}) =>
      CompactionMarker(id: id, mode: mode, phase: phase ?? this.phase, threadId: threadId);
}

/// One-shot error from a stream action (compact/fork) surfaced as a toast;
/// distinct from `error` (restore/turn failures rendered in-stream).
class ActionError {
  const ActionError({required this.kind, required this.message});
  final String kind; // "compact" | "fork"
  final String message;
}

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
    this.planMode = false,
    this.commands = const [],
    this.compactionMarkers = const [],
    this.userMessageTurnIndex = const {},
    this.actionError,
    this.isCompacting = false,
    this.isForking = false,
    this.isRollingBack = false,
    this.restoreVersion = 0,
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

  /// Plan mode: turns are sent with `agentType: "plan"`; approving a plan
  /// approval clears it (rejection keeps it on).
  final bool planMode;

  /// `/`-command registry snapshot from `command/list` (drives the menu).
  final List<proto.CommandInfo> commands;

  /// In-stream compaction dividers (auto + manual), oldest first.
  final List<CompactionMarker> compactionMarkers;

  /// userMessage itemId → numeric turn index (rollback affordance driver).
  final Map<String, int> userMessageTurnIndex;

  final ActionError? actionError;
  final bool isCompacting;
  final bool isForking;
  final bool isRollingBack;

  /// Bumped on fork/rollback re-binds (the desktop remounts the pane on it;
  /// mobile uses it to reset per-thread UI state like drafts).
  final int restoreVersion;

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
    bool? planMode,
    List<proto.CommandInfo>? commands,
    List<CompactionMarker>? compactionMarkers,
    Map<String, int>? userMessageTurnIndex,
    ActionError? actionError,
    bool clearActionError = false,
    bool? isCompacting,
    bool? isForking,
    bool? isRollingBack,
    int? restoreVersion,
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
        planMode: planMode ?? this.planMode,
        commands: commands ?? this.commands,
        compactionMarkers: compactionMarkers ?? this.compactionMarkers,
        userMessageTurnIndex: userMessageTurnIndex ?? this.userMessageTurnIndex,
        actionError: clearActionError ? null : (actionError ?? this.actionError),
        isCompacting: isCompacting ?? this.isCompacting,
        isForking: isForking ?? this.isForking,
        isRollingBack: isRollingBack ?? this.isRollingBack,
        restoreVersion: restoreVersion ?? this.restoreVersion,
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
  Map<String, int> _userMessageTurnIndex = const {};
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

      // Fetch the command-registry snapshot (drives the `/`-menu). Fire-and-
      // forget: commands must not gate the restore path, and a failure just
      // leaves the menu on its last (possibly empty) snapshot.
      unawaited(client.commandList().then((commands) {
        if (!isCurrent()) return;
        _emit(_state.copyWith(commands: commands));
      }).catchError((Object _) {}));

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
        userMessageTurnIndex: _userMessageTurnIndex,
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
      _userMessageTurnIndex = const {};
      _emit(_state.copyWith(
        messages: const [],
        threadId: null,
        approvals: const [],
        approvalStatusByItemId: const {},
        userMessageTurnIndex: const {},
        connection: ConnectionPhase.ready,
      ));
    }
  }

  void _bindThread(proto.Thread thread) {
    client.currentThreadId = thread.id;
    client.lastTurnIndex = computeLastTurnIndex(thread);
    _history = projectItems(thread.turns.expand((turn) => turn.items), baseUrl);
    _userMessageTurnIndex = buildUserMessageTurnIndex(thread);
    _projector = null;
  }

  /// Programmatically start a turn: ensures the socket is open, lazily binds a
  /// thread (`thread/start` on a fresh session), and fires `turn/start` with
  /// text + image inputs and the composer's turn options. Plan mode rides
  /// along as `agentType: "plan"`.
  Future<void> send({
    required String text,
    List<String> imageUrls = const [],
    String? effort,
    String? permissionMode,
    String? modelId,
  }) async {
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
    final parts = <UiPart>[
      if (text.isNotEmpty) TextUiPart(text: text),
      for (final url in imageUrls)
        if (resolveImageUrl(url, baseUrl) case final resolved?) ImageUiPart(url: resolved),
    ];
    final userMessage = ChatMessage(
      id: 'local-${DateTime.now().microsecondsSinceEpoch}',
      fromUser: true,
      parts: parts,
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
        input: [
          if (text.isNotEmpty) proto.textUserInput(text),
          for (final url in imageUrls) proto.imageUserInput(url),
        ],
        model: effectiveModel,
        effort: effort,
        permissionMode: permissionMode,
        agentType: state.planMode ? 'plan' : null,
      );
    } catch (turnError) {
      _emit(_state.copyWith(error: turnError.toString(), turnPhase: TurnPhase.idle));
    }
  }

  /// Text-only convenience wrapper over [send].
  Future<void> sendText(String text, {String? modelId}) =>
      send(text: text, modelId: modelId);

  /// Toggle plan mode. Resolving a plan approval clears it atomically.
  void setPlanMode(bool enabled) => _emit(_state.copyWith(planMode: enabled));

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
    // Approving a plan clears plan mode: the next turn/start carries no
    // `agentType`, so it runs as the default agent with the full tool set.
    // Rejection keeps plan mode on.
    if (approved && entry.kind == proto.ApprovalKind.plan) {
      _emit(_state.copyWith(planMode: false));
    }

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

  /// Manually compact the bound thread (`/compact`): `thread/compact/start`
  /// then re-resume so the pane re-renders with the compacted history. A
  /// manual marker rides the stream; failure drops it and surfaces an
  /// action error.
  Future<void> compactThread() async {
    final threadId = client.currentThreadId;
    if (threadId == null) {
      _emit(_state.copyWith(actionError: const ActionError(kind: 'compact', message: 'no active thread')));
      return;
    }
    _emit(_state.copyWith(
      clearError: true,
      clearActionError: true,
      isCompacting: true,
      compactionMarkers: [
        ...state.compactionMarkers,
        CompactionMarker(
          id: 'manual:$threadId:${DateTime.now().millisecondsSinceEpoch}',
          mode: CompactionMode.manual,
          phase: CompactionPhase.compacting,
          threadId: threadId,
        ),
      ],
    ));
    final markerId = state.compactionMarkers.last.id;
    try {
      await client.threadCompactStart(threadId: threadId);
      final thread = await client.threadResume(threadId: threadId);
      _bindThread(thread);
      _emit(_state.copyWith(
        threadId: thread.id,
        messages: List.unmodifiable(_history),
        userMessageTurnIndex: _userMessageTurnIndex,
        compactionMarkers: [
          for (final marker in state.compactionMarkers)
            if (marker.id == markerId) marker.copyWith(phase: CompactionPhase.compacted) else marker,
        ],
        isCompacting: false,
      ));
    } catch (compactError) {
      _emit(_state.copyWith(
        compactionMarkers: [for (final marker in state.compactionMarkers) if (marker.id != markerId) marker],
        isCompacting: false,
        actionError: ActionError(kind: 'compact', message: compactError.toString()),
      ));
    }
  }

  /// Fork the bound thread (`/fork`): the child lives under the same slab
  /// session; the socket re-binds to it and its (copied) history re-renders.
  /// The parent is retained server-side but unreachable from the UI.
  Future<void> forkThread() async {
    final threadId = client.currentThreadId;
    if (threadId == null) {
      _emit(_state.copyWith(actionError: const ActionError(kind: 'fork', message: 'no active thread')));
      return;
    }
    _emit(_state.copyWith(clearError: true, clearActionError: true, isForking: true));
    try {
      final child = await client.threadFork(threadId: threadId);
      final thread = await client.threadResume(threadId: child.id);
      _bindThread(thread);
      _emit(_state.copyWith(
        threadId: thread.id,
        messages: List.unmodifiable(_history),
        userMessageTurnIndex: _userMessageTurnIndex,
        restoreVersion: state.restoreVersion + 1,
        isForking: false,
      ));
    } catch (forkError) {
      _emit(_state.copyWith(
        isForking: false,
        actionError: ActionError(kind: 'fork', message: forkError.toString()),
      ));
    }
  }

  /// Retract `turnIndex` and every later turn (`thread/rollback` with
  /// `toTurnId: n-1`), then re-resume. Turn 0 is a no-op.
  Future<void> rollbackFromTurn(int turnIndex) async {
    final threadId = client.currentThreadId;
    if (threadId == null || turnIndex <= 0) return;
    _emit(_state.copyWith(clearError: true, isRollingBack: true));
    try {
      await client.threadRollback(threadId: threadId, toTurnId: (turnIndex - 1).toString());
      final thread = await client.threadResume(threadId: threadId);
      _bindThread(thread);
      _emit(_state.copyWith(
        threadId: thread.id,
        messages: List.unmodifiable(_history),
        userMessageTurnIndex: _userMessageTurnIndex,
        restoreVersion: state.restoreVersion + 1,
        isRollingBack: false,
      ));
    } catch (rollbackError) {
      _emit(_state.copyWith(error: rollbackError.toString(), isRollingBack: false));
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
      case HarnessNotification.contextCompacting:
        _onContextCompacting(params);
        return;
      case HarnessNotification.contextCompacted:
        _onContextCompacted(params);
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

  /// Context-compaction lifecycle: an in-progress auto-compaction adds a
  /// "compacting" marker (one per thread); the terminal event flips it to
  /// "compacted" or removes it (status "skipped" = a started compaction that
  /// didn't shrink).
  void _onContextCompacting(Map<String, Object?> params) {
    final threadId = _string(params['threadId']);
    if (threadId != client.currentThreadId) return;
    final alreadyCompacting = state.compactionMarkers.any(
      (marker) =>
          marker.mode == CompactionMode.auto && marker.phase == CompactionPhase.compacting && marker.threadId == threadId,
    );
    if (alreadyCompacting) return;
    _emit(_state.copyWith(
      compactionMarkers: [
        ...state.compactionMarkers,
        CompactionMarker(
          id: 'auto:$threadId:${DateTime.now().millisecondsSinceEpoch}',
          mode: CompactionMode.auto,
          phase: CompactionPhase.compacting,
          threadId: threadId,
        ),
      ],
    ));
  }

  void _onContextCompacted(Map<String, Object?> params) {
    final threadId = _string(params['threadId']);
    if (threadId != client.currentThreadId) return;
    if (_string(params['status']) == 'skipped') {
      // Started but did not compact — drop the in-progress marker.
      _emit(_state.copyWith(
        compactionMarkers: [
          for (final marker in state.compactionMarkers)
            if (!(marker.mode == CompactionMode.auto && marker.threadId == threadId && marker.phase == CompactionPhase.compacting))
              marker,
        ],
      ));
    } else {
      _emit(_state.copyWith(
        compactionMarkers: [
          for (final marker in state.compactionMarkers)
            if (marker.mode == CompactionMode.auto && marker.threadId == threadId && marker.phase == CompactionPhase.compacting)
              marker.copyWith(phase: CompactionPhase.compacted)
            else
              marker,
        ],
      ));
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
