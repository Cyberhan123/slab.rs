/// REST client tests over a fake dio adapter — exercises the real interceptor
/// stack (auth stamping, error-envelope mapping), not a stubbed client.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/core/network/slab_dio.dart';
import 'package:slab_mobile/data/rest_client.dart';

class FakeAdapter implements HttpClientAdapter {
  FakeAdapter(this.responder);

  final FutureOr<ResponseBody> Function(RequestOptions options) responder;
  final List<RequestOptions> requests = [];

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    requests.add(options);
    return responder(options);
  }

  @override
  void close({bool force = false}) {}
}

ResponseBody _json(Object body, {int status = 200}) => ResponseBody.fromString(
      jsonEncode(body),
      status,
      headers: {'content-type': ['application/json']},
    );

const _sessionBody = {
  'id': 's1',
  'name': 'Session One',
  'created_at': '2026-01-01T00:00:00Z',
  'updated_at': '2026-01-02T00:00:00Z',
};

SlabRestClient _client(FakeAdapter adapter, {String? bearerToken}) {
  // Real interceptor stack; only the socket is fake.
  final dio = buildSlabDio(baseUrl: Uri.parse('http://127.0.0.1:9'))..httpClientAdapter = adapter;
  return SlabRestClient(baseUrl: Uri.parse('http://127.0.0.1:9'), bearerToken: bearerToken, dio: dio);
}

void main() {
  test('probeHealth decodes status and version', () async {
    final adapter = FakeAdapter((_) async => _json({'status': 'ok', 'version': '1.2.3'}));
    final client = _client(adapter);
    final health = await client.probeHealth();
    expect(health.ok, isTrue);
    expect(health.version, '1.2.3');
  });

  test('probeHealth returns ok:false on a non-200 status', () async {
    final adapter = FakeAdapter((_) async => _json({'status': 'degraded'}, status: 503));
    final client = _client(adapter);
    final health = await client.probeHealth();
    expect(health.ok, isFalse);
  });

  test('probeHealth returns ok:false on a transport failure', () async {
    final adapter = FakeAdapter((_) async => throw StateError('socket gone'));
    final client = _client(adapter);
    final health = await client.probeHealth();
    expect(health.ok, isFalse);
  });

  test('getSetupStatus decodes the initialized flags', () async {
    final adapter = FakeAdapter(
      (options) async => _json({'initialized': true, 'runtime_payload_installed': true}),
    );
    final client = _client(adapter);
    final status = await client.getSetupStatus();
    expect(status.initialized, isTrue);
    expect(status.runtimePayloadInstalled, isTrue);
    expect(adapter.requests.single.uri.path, '/v1/setup/status');
  });

  test('bearer token is stamped on every request', () async {
    final adapter = FakeAdapter((_) async => _json([_sessionBody]));
    final client = _client(adapter, bearerToken: 'secret');
    await client.listSessions();
    expect(adapter.requests.single.headers['Authorization'], 'Bearer secret');
  });

  test('non-2xx maps to SlabRestException with server message and status', () async {
    final adapter = FakeAdapter((_) async => _json({'message': 'boom'}, status: 400));
    final client = _client(adapter);
    await expectLater(
      client.getSetupStatus(),
      throwsA(
        isA<SlabRestException>()
            .having((e) => e.message, 'message', 'boom')
            .having((e) => e.statusCode, 'statusCode', 400),
      ),
    );
  });

  test('listSessions accepts a bare array body', () async {
    final adapter = FakeAdapter((_) async => _json([_sessionBody]));
    final client = _client(adapter);
    final sessions = await client.listSessions();
    expect(sessions, hasLength(1));
    expect(sessions.single.id, 's1');
    expect(sessions.single.createdAt, '2026-01-01T00:00:00Z');
  });

  test('listSessions accepts a {data: []} envelope', () async {
    final adapter = FakeAdapter((_) async => _json({
          'data': [_sessionBody],
        }));
    final client = _client(adapter);
    final sessions = await client.listSessions();
    expect(sessions, hasLength(1));
    expect(sessions.single.name, 'Session One');
  });

  test('listSessions rejects unexpected shapes', () async {
    final adapter = FakeAdapter((_) async => _json({'unexpected': true}));
    final client = _client(adapter);
    await expectLater(
      client.listSessions(),
      throwsA(isA<SlabRestException>().having((e) => e.statusCode, 'statusCode', isNull)),
    );
  });

  test('createSession posts the name and decodes the record', () async {
    Object? sentBody;
    final adapter = FakeAdapter((options) async {
      sentBody = options.data;
      return _json(_sessionBody, status: 201);
    });
    final client = _client(adapter);
    final session = await client.createSession(name: 'New chat');
    expect(session.id, 's1');
    expect(adapter.requests.single.method, 'POST');
    expect(sentBody, {'name': 'New chat'});
  });

  test('renameSession issues PUT with the new name', () async {
    Object? sentBody;
    final adapter = FakeAdapter((options) async {
      sentBody = options.data;
      return _json({..._sessionBody, 'name': 'Renamed'});
    });
    final client = _client(adapter);
    final session = await client.renameSession(id: 's1', name: 'Renamed');
    expect(session.name, 'Renamed');
    expect(adapter.requests.single.method, 'PUT');
    expect(adapter.requests.single.uri.path, '/v1/sessions/s1');
    expect(sentBody, {'name': 'Renamed'});
  });

  test('deleteSession issues DELETE', () async {
    final adapter = FakeAdapter((_) async => _json({}, status: 204));
    final client = _client(adapter);
    await client.deleteSession('s1');
    expect(adapter.requests.single.method, 'DELETE');
    expect(adapter.requests.single.uri.path, '/v1/sessions/s1');
  });
}
