/// Sessions list state machine: bootstrap (setup gate → list), 5s health
/// poll, create/rename/delete. Navigation stays in the view; the cubit only
/// flags a setup redirect (`setupRedirect`) for the view to act on once.
library;

import 'dart:async';

import 'package:flutter_bloc/flutter_bloc.dart';

import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/domain/session_labels.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';

class SessionsState {
  const SessionsState({
    this.sessions,
    this.labels = const {},
    this.error,
    this.setupChecked = false,
    this.setupRedirect = false,
  });

  /// Null while the first load is in flight.
  final List<SessionRecord>? sessions;

  /// Local label overrides (first-prompt titles) keyed by session id.
  final Map<String, String> labels;

  /// Last refresh error (toString'd for display); null when reachable.
  final Object? error;

  /// True once the setup-gate check finished without redirecting.
  final bool setupChecked;

  /// Set when the server reports `initialized: false`; the view navigates to
  /// `/setup` in a BlocListener.
  final bool setupRedirect;

  SessionsState copyWith({
    List<SessionRecord>? sessions,
    Map<String, String>? labels,
    Object? error,
    bool? setupChecked,
    bool? setupRedirect,
  }) => SessionsState(
    sessions: sessions ?? this.sessions,
    labels: labels ?? this.labels,
    error: error ?? this.error,
    setupChecked: setupChecked ?? this.setupChecked,
    setupRedirect: setupRedirect ?? this.setupRedirect,
  );
}

class SessionsCubit extends Cubit<SessionsState> {
  SessionsCubit({SlabRestClient? client, SessionMetaDao? sessionMeta})
    : _client = client,
      _sessionMeta = sessionMeta,
      super(const SessionsState());

  final SlabRestClient? _client;

  /// Local label overrides + the current-session pointer (null in tests that
  /// don't exercise persistence).
  final SessionMetaDao? _sessionMeta;
  Timer? _poll;
  bool _started = false;
  bool _bootstrappedOnce = false;

  /// Idempotent so the page's BlocProvider wiring can call it unconditionally.
  void start() {
    if (_started) return;
    _started = true;
    _bootstrap();
    _poll = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
  }

  Future<void> _bootstrap() async {
    await _checkSetupGate();
    await refresh();
    // Assistant bootstrap parity: a server with no sessions gets one created
    // so the chat surface always has a conversation to open (once per page
    // load, not on every refresh).
    if (!_bootstrappedOnce) {
      _bootstrappedOnce = true;
      final sessions = state.sessions;
      if (sessions != null && sessions.isEmpty && _client != null) {
        await _client.createSession();
        await refresh();
      }
    }
  }

  Future<void> _checkSetupGate() async {
    final client = _client;
    if (client == null) return;
    try {
      final setup = await client.getSetupStatus();
      if (!setup.initialized) {
        emit(state.copyWith(setupRedirect: true));
        return;
      }
    } on SlabRestException {
      // Unreachable server is NOT "not initialized" (SetupGuard parity) —
      // the sessions list + health dot surface it instead.
    }
    emit(state.copyWith(setupChecked: true));
  }

  Future<void> refresh() async {
    final client = _client;
    if (client == null) return;
    try {
      final sessions = await client.listSessions();
      final labels = await _sessionMeta?.labels() ?? const <String, String>{};
      // Drop overrides for sessions that no longer exist server-side.
      await _sessionMeta?.retainOnly(
        sessions.map((record) => record.id).toSet(),
      );
      emit(
        SessionsState(
          sessions: sessions,
          labels: labels,
          setupChecked: true,
          setupRedirect: state.setupRedirect,
        ),
      );
    } on Object catch (error) {
      emit(state.copyWith(error: error.toString()));
    }
  }

  /// First-prompt auto-title: only when the server name is still a default.
  Future<void> noteUserPrompt({
    required String sessionId,
    required String prompt,
    required String serverName,
  }) async {
    final dao = _sessionMeta;
    if (dao == null) return;
    final existing = state.labels[sessionId];
    if (existing != null && existing.isNotEmpty) return;
    if (!isDefaultSessionLabel(serverName)) return;
    final label = createConversationLabel(prompt, serverName);
    await dao.upsertLabel(sessionId, label);
    emit(state.copyWith(labels: {...state.labels, sessionId: label}));
  }

  /// Creates a session and returns it (the view navigates to its chat).
  Future<SessionRecord> create() async {
    final record = await _client!.createSession();
    await refresh();
    return record;
  }

  Future<void> rename({required String id, required String name}) async {
    await _client?.renameSession(id: id, name: name);
    await refresh();
  }

  Future<void> delete(String id) async {
    await _client?.deleteSession(id);
    await refresh();
  }

  @override
  Future<void> close() async {
    _poll?.cancel();
    await super.close();
  }
}
