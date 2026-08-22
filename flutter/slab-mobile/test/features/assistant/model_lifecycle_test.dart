/// Model lifecycle tests: the status-label priority matrix and the
/// repository pipeline (download → task poll → load, with the forced
/// re-download retry) over a fake dio adapter.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/core/network/slab_dio.dart';
import 'package:slab_mobile/data/model_types.dart';
import 'package:slab_mobile/data/rest_client.dart';
import 'package:slab_mobile/features/assistant/model/model_repository.dart';
import 'package:slab_mobile/features/assistant/model/model_status_label.dart';
import 'package:slab_mobile/l10n/catalog.dart';

Map<String, Object?> _model(String id, {
  String kind = 'local',
  String status = 'ready',
  String? localPath = '/models/x.gguf',
  int? contextWindow = 32768,
  int? runtimeContextLength,
  List<String> capabilities = const ['chat_generation'],
}) =>
    {
      'id': id,
      'display_name': 'Model $id',
      'kind': kind,
      'status': status,
      'capabilities': capabilities,
      'spec': {'local_path': ?localPath},
      'chat_capabilities': {'context_window': ?contextWindow},
      'runtime_state': {'context_length': ?runtimeContextLength},
    };

/// Scriptable adapter: per-path handlers; the download response carries a
/// task id and the task flips to succeeded after N polls.
class _FakeAdapter implements HttpClientAdapter {
  _FakeAdapter(this.models);

  List<Map<String, Object?>> models;
  int taskPolls = 0;
  int downloadCalls = 0;
  int loadCalls = 0;
  int pollsUntilSucceeded = 1;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    final path = options.uri.path;
    if (path == '/v1/models') {
      return _json(models);
    }
    if (path == '/v1/models/download') {
      downloadCalls += 1;
      return _json({'operation_id': 'task-1'});
    }
    if (path == '/v1/models/load') {
      loadCalls += 1;
      // Simulate a corrupt first load: throw once before succeeding.
      if (loadCalls == 1 && failFirstLoad) {
        failFirstLoad = false;
        throw StateError('weights corrupt');
      }
      loaded = true;
      return _json({});
    }
    if (path.startsWith('/v1/tasks/')) {
      taskPolls += 1;
      if (taskPolls <= pollsUntilSucceeded - 1) {
        return _json({'id': 'task-1', 'status': 'running'});
      }
      // Task done: the model gains a local path / runtime state afterwards.
      models = [
        for (final model in models)
          if (model['id'] == 'qwen')
            {
              ...model,
              'status': 'ready',
              'spec': {'local_path': '/models/qwen.gguf'},
              if (loaded || !failFirstLoad) 'runtime_state': {'context_length': 16384},
            }
          else
            model,
      ];
      return _json({'id': 'task-1', 'status': 'succeeded'});
    }
    return _json({});
  }

  bool failFirstLoad = false;
  bool loaded = false;

  ResponseBody _json(Object body) => ResponseBody.fromString(
        jsonEncode(body),
        200,
        headers: {'content-type': ['application/json']},
      );

  @override
  void close({bool force = false}) {}
}

SlabRestClient _client(_FakeAdapter adapter) {
  final dio = buildSlabDio(baseUrl: Uri.parse('http://127.0.0.1:9'))..httpClientAdapter = adapter;
  return SlabRestClient(baseUrl: Uri.parse('http://127.0.0.1:9'), dio: dio);
}

void main() {
  group('getSelectedModelStatusLabel', () {
    // The catalog's en-US table is enough for priority-matrix assertions.
    final catalog = SlabCatalog.fromJson('en-US', '{"pages.assistant.status.preparingSession":"Preparing session…","pages.assistant.status.loadingModels":"Loading models…","common.fields.selectModel":"Select a model","pages.assistant.status.runtimeContextWindow":"{{formatted}} ctx","pages.assistant.status.needsDownload":"Needs download","pages.assistant.connection.connected":"Connected"}');

    test('priority: session gates beat model state', () {
      String label({required bool sessionReady, bool loadingModels = false}) => getSelectedModelStatusLabel(
            sessionReady: sessionReady,
            isHistoryLoading: false,
            isCreatingSession: false,
            isDeletingSession: false,
            modelLoading: loadingModels,
            isPreparingModel: false,
            eventsConnected: false,
            selectedModel: null,
            catalog: catalog,
          );
      expect(label(sessionReady: false), 'Preparing session…');
      expect(label(sessionReady: true, loadingModels: true), 'Loading models…');
      expect(label(sessionReady: true), 'Select a model');
    });

    test('model parts compose: label + context window + connected', () {
      final model = AiModelRecord.fromJson(_model('qwen', runtimeContextLength: 32768));
      final label = getSelectedModelStatusLabel(
        sessionReady: true,
        isHistoryLoading: false,
        isCreatingSession: false,
        isDeletingSession: false,
        modelLoading: false,
        isPreparingModel: false,
        eventsConnected: true,
        selectedModel: model,
        catalog: catalog,
      );
      expect(label, 'Model qwen / 32,768 ctx / Connected');
    });

    test('undownloaded local model flags needs-download', () {
      final model = AiModelRecord.fromJson(_model('qwen', status: 'not_downloaded', localPath: null, contextWindow: null));
      final label = getSelectedModelStatusLabel(
        sessionReady: true,
        isHistoryLoading: false,
        isCreatingSession: false,
        isDeletingSession: false,
        modelLoading: false,
        isPreparingModel: false,
        eventsConnected: false,
        selectedModel: model,
        catalog: catalog,
      );
      expect(label, 'Model qwen / Needs download');
    });
  });

  group('ModelRepository', () {
    test('prepare downloads via task polling and loads', () async {
      final adapter = _FakeAdapter([
        _model('qwen', status: 'not_downloaded', localPath: null),
      ]);
      final repo = ModelRepository(
        client: _client(adapter),
        pollInterval: const Duration(milliseconds: 1),
        pollTimeout: const Duration(seconds: 1),
      );
      final prepared = await repo.prepare('qwen');
      expect(adapter.downloadCalls, 1);
      expect(adapter.loadCalls, 1);
      expect(adapter.taskPolls, greaterThanOrEqualTo(1));
      expect(prepared.downloaded, isTrue);
      expect(prepared.runtimeContextLength, 16384);
    });

    test('already-downloaded model skips the download round-trip', () async {
      final adapter = _FakeAdapter([
        _model('qwen', runtimeContextLength: 16384),
      ]);
      final repo = ModelRepository(
        client: _client(adapter),
        pollInterval: const Duration(milliseconds: 1),
      );
      final prepared = await repo.prepare('qwen');
      expect(adapter.downloadCalls, 0);
      expect(prepared.runtimeContextLength, 16384);
    });

    test('failed load forces one re-download then succeeds', () async {
      final adapter = _FakeAdapter([
        _model('qwen', status: 'not_downloaded', localPath: null),
      ])
        ..failFirstLoad = true
        ..pollsUntilSucceeded = 1;
      final repo = ModelRepository(
        client: _client(adapter),
        pollInterval: const Duration(milliseconds: 1),
      );
      final prepared = await repo.prepare('qwen');
      // First attempt (download + load fails) then forced re-download + load.
      expect(adapter.loadCalls, 2);
      expect(adapter.downloadCalls, 2);
      expect(prepared.downloaded, isTrue);
    });
  });
}
