/// App-wide REST error type. Every non-2xx response funnels through here so
/// call sites (`setup_gate_page`, `sessions_page`, …) keep one catch type
/// regardless of the underlying transport.
library;

/// Extracts the human-readable message from a decoded error body.
///
/// slab-server error envelopes look like `{"message": "..."}` (optionally with
/// `code`/`param`/`request_id` siblings); anything else falls back to a
/// generic message so the UI never renders an empty string.
String slabApiErrorMessage(Object? body) {
  if (body is Map<String, Object?> && body['message'] is String) {
    return body['message']! as String;
  }
  return 'request failed';
}

/// Thrown for HTTP-level failures (non-2xx) and transport failures
/// (`statusCode == null`) alike.
class SlabRestException implements Exception {
  const SlabRestException(this.message, this.statusCode);

  final String message;

  /// `null` when the request never got a response (connect error, timeout).
  final int? statusCode;

  @override
  String toString() => statusCode == null ? message : '$message (HTTP $statusCode)';
}
