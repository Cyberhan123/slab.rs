/// Thin REST client for the slab-server `/v1` surface the mobile app needs
/// (health probe, setup gate, session management). The harness WS lives in
/// `lib/proto/harness_client.dart`; this file deliberately stays HTTP-only.
///
/// Transport is dio (auth + error-envelope interceptors live in
/// `lib/core/network/`); `SlabRestException` is re-exported so existing import
/// sites are untouched.
library;

import 'package:dio/dio.dart';

import '../core/network/auth_interceptor.dart';
import '../core/network/slab_api_error.dart';
import '../core/network/slab_dio.dart';
import 'model_types.dart';
import 'settings_types.dart';

export '../core/network/slab_api_error.dart';

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

class SlabRestClient {
  /// [dio] is injectable for tests; when omitted a default client is built
  /// (and closed again by [dispose]).
  SlabRestClient({required Uri baseUrl, String? bearerToken, Dio? dio})
      : _baseUrl = baseUrl,
        _dio = dio ?? buildSlabDio(baseUrl: baseUrl),
        _ownsDio = dio == null {
    if (bearerToken != null && bearerToken.isNotEmpty) {
      _dio.interceptors.insert(0, SlabAuthInterceptor(tokenProvider: () => bearerToken));
    }
  }

  final Uri _baseUrl;
  final Dio _dio;
  final bool _ownsDio;

  Uri _uri(String path) => _baseUrl.replace(path: path, query: '', fragment: '');

  /// dio already decoded the JSON body; this guards status + envelope shape
  /// for the handful of endpoints that must return an object.
  Map<String, Object?> _decode(Response<Object?> response) {
    final body = response.data;
    final status = response.statusCode;
    if (status == null || status < 200 || status >= 300) {
      throw SlabRestException(slabApiErrorMessage(body), status);
    }
    if (body is! Map<String, Object?>) {
      throw const SlabRestException('unexpected response shape', null);
    }
    return body;
  }

  /// Unwraps dio failures: the error interceptor has already mapped them to
  /// [SlabRestException]; anything unmapped degrades to a transport error.
  Future<T> _run<T>(Future<T> Function() action) async {
    try {
      return await action();
    } on DioException catch (e) {
      final inner = e.error;
      if (inner is SlabRestException) throw inner;
      throw SlabRestException(e.message ?? 'request failed', null);
    }
  }

  /// Heartbeat probe — `false` on any transport error (never throws).
  Future<HealthStatus> probeHealth() async {
    try {
      final response = await _dio
          .getUri<Object?>(_uri('/health'))
          .timeout(const Duration(seconds: 4));
      if (response.statusCode != 200) return const HealthStatus(ok: false);
      final body = response.data;
      return HealthStatus(
        ok: body is Map<String, Object?> && body['status'] == 'ok',
        version: body is Map<String, Object?> && body['version'] is String ? body['version']! as String : null,
      );
    } catch (_) {
      return const HealthStatus(ok: false);
    }
  }

  Future<SetupStatus> getSetupStatus() => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/setup/status'));
        return SetupStatus.fromJson(_decode(response));
      });

  Future<List<SessionRecord>> listSessions() => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/sessions'));
        final body = response.data;
        if (body is List) {
          return body.whereType<Map<String, Object?>>().map(SessionRecord.fromJson).toList(growable: false);
        }
        if (body is Map<String, Object?> && body['data'] is List) {
          return (body['data']! as List).whereType<Map<String, Object?>>().map(SessionRecord.fromJson).toList(growable: false);
        }
        throw const SlabRestException('unexpected sessions response shape', null);
      });

  /// Create a session; its `id` doubles as the harness WS `?token=`.
  Future<SessionRecord> createSession({String? name}) => _run(() async {
        final response = await _dio.postUri<Object?>(
          _uri('/v1/sessions'),
          data: {if (name != null && name.isNotEmpty) 'name': name},
        );
        return SessionRecord.fromJson(_decode(response));
      });

  Future<SessionRecord> renameSession({required String id, required String name}) => _run(() async {
        final response = await _dio.putUri<Object?>(
          _uri('/v1/sessions/$id'),
          data: {'name': name},
        );
        return SessionRecord.fromJson(_decode(response));
      });

  Future<void> deleteSession(String id) => _run(() async {
        final response = await _dio.deleteUri<Object?>(_uri('/v1/sessions/$id'));
        _decode(response);
      });

  // ── Models & tasks (assistant model lifecycle) ─────────────────────────────

  /// `GET /v1/models` — accepts both a bare array and a `{data: []}` envelope.
  Future<List<AiModelRecord>> listModels() => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/models'));
        final body = response.data;
        final List<Object?> raw = body is List
            ? body
            : body is Map<String, Object?> && body['data'] is List
                ? body['data']! as List
                : const [];
        return raw.whereType<Map<String, Object?>>().map(AiModelRecord.fromJson).toList(growable: false);
      });

  /// `POST /v1/models/download` → the task envelope (`operation_id`/`task_id`).
  Future<Map<String, Object?>> downloadModel(String modelId) => _run(() async {
        final response = await _dio.postUri<Object?>(_uri('/v1/models/download'), data: {'model_id': modelId});
        final body = response.data;
        return body is Map<String, Object?> ? body : const <String, Object?>{};
      });

  Future<void> loadModel(String modelId) => _run(() async {
        await _dio.postUri<Object?>(_uri('/v1/models/load'), data: {'model_id': modelId});
      });

  Future<void> switchModel(String modelId) => _run(() async {
        await _dio.postUri<Object?>(_uri('/v1/models/switch'), data: {'model_id': modelId});
      });

  Future<void> unloadModel(String modelId) => _run(() async {
        await _dio.postUri<Object?>(_uri('/v1/models/unload'), data: {'model_id': modelId});
      });

  /// `GET /v1/settings` — the schema-driven settings document.
  Future<SettingsDocumentView> getSettingsDocument() => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/settings'));
        final body = response.data;
        if (body is! Map<String, Object?>) {
          throw const SlabRestException('unexpected settings document shape', null);
        }
        return SettingsDocumentView.fromJson(body);
      });

  /// `GET /v1/settings/{pmid}` — one property.
  Future<SettingPropertyView> getSetting(String pmid) => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/settings/${Uri.encodeComponent(pmid)}'));
        final body = _decode(response);
        return SettingPropertyView.fromJson(body);
      });

  /// `PUT /v1/settings/{pmid}` — set (with value) or unset (reset to default).
  /// A 400 with a validation payload throws [SettingValidationException].
  Future<SettingPropertyView> updateSetting({required String pmid, required bool set, Object? value}) => _run(() async {
        try {
          final response = await _dio.putUri<Object?>(
            _uri('/v1/settings/${Uri.encodeComponent(pmid)}'),
            data: updateSettingBody(set: set, value: value),
          );
          return SettingPropertyView.fromJson(_decode(response));
        } on DioException catch (error) {
          final inner = error.error;
          if (inner is SlabRestException && inner.statusCode == 400) {
            // Re-decode the raw body for the structured validation payload.
            final validation = SettingValidationErrorData.fromJson(
                error.response?.data is Map<String, Object?> ? error.response!.data as Map<String, Object?> : null);
            if (validation != null) {
              throw SettingValidationException(validation, 400);
            }
          }
          rethrow;
        }
      });

  /// `GET /v1/tasks/{id}` — polled until a terminal status.
  Future<TaskRecord> getTask(String id) => _run(() async {
        final response = await _dio.getUri<Object?>(_uri('/v1/tasks/$id'));
        final body = _decode(response);
        return TaskRecord.fromJson(body);
      });

  /// Only closes a client this constructor built; injected clients belong to
  /// their owner (tests reuse them across cases).
  void dispose() {
    if (_ownsDio) _dio.close();
  }
}
