/// Thin REST client for the slab-server `/v1` surface the mobile app needs
/// (health probe, setup gate, session management). The harness WS lives in
/// `lib/proto/harness_client.dart`; this file deliberately stays HTTP-only.
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

/// `GET /health` → `{"status": "ok", "version": "..."}`.
class HealthStatus {
  const HealthStatus({required this.ok, this.version});
  final bool ok;
  final String? version;
}

/// `GET /v1/setup/status` — phase 1 consumes only `initialized`; the
/// component detail feeds the desktop setup wizard.
class SetupStatus {
  const SetupStatus({required this.initialized, this.runtimePayloadInstalled = false});
  final bool initialized;
  final bool runtimePayloadInstalled;

  static SetupStatus fromJson(Map<String, Object?> json) => SetupStatus(
        initialized: json['initialized'] is bool ? json['initialized']! as bool : false,
        runtimePayloadInstalled:
            json['runtime_payload_installed'] is bool ? json['runtime_payload_installed']! as bool : false,
      );
}

/// `SessionResponse` on the wire (snake_case except id/name).
class SessionRecord {
  const SessionRecord({required this.id, required this.name, required this.createdAt, required this.updatedAt});
  final String id;
  final String name;
  final String createdAt;
  final String updatedAt;

  static SessionRecord fromJson(Map<String, Object?> json) => SessionRecord(
        id: json['id'] is String ? json['id']! as String : '',
        name: json['name'] is String ? json['name']! as String : '',
        createdAt: json['created_at'] is String ? json['created_at']! as String : '',
        updatedAt: json['updated_at'] is String ? json['updated_at']! as String : '',
      );
}

class SlabRestException implements Exception {
  const SlabRestException(this.message, this.statusCode);
  final String message;
  final int? statusCode;
  @override
  String toString() => statusCode == null ? message : '$message (HTTP $statusCode)';
}

class SlabRestClient {
  SlabRestClient({required Uri baseUrl, String? bearerToken, http.Client? httpClient})
      : _baseUrl = baseUrl,
        _headers = {
          if (bearerToken != null && bearerToken.isNotEmpty) 'Authorization': 'Bearer $bearerToken',
        },
        _httpClient = httpClient ?? http.Client();

  final Uri _baseUrl;
  final Map<String, String> _headers;
  final http.Client _httpClient;

  Uri _uri(String path) => _baseUrl.replace(path: path, query: '', fragment: '');

  Map<String, Object?> _decode(http.Response response) {
    Object? body;
    try {
      body = jsonDecode(utf8.decode(response.bodyBytes));
    } catch (_) {
      body = null;
    }
    if (response.statusCode < 200 || response.statusCode >= 300) {
      final message = body is Map<String, Object?> && body['message'] is String
          ? body['message']! as String
          : 'request failed';
      throw SlabRestException(message, response.statusCode);
    }
    if (body is! Map<String, Object?>) {
      throw const SlabRestException('unexpected response shape', null);
    }
    return body;
  }

  /// Heartbeat probe — `false` on any transport error (never throws).
  Future<HealthStatus> probeHealth() async {
    try {
      final response = await _httpClient.get(_uri('/health'), headers: _headers).timeout(const Duration(seconds: 4));
      if (response.statusCode != 200) return const HealthStatus(ok: false);
      final body = jsonDecode(utf8.decode(response.bodyBytes));
      return HealthStatus(
        ok: body is Map<String, Object?> && body['status'] == 'ok',
        version: body is Map<String, Object?> && body['version'] is String ? body['version']! as String : null,
      );
    } catch (_) {
      return const HealthStatus(ok: false);
    }
  }

  Future<SetupStatus> getSetupStatus() async {
    final response = await _httpClient.get(_uri('/v1/setup/status'), headers: _headers);
    return SetupStatus.fromJson(_decode(response));
  }

  Future<List<SessionRecord>> listSessions() async {
    final response = await _httpClient.get(_uri('/v1/sessions'), headers: _headers);
    final body = jsonDecode(utf8.decode(response.bodyBytes));
    if (body is List) {
      return body.whereType<Map<String, Object?>>().map(SessionRecord.fromJson).toList(growable: false);
    }
    if (body is Map<String, Object?> && body['data'] is List) {
      return (body['data']! as List).whereType<Map<String, Object?>>().map(SessionRecord.fromJson).toList(growable: false);
    }
    throw const SlabRestException('unexpected sessions response shape', null);
  }

  /// Create a session; its `id` doubles as the harness WS `?token=`.
  Future<SessionRecord> createSession({String? name}) async {
    final response = await _httpClient.post(
      _uri('/v1/sessions'),
      headers: {..._headers, 'Content-Type': 'application/json'},
      body: jsonEncode({if (name != null && name.isNotEmpty) 'name': name}),
    );
    return SessionRecord.fromJson(_decode(response));
  }

  Future<SessionRecord> renameSession({required String id, required String name}) async {
    final response = await _httpClient.put(
      _uri('/v1/sessions/$id'),
      headers: {..._headers, 'Content-Type': 'application/json'},
      body: jsonEncode({'name': name}),
    );
    return SessionRecord.fromJson(_decode(response));
  }

  Future<void> deleteSession(String id) async {
    final response = await _httpClient.delete(_uri('/v1/sessions/$id'), headers: _headers);
    _decode(response);
  }

  void dispose() => _httpClient.close();
}
