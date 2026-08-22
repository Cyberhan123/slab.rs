/// Settings wire decode + API tests over a fake dio adapter.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/core/network/slab_dio.dart';
import 'package:slab_mobile/data/rest_client.dart';
import 'package:slab_mobile/data/settings_types.dart';

const _document = {
  'schema_version': 3,
  'settings_path': '/etc/slab/settings.toml',
  'warnings': ['deprecated key'],
  'sections': [
    {
      'id': 'server',
      'title': 'Server',
      'description_md': 'Bind address',
      'subsections': [
        {
          'id': 'general',
          'title': 'General',
          'properties': [
            {
              'pmid': 'server.address',
              'label': 'Bind address',
              'description_md': 'Interface to bind',
              'editable': true,
              'schema': {
                'type': 'string',
                'default_value': '127.0.0.1',
                'order': 1,
              },
              'effective_value': '0.0.0.0',
              'override_value': '0.0.0.0',
              'is_overridden': true,
              'change_effect': 'needs_restart',
              'overridden_by': {
                'type': 'env',
                'var_name': 'SLAB_SERVER_ADDRESS',
                'var_value_present': true,
              },
              'search_terms': ['bind', 'listen'],
            },
            {
              'pmid': 'general.language',
              'label': 'Language',
              'schema': {
                'type': 'string',
                'enum': ['en-US', 'zh-CN'],
              },
              'effective_value': 'en-US',
              'is_overridden': false,
              'change_effect': 'live',
              'search_terms': [],
            },
            {
              'pmid': 'providers.registry',
              'label': 'Providers',
              'schema': {'type': 'array', 'json_schema': {'type': 'array'}},
              'effective_value': <Object?>[],
              'is_overridden': false,
              'change_effect': 'needs_restart',
              'search_terms': [],
            },
          ],
        },
      ],
    },
  ],
};

class _FakeAdapter implements HttpClientAdapter {
  _FakeAdapter(this.responder);

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

SlabRestClient _client(_FakeAdapter adapter) {
  final dio = buildSlabDio(baseUrl: Uri.parse('http://127.0.0.1:9'))..httpClientAdapter = adapter;
  return SlabRestClient(baseUrl: Uri.parse('http://127.0.0.1:9'), dio: dio);
}

void main() {
  test('document decode: sections, schema enums, override provenance', () async {
    final adapter = _FakeAdapter((_) async => _json(_document));
    final client = _client(adapter);
    final doc = await client.getSettingsDocument();
    expect(doc.schemaVersion, 3);
    expect(doc.warnings, ['deprecated key']);
    final property = doc.sections.single.subsections.single.properties[0];
    expect(property.pmid, 'server.address');
    expect(property.changeEffect, SettingChangeEffect.needsRestart);
    expect(property.isOverridden, isTrue);
    final env = property.overriddenBy;
    expect(env, isA<SettingEnvOverride>());
    expect((env as SettingEnvOverride).varName, 'SLAB_SERVER_ADDRESS');
    expect(property.schema.valueType, SettingValueType.string);
    expect(property.schema.defaultValue, '127.0.0.1');

    final language = doc.sections.single.subsections.single.properties[1];
    expect(language.schema.enumValues, ['en-US', 'zh-CN']);
    expect(language.overriddenBy, isNull);

    final registry = doc.sections.single.subsections.single.properties[2];
    expect(registry.schema.valueType, SettingValueType.array);
    expect(registry.schema.jsonSchema, isNotNull);
  });

  test('updateSetting sends op set/unset bodies and decodes the response', () async {
    Object? sent;
    final adapter = _FakeAdapter((options) async {
      sent = options.data;
      // Return a minimal valid property body.
      return _json({
        'pmid': 'server.address',
        'label': 'Bind address',
        'schema': {'type': 'string'},
        'effective_value': '127.0.0.1',
      });
    });
    final client = _client(adapter);
    await client.updateSetting(pmid: 'server.address', set: true, value: '127.0.0.1');
    expect(sent, {'op': 'set', 'value': '127.0.0.1'});
    expect(adapter.requests.single.uri.path, '/v1/settings/server.address');

    await client.updateSetting(pmid: 'server.address', set: false);
    expect(sent, {'op': 'unset'});
  });

  test('400 validation errors surface as SettingValidationException with the JSON-pointer path', () async {
    final adapter = _FakeAdapter((_) async => _json({
          'type': 'validation',
          'pmid': 'providers.registry',
          'message': 'invalid api base',
          'path': '/0/auth/api_base',
        }, status: 400));
    final client = _client(adapter);
    await expectLater(
      client.updateSetting(pmid: 'providers.registry', set: true, value: const []),
      throwsA(
        isA<SettingValidationException>()
            .having((e) => e.data.path, 'path', '/0/auth/api_base')
            .having((e) => e.data.pmid, 'pmid', 'providers.registry'),
      ),
    );
  });

  test('updateSettingBody shape', () {
    expect(updateSettingBody(set: true, value: 5), {'op': 'set', 'value': 5});
    expect(updateSettingBody(set: false), {'op': 'unset'});
  });
}
