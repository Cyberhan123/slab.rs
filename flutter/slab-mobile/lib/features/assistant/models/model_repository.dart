/// Model lifecycle pipeline over the REST surface: ensure-downloaded (task
/// polling) → ensure-loaded, with the desktop's one retry path (force
/// re-download on first load failure). Port of the `use-ai-model.tsx`
/// imperative core, framework-free for testing.
library;

import 'dart:async';

import 'package:slab_mobile/data/rest/model_types.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';

/// Task poll cadence mirrors the desktop task page constants.
const modelDownloadPollInterval = Duration(seconds: 2);
const modelDownloadTimeout = Duration(minutes: 30);

class ModelPrepareException implements Exception {
  const ModelPrepareException(this.message);
  final String message;
  @override
  String toString() => message;
}

/// `operation_id` | `task_id` (the desktop `extractTaskId` contract).
String? extractTaskId(Object? payload) {
  if (payload is! Map<String, Object?>) return null;
  for (final key in const ['operation_id', 'task_id']) {
    final value = payload[key];
    if (value is String && value.trim().isNotEmpty) return value.trim();
  }
  return null;
}

class ModelRepository {
  ModelRepository({
    required SlabRestClient client,
    this.pollInterval = modelDownloadPollInterval,
    this.pollTimeout = modelDownloadTimeout,
  }) : _client = client;

  final SlabRestClient _client;
  final Duration pollInterval;
  final Duration pollTimeout;

  /// Models usable as the assistant model (chat-generation capable).
  Future<List<AiModelRecord>> chatModels() async {
    final models = await _client.listModels();
    return models.where((model) => model.supportsChatGeneration).toList(growable: false);
  }

  /// Re-fetch and return one model (null when it disappeared server-side).
  Future<AiModelRecord?> refreshModel(String modelId) async {
    final models = await chatModels();
    return models.where((model) => model.id == modelId).firstOrNull;
  }

  Future<void> _waitForTask(String taskId) async {
    final deadline = DateTime.now().add(pollTimeout);
    while (DateTime.now().isBefore(deadline)) {
      final task = await _client.getTask(taskId);
      if (task.succeeded) return;
      if (task.failed) {
        throw ModelPrepareException(task.errorMsg ?? 'Task $taskId ended with status: ${task.status}');
      }
      await Future<void>.delayed(pollInterval);
    }
    throw ModelPrepareException('Timed out waiting for model task $taskId.');
  }

  /// Download a local model unless already on disk (or [forceDownload]).
  /// Returns the refreshed record (with `local_path` set).
  Future<AiModelRecord> ensureDownloaded(String modelId, {bool forceDownload = false}) async {
    var model = await refreshModel(modelId);
    if (model == null) throw ModelPrepareException("Selected model '$modelId' is not available.");
    if (model.kind == 'cloud') return model;
    if (model.downloaded && !forceDownload) return model;

    final taskId = extractTaskId(await _client.downloadModel(modelId));
    if (taskId == null) {
      throw ModelPrepareException("Failed to start model download task for '$modelId'.");
    }
    await _waitForTask(taskId);

    model = await refreshModel(modelId);
    if (model == null || !model.downloaded) {
      throw ModelPrepareException("Model '$modelId' download completed, but local_path is empty.");
    }
    return model;
  }

  /// Load a local model into the runtime (cloud models need no load).
  Future<AiModelRecord> ensureLoaded(String modelId) async {
    final model = await refreshModel(modelId);
    if (model == null) throw ModelPrepareException("Selected model '$modelId' is not available.");
    if (model.kind == 'cloud') return model;
    await _client.loadModel(modelId);
    return await refreshModel(modelId) ?? model;
  }

  /// Full prepare with the desktop retry semantics: on the first load
  /// failure, force a re-download once and try again.
  Future<AiModelRecord> prepare(String modelId) async {
    await ensureDownloaded(modelId);
    try {
      return await ensureLoaded(modelId);
    } on Object catch (loadError) {
      if (loadError is ModelPrepareException && loadError.message.contains('not available')) rethrow;
      // One retry: the on-disk weights may be corrupt — re-download, then load.
      await ensureDownloaded(modelId, forceDownload: true);
      try {
        return await ensureLoaded(modelId);
      } catch (_) {
        throw ModelPrepareException('Model load failed after re-download: $loadError');
      }
    }
  }
}
