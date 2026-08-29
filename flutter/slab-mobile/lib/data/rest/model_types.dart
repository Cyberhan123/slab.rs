/// REST `/v1/models` wire decode (UnifiedModelResponse subset the assistant
/// needs). Hand-written tolerant codecs, matching the `rest_client.dart` style.
library;

/// One selectable model (local or cloud).
class AiModelRecord {
  const AiModelRecord({
    required this.id,
    required this.displayName,
    required this.kind,
    required this.status,
    required this.capabilities,
    this.localPath,
    this.contextWindow,
    this.runtimeContextLength,
    this.sizeBytes,
  });

  final String id;
  final String displayName;

  /// `local` | `cloud` (wire `kind`).
  final String kind;

  /// `ready` | `not_downloaded` | `downloading` | `error` (normalized).
  final String status;
  final List<String> capabilities;
  final String? localPath;

  /// Static context window from the model spec (when known).
  final int? contextWindow;

  /// Runtime context length of the loaded instance (from `runtime_state`).
  final int? runtimeContextLength;
  final int? sizeBytes;

  bool get downloaded => kind == 'cloud' || (localPath != null && localPath!.isNotEmpty);
  bool get pending => status == 'downloading';
  bool get supportsChatGeneration => capabilities.contains('chat_generation');

  static AiModelRecord fromJson(Map<String, Object?> json) {
    final spec = json['spec'] is Map<String, Object?> ? json['spec']! as Map<String, Object?> : const <String, Object?>{};
    final chat = json['chat_capabilities'] is Map<String, Object?>
        ? json['chat_capabilities']! as Map<String, Object?>
        : const <String, Object?>{};
    final runtime = json['runtime_state'] is Map<String, Object?>
        ? json['runtime_state']! as Map<String, Object?>
        : const <String, Object?>{};
    final rawStatus = json['status'] is String ? json['status']! as String : '';
    final status = switch (rawStatus) {
      'ready' || 'not_downloaded' || 'downloading' || 'error' => rawStatus,
      _ => 'error',
    };
    return AiModelRecord(
      id: json['id'] is String ? json['id']! as String : '',
      displayName: json['display_name'] is String ? json['display_name']! as String : '',
      kind: json['kind'] is String ? json['kind']! as String : 'local',
      status: status,
      capabilities: (json['capabilities'] is List ? json['capabilities']! as List : const []).whereType<String>().toList(growable: false),
      localPath: spec['local_path'] is String && (spec['local_path']! as String).isNotEmpty ? spec['local_path']! as String : null,
      contextWindow: chat['context_window'] is int ? chat['context_window']! as int : null,
      runtimeContextLength: runtime['context_length'] is int ? runtime['context_length']! as int : null,
      sizeBytes: json['size_bytes'] is int ? json['size_bytes']! as int : null,
    );
  }
}

/// GET/POST task shapes (`/v1/tasks/{id}`).
class TaskRecord {
  const TaskRecord({required this.id, required this.status, this.errorMsg});
  final String id;
  final String status;
  final String? errorMsg;

  bool get succeeded => status == 'succeeded';
  bool get failed => status == 'failed' || status == 'cancelled' || status == 'error';

  static TaskRecord fromJson(Map<String, Object?> json) => TaskRecord(
        id: json['id'] is String ? json['id']! as String : '',
        status: json['status'] is String ? json['status']! as String : '',
        errorMsg: json['error_msg'] is String ? json['error_msg']! as String : null,
      );
}
