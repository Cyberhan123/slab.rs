/// Fake harness socket: auto-responds to the `initialize` handshake, records
/// every outbound request, lets tests script responses and push notifications,
/// and can simulate an unexpected drop (stream done) for reconnect tests.
library;

import 'dart:async';
import 'dart:convert';

import 'package:slab_mobile/data/harness/harness_client.dart';
import 'package:slab_mobile/data/harness/harness_methods.dart';

class FakeSlabSocket implements SlabSocket {
  FakeSlabSocket({this.onRequest});

  /// Scripted responder; `null` → respond with `{}`. Assignable after
  /// construction so tests can flip behavior between phases.
  Map<String, Object?>? Function(String method, Map<String, Object?>? params)? onRequest;

  /// When set and non-null for a method, respond with a JSON-RPC error frame
  /// (message) instead of a result — used for e.g. "no thread to resume".
  String? Function(String method)? failWith;

  final _incoming = StreamController<String>.broadcast();
  final List<Map<String, Object?>> requests = [];
  bool dropped = false;

  static int instanceCount = 0;
  late final int instance = instanceCount++;

  @override
  Stream<String> get incoming => _incoming.stream;

  @override
  void send(String data) {
    final frame = jsonDecode(data);
    if (frame is! Map<String, Object?>) return;
    requests.add(frame);
    final method = frame['method'] as String;
    if (method == HarnessMethod.initialize) {      serverRespond(frame['id'], {
        'serverInfo': {'name': 'slab-server-test', 'version': '0.0.0'},
      });
      return;
    }
    final failure = failWith?.call(method);
    if (failure != null) {
      serverError(frame['id'], failure);
      return;
    }
    final responder = onRequest;
    if (responder != null) {
      final params = frame['params'];
      serverRespond(frame['id'], responder(method, params is Map<String, Object?> ? params : null) ?? const <String, Object?>{});
    }
  }

  @override
  Future<void> close() async {
    dropped = true;
    await _incoming.close();
  }

  /// Simulate an unexpected transport drop (no user close).
  void drop() {
    dropped = true;
    _incoming.close();
  }

  void serverRespond(Object? id, Map<String, Object?> result) => _addFrame({
        'jsonrpc': '2.0',
        'id': id,
        'result': result,
      });

  void serverError(Object? id, String message, {int code = -32000}) => _addFrame({
        'jsonrpc': '2.0',
        'id': id,
        'error': {'code': code, 'message': message},
      });

  void push(String method, [Map<String, Object?>? params]) => _addFrame({
        'jsonrpc': '2.0',
        'method': method,
        'params': ?params,
      });

  void _addFrame(Map<String, Object?> frame) {
    if (_incoming.isClosed) return; // late frames after a drop are dropped
    _incoming.add(jsonEncode(frame));
  }

  int countRequests(String method) => requests.where((r) => r['method'] == method).length;
}

/// Hands out the given sockets in order (index 0 first); extra `open()` calls
/// beyond the list throw.
class FakeSocketFactory {
  FakeSocketFactory(this._sockets);
  final List<FakeSlabSocket> _sockets;
  final List<FakeSlabSocket> created = [];
  int _next = 0;

  Future<SlabSocket> call(Uri uri) async {
    if (_next >= _sockets.length) throw StateError('no more fake sockets');
    final socket = _sockets[_next++];
    created.add(socket);
    return socket;
  }
}
