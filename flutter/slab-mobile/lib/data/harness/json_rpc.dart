/// JSON-RPC 2.0 wire helpers for the harness control plane.
///
/// Port of `packages/slab-core/src/harness/json-rpc.ts` (kept 1:1 so the two
/// clients cannot diverge on classification semantics): the server sends
/// responses (to our requests) and notifications (agent events); inbound
/// requests and invalid frames are ignored.
library;

import 'dart:convert';

/// Monotonic JSON-RPC request id (integer).
int _nextId = 1;
int nextRequestId() => _nextId++;

/// Outbound request envelope. Optional params are omitted on the wire.
Map<String, Object?> buildRequestFrame(Object id, String method, Object? params) => {
      'jsonrpc': '2.0',
      'id': id,
      'method': method,
      'params': ?params,
    };

String encodeFrame(Map<String, Object?> frame) => jsonEncode(frame);

/// Decode one text frame; `null` when it is not valid JSON.
Object? parseFrame(String? data) {
  if (data == null) return null;
  try {
    return jsonDecode(data);
  } catch (_) {
    return null;
  }
}

/// One classified inbound wire frame.
sealed class InboundFrame {}

/// Response to a prior request (result or error).
final class ResponseFrame extends InboundFrame {
  ResponseFrame({required this.id, required this.result});
  final Object id;
  final Object? result;
}

/// Error response to a prior request.
final class ErrorFrame extends InboundFrame {
  ErrorFrame({required this.id, required this.code, required this.message});
  final Object id;
  final int code;
  final String message;
}

/// Server-pushed notification (agent event delivery).
final class NotificationFrame extends InboundFrame {
  NotificationFrame({required this.method, this.params});
  final String method;
  final Map<String, Object?>? params;
}

/// Classify one inbound wire frame; `null` for invalid frames and inbound
/// requests (the server does not send requests to the client).
InboundFrame? classifyInbound(Object? decoded) {
  if (decoded is! Map<String, Object?>) return null;
  if (decoded['jsonrpc'] != '2.0') return null;
  final method = decoded['method'];
  if (method is String) {
    if (decoded['id'] != null) return null; // inbound request — ignored
    final params = decoded['params'];
    return NotificationFrame(method: method, params: params is Map<String, Object?> ? params : null);
  }
  final id = decoded['id'];
  final error = decoded['error'];
  if (error is Map<String, Object?> && id != null) {
    return ErrorFrame(
      id: id,
      code: error['code'] is int ? error['code']! as int : -32000,
      message: error['message'] is String ? error['message']! as String : 'harness error',
    );
  }
  if (decoded.containsKey('result') && id != null) {
    return ResponseFrame(id: id, result: decoded['result']);
  }
  return null;
}
