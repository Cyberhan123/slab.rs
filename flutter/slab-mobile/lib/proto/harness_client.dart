/// Persistent JSON-RPC 2.0 WebSocket client for the `/v1/agents/harness`
/// control plane — the Dart port of
/// `packages/slab-core/src/harness/harness-client.ts` **plus mobile reconnect**.
///
/// One client owns one socket for one slab session: it opens the connection,
/// completes the mandatory `initialize` handshake, correlates requests by id,
/// and dispatches server-pushed notifications. Threads are harness-local and
/// socket-scoped, so the client tracks `currentThreadId` and `lastTurnIndex`
/// (used to separate replayed history from live-turn events).
///
/// Reconnect delta vs. the TS client (which deliberately has none): on an
/// unexpected close the client transitions to `reconnecting`, redials with
/// exponential backoff (500ms·2^n, capped 30s, ±30% jitter), re-runs the
/// `initialize` handshake, and returns to `ready` — the owning controller then
/// re-binds the thread via `thread/resume` (transport restore is the client's
/// job; thread lifecycle stays with the controller, mirroring the TS split).
library;

import 'dart:async';
import 'dart:math';

import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import 'harness_methods.dart';
import 'harness_types.dart' as proto;
import 'json_rpc.dart';

// ── Socket seam (tests inject fakes; prod uses IOWebSocketChannel) ──────────

/// Minimal socket surface the client needs.
abstract class SlabSocket {
  Stream<String> get incoming;
  void send(String data);
  Future<void> close();
}

typedef SlabSocketFactory = Future<SlabSocket> Function(Uri uri);

final class IoWebSocketSocket implements SlabSocket {
  IoWebSocketSocket(this._channel);
  final WebSocketChannel _channel;

  @override
  Stream<String> get incoming => _channel.stream.cast<String>();

  @override
  void send(String data) => _channel.sink.add(data);

  @override
  Future<void> close() => _channel.sink.close();
}

Future<SlabSocket> _defaultSocketFactory(Uri uri) async {
  final channel = IOWebSocketChannel.connect(uri);
  await channel.ready;
  return IoWebSocketSocket(channel);
}

// ── Client ──────────────────────────────────────────────────────────────────

enum HarnessStatus { idle, opening, ready, reconnecting, closed }

/// Cap on how long we wait for the WS dial before failing the attempt.
const _wsOpenTimeout = Duration(seconds: 5);
/// Cap on how long we wait for any single JSON-RPC response.
const _requestTimeout = Duration(seconds: 30);
const _backoffBaseDefault = Duration(milliseconds: 500);
const _backoffCap = Duration(seconds: 30);

/// Build the harness WS URL: `ws(s)://<origin>/v1/agents/harness?token=<sessionId>`.
Uri harnessWebSocketUri(Uri baseUrl, String sessionId) => baseUrl.replace(
      scheme: baseUrl.scheme == 'https' || baseUrl.scheme == 'wss' ? 'wss' : 'ws',
      path: '/v1/agents/harness',
      query: 'token=${Uri.encodeComponent(sessionId)}',
      fragment: '',
    );

class _Pending {
  _Pending(this.completer, this.timer);
  final Completer<Object?> completer;
  final Timer timer;
}

class HarnessClient {
  HarnessClient({
    required Uri baseUrl,
    required this.sessionId,
    SlabSocketFactory? socketFactory,
    this.clientName = 'slab-mobile',
    this.clientVersion = '1.0',
    Random? random,
    Duration? backoffBase,
  })  : _wsUri = harnessWebSocketUri(baseUrl, sessionId),
        _socketFactory = socketFactory ?? _defaultSocketFactory,
        _random = random ?? Random(),
        _backoffBase = backoffBase ?? _backoffBaseDefault;

  final Uri _wsUri;
  final String sessionId;
  final String clientName;
  final String clientVersion;
  final SlabSocketFactory _socketFactory;
  final Random _random;
  final Duration _backoffBase;

  final _statusController = StreamController<HarnessStatus>.broadcast();
  final _notificationController = StreamController<NotificationFrame>.broadcast();
  final _pending = <int, _Pending>{};

  SlabSocket? _socket;
  HarnessStatus _status = HarnessStatus.idle;
  Future<void>? _opening;
  bool _userClosed = false;
  Timer? _reconnectTimer;
  int _reconnectAttempts = 0;

  /// The harness thread id currently bound on this socket (`thread/start` or
  /// `thread/resume`). Used as the `threadId` for `turn/start` etc.
  String? currentThreadId;

  /// Highest numeric `turnId` seen on the current thread. Live-turn events
  /// have `turnId > lastTurnIndex`; replayed history has `turnId <= lastTurnIndex`.
  int lastTurnIndex = -1;

  HarnessStatus get status => _status;
  Stream<HarnessStatus> get statusStream => _statusController.stream;
  Stream<NotificationFrame> get notifications => _notificationController.stream;

  void _setStatus(HarnessStatus status) {
    if (_status == status) return;
    _status = status;
    if (!_statusController.isClosed) _statusController.add(status);
  }

  /// Connect (if needed) and complete the `initialize` handshake. Idempotent;
  /// also the await point while a reconnect is in flight.
  Future<void> open() {
    if (_status == HarnessStatus.ready) return Future.value();
    return _opening ??= _openFlow().whenComplete(() => _opening = null);
  }

  Future<void> _openFlow() async {
    _setStatus(HarnessStatus.opening);
    await _dial();
    _reconnectAttempts = 0;
    _setStatus(HarnessStatus.ready);
  }

  Future<void> _dial() async {
    final socket = await _socketFactory(_wsUri).timeout(_wsOpenTimeout);
    _socket = socket;
    socket.incoming.listen(_handleData, onDone: _handleClosed, onError: (Object _) => _handleClosed());
    // Mandatory handshake — every other method is rejected until this returns.
    await _request(HarnessMethod.initialize, {
      'clientInfo': {'name': clientName, 'version': clientVersion},
    }, raw: true);
  }

  void _handleData(String data) {
    final classified = classifyInbound(parseFrame(data));
    switch (classified) {
      case ResponseFrame(:final id, :final result):
        _settle(id is int ? id : -1, value: result);
      case ErrorFrame(:final id, :final message):
        _settle(id is int ? id : -1, error: HarnessRpcException(message));
      case NotificationFrame():
        if (!_notificationController.isClosed) _notificationController.add(classified);
      case null:
        break; // invalid frames / inbound requests are ignored
    }
  }

  void _handleClosed() {
    _reconnectTimer?.cancel();
    _socket = null;
    for (final entry in _pending.values) {
      entry.timer.cancel();
      entry.completer.completeError(StateError('harness socket closed'));
    }
    _pending.clear();
    if (_userClosed) {
      _setStatus(HarnessStatus.closed);
      return;
    }
    _setStatus(HarnessStatus.reconnecting);
    _scheduleReconnect();
  }

  void _scheduleReconnect() {
    _reconnectAttempts += 1;
    final exponent = min(_reconnectAttempts - 1, 10);
    final delayMs = min(_backoffBase.inMilliseconds * (1 << exponent), _backoffCap.inMilliseconds);
    // ±30% jitter so multiple clients do not redial in lockstep.
    final jitter = 1.0 + (_random.nextDouble() * 0.6 - 0.3);
    _reconnectTimer = Timer(Duration(milliseconds: (delayMs * jitter).round()), () {
      if (_userClosed) return;
      // Reuse the shared open() gate; a failure falls back into _handleClosed.
      open().catchError((Object _) {});
    });
  }

  void _settle(int id, {Object? value, Object? error}) {
    final entry = _pending.remove(id);
    if (entry == null) return;
    entry.timer.cancel();
    if (error != null) {
      entry.completer.completeError(error);
    } else {
      entry.completer.complete(value);
    }
  }

  /// Send a JSON-RPC request and await its `result`. Ensures the socket is
  /// open (and initialized) first, except for `initialize` itself.
  Future<Object?> sendRequest(String method, [Map<String, Object?>? params]) =>
      _request(method, params, raw: false);

  Future<Object?> _request(String method, Map<String, Object?>? params, {required bool raw}) async {
    if (!raw) await open();
    final socket = _socket;
    if (socket == null) throw StateError('harness socket is not open');
    final id = nextRequestId();
    final completer = Completer<Object?>();
    final timer = Timer(_requestTimeout, () {
      _settle(id, error: TimeoutException('harness request timed out: $method'));
    });
    _pending[id] = _Pending(completer, timer);
    socket.send(encodeFrame(buildRequestFrame(id, method, params)));
    return completer.future;
  }

  // ── Method wrappers (phase-1 subset) ──────────────────────────────────────

  /// `thread/start` → the started thread.
  Future<proto.Thread> threadStart({String? model}) async => proto.Thread.fromJson(
      _expectObject(await sendRequest(HarnessMethod.threadStart, proto.threadStartParams(model: model)))['thread']! as Map<String, Object?>);

  /// `thread/resume` → the resumed thread.
  Future<proto.Thread> threadResume({String? threadId}) async => proto.Thread.fromJson(
      _expectObject(await sendRequest(HarnessMethod.threadResume, proto.threadResumeParams(threadId: threadId)))['thread']! as Map<String, Object?>);

  /// `thread/list` → threads for the session.
  Future<List<proto.Thread>> threadList() async {
    final result = _expectObject(await sendRequest(HarnessMethod.threadList));
    return (result['data'] is List ? result['data']! as List : const [])
        .whereType<Map<String, Object?>>()
        .map(proto.Thread.fromJson)
        .toList(growable: false);
  }

  /// `turn/start` → the ack; the turn's events arrive as notifications.
  Future<void> turnStart({required String threadId, required List<Map<String, Object?>> input, required String model}) async {
    await sendRequest(HarnessMethod.turnStart, proto.turnStartParams(threadId: threadId, input: input, model: model));
  }

  /// `turn/interrupt` (best-effort; turnId "0" mirrors the TS client).
  Future<void> turnInterrupt({required String threadId}) async {
    await sendRequest(HarnessMethod.turnInterrupt, proto.turnInterruptParams(threadId: threadId, turnId: '0'));
  }

  /// `approval/resolve` with a persistence scope.
  Future<proto.ApprovalResolveResult> approvalResolve({
    required String threadId,
    required String itemId,
    required bool approved,
    proto.ApprovalScope? scope,
  }) async =>
      proto.ApprovalResolveResult.fromJson(_expectObject(await sendRequest(
          HarnessMethod.approvalResolve,
          proto.approvalResolveParams(threadId: threadId, itemId: itemId, approved: approved, scope: scope))));

  /// Close the socket, reject pending requests, and stop reconnecting.
  Future<void> close() async {
    _userClosed = true;
    _reconnectTimer?.cancel();
    final socket = _socket;
    _handleClosed();
    await socket?.close();
    await _statusController.close();
    await _notificationController.close();
  }
}

class HarnessRpcException implements Exception {
  HarnessRpcException(this.message);
  final String message;
  @override
  String toString() => message;
}

Map<String, Object?> _expectObject(Object? value) {
  if (value is Map<String, Object?>) return value;
  throw const FormatException('harness response was not an object');
}
