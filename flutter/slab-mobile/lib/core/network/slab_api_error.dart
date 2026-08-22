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
  const SlabRestException(this.message, this.statusCode, {this.i18nKey, this.i18nParams});

  final String message;

  /// `null` when the request never got a response (connect error, timeout).
  final int? statusCode;

  /// Server i18n ref (`server.errors.*`) for the message field, when the
  /// envelope carried one — `request_errors.dart` translates it.
  final String? i18nKey;
  final Map<String, Object?>? i18nParams;

  @override
  String toString() => statusCode == null ? message : '$message (HTTP $statusCode)';
}

/// Extracts the message plus its server i18n ref from a decoded error body.
///
/// slab-server error envelopes look like `{"message": "...", "i18n":
/// {"message": {"key": "server.errors.x", "params": {...}}}}`.
(String, String?, Map<String, Object?>?) slabApiErrorWithI18n(Object? body) {
  if (body is! Map<String, Object?>) return ('request failed', null, null);
  final message = body['message'] is String ? body['message']! as String : 'request failed';
  final i18n = body['i18n'];
  if (i18n is Map<String, Object?>) {
    final ref = i18n['message'];
    if (ref is Map<String, Object?> && ref['key'] is String) {
      final params = ref['params'];
      return (
        message,
        ref['key']! as String,
        params is Map<String, Object?> ? params : null,
      );
    }
  }
  return (message, null, null);
}
