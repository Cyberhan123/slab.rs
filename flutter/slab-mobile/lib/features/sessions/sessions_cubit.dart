/// Sessions list state machine: bootstrap (setup gate → list), 5s health
/// poll, create/rename/delete. Navigation stays in the view; the cubit only
/// flags a setup redirect (`setupRedirect`) for the view to act on once.
library;

import 'dart:async';

import 'package:flutter_bloc/flutter_bloc.dart';

import '../../data/rest_client.dart';

class SessionsState {
  const SessionsState({
    this.sessions,
    this.error,
    this.setupChecked = false,
    this.setupRedirect = false,
  });

  /// Null while the first load is in flight.
  final List<SessionRecord>? sessions;

  /// Last refresh error (toString'd for display); null when reachable.
  final Object? error;

  /// True once the setup-gate check finished without redirecting.
  final bool setupChecked;

  /// Set when the server reports `initialized: false`; the view navigates to
  /// `/setup` in a BlocListener.
  final bool setupRedirect;

  SessionsState copyWith({
    List<SessionRecord>? sessions,
    Object? error,
    bool? setupChecked,
    bool? setupRedirect,
  }) =>
      SessionsState(
        sessions: sessions ?? this.sessions,
        error: error ?? this.error,
        setupChecked: setupChecked ?? this.setupChecked,
        setupRedirect: setupRedirect ?? this.setupRedirect,
      );
}

class SessionsCubit extends Cubit<SessionsState> {
  SessionsCubit({SlabRestClient? client})
      : _client = client,
        super(const SessionsState());

  final SlabRestClient? _client;
  Timer? _poll;
  bool _started = false;

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
      emit(SessionsState(sessions: sessions, setupChecked: true, setupRedirect: state.setupRedirect));
    } on Object catch (error) {
      emit(state.copyWith(error: error.toString()));
    }
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
