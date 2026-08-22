/// Harness protocol DTOs (the phase-1 mobile subset).
///
/// Hand-written codecs over the generated wire contract owned by
/// `packages/slab-core/src/harness/generated/index.ts` (ts-rs bindings from
/// `crates/slab-proto` + `crates/slab-agent`). Wire fields are camelCase;
/// optional fields are omitted on the wire (never `null`). The two historical
/// snake_case exceptions are preserved: `UserMessageContent.image` fields
/// (`image_url` / `mime_type`) and the `Plan` payload.
///
/// Method/notification strings live in `harness_methods.dart` (drift-guarded
/// by `bun run gen:harness`).
library;

// ── Envelope-adjacent small types ───────────────────────────────────────────

/// Persistence scope chosen by the user when approving a prompt.
enum ApprovalScope {
  runOnce('run_once'),
  alwaysInWorkspace('always_in_workspace'),
  always('always'),
  deny('deny');

  const ApprovalScope(this.wire);
  final String wire;

  static ApprovalScope? fromWire(String? value) =>
      ApprovalScope.values.where((s) => s.wire == value).firstOrNull;
}

/// `approval/resolve` request params.
Map<String, Object?> approvalResolveParams({
  required String threadId,
  required String itemId,
  required bool approved,
  ApprovalScope? scope,
}) => {
      'threadId': threadId,
      'itemId': itemId,
      'approved': approved,
      if (scope != null) 'scope': scope.wire,
    };

/// `approval/resolve` result: `delivered == false` means the pending entry was
/// gone server-side and the decision was not actioned.
class ApprovalResolveResult {
  ApprovalResolveResult({this.delivered, this.status});
  final bool? delivered;
  final String? status;

  static ApprovalResolveResult fromJson(Map<String, Object?> json) => ApprovalResolveResult(
        delivered: json['delivered'] is bool ? json['delivered']! as bool : null,
        status: json['status'] is String ? json['status']! as String : null,
      );
}

// ── Core conversation types ─────────────────────────────────────────────────

class Thread {
  Thread({required this.id, required this.preview, required this.modelProvider, required this.createdAt, required this.turns, this.path, this.cwd});
  final String id;
  final String preview;
  final String modelProvider;
  /// Unix epoch milliseconds.
  final int createdAt;
  final String? path;
  final String? cwd;
  final List<Turn> turns;

  static Thread fromJson(Map<String, Object?> json) => Thread(
        id: json['id']! as String,
        preview: json['preview'] is String ? json['preview']! as String : '',
        modelProvider: json['modelProvider'] is String ? json['modelProvider']! as String : '',
        createdAt: json['createdAt'] is int ? json['createdAt']! as int : 0,
        path: json['path'] is String ? json['path']! as String : null,
        cwd: json['cwd'] is String ? json['cwd']! as String : null,
        turns: (json['turns'] is List ? json['turns']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(Turn.fromJson)
            .toList(growable: false),
      );
}

class Turn {
  Turn({required this.id, required this.items, required this.status, this.error});
  final String id;
  final List<TurnItem> items;
  /// Open string set on the wire: `completed` / `interrupted` / `failed` / `inProgress`.
  final String status;
  final TurnError? error;

  static Turn fromJson(Map<String, Object?> json) => Turn(
        id: json['id']! as String,
        items: (json['items'] is List ? json['items']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(TurnItem.fromJson)
            .toList(growable: false),
        status: json['status'] is String ? json['status']! as String : '',
        error: json['error'] is Map<String, Object?> ? TurnError.fromJson(json['error']! as Map<String, Object?>) : null,
      );

  /// Numeric turn index, or `null` when the id does not parse (matches TS `Number(turn.id)` + NaN check).
  int? get numericId => int.tryParse(id);
}

class TurnError {
  TurnError({this.code, this.message});
  final String? code;
  final String? message;

  static TurnError fromJson(Map<String, Object?> json) => TurnError(
        code: json['code'] is String ? json['code']! as String : null,
        message: json['message'] is String ? json['message']! as String : null,
      );
}

// ── TurnItem sealed hierarchy ───────────────────────────────────────────────

/// One item within a turn, discriminated by `type` (camelCase and PascalCase
/// spellings are both accepted on decode, per the Rust serde contract).
sealed class TurnItem {
  const TurnItem({required this.id});

  /// Stable item id (unique within the thread; drives part merging).
  final String id;

  static TurnItem fromJson(Map<String, Object?> json) {
    final rawType = json['type'];
    if (rawType is! String) return UnknownItem(id: json['id'] is String ? json['id']! as String : '');
    // camelCase ↔ PascalCase tolerance: compare against the lower-cased first letter.
    final type = rawType.substring(0, 1).toLowerCase() + rawType.substring(1);
    switch (type) {
      case 'userMessage':
        return UserMessageItem(
          id: _id(json),
          content: (json['content'] is List ? json['content']! as List : const [])
              .whereType<Map<String, Object?>>()
              .map(UserMessageContent.fromJson)
              .toList(growable: false),
        );
      case 'agentMessage':
        return AgentMessageItem(id: _id(json), text: _string(json, 'text'));
      case 'reasoning':
        return ReasoningItem(
          id: _id(json),
          summary: _reasoningText(json['summary']),
          content: _reasoningText(json['content']),
        );
      case 'commandExecution':
        return CommandExecutionItem(
          id: _id(json),
          command: _string(json, 'command'),
          cwd: _string(json, 'cwd'),
          processId: _optString(json, 'processId'),
          status: _string(json, 'status'),
          aggregatedOutput: _optString(json, 'aggregatedOutput'),
          exitCode: json['exitCode'] is int ? json['exitCode']! as int : null,
          durationMs: json['durationMs'] is int ? json['durationMs']! as int : null,
        );
      case 'fileChange':
        return FileChangeItem(
          id: _id(json),
          changes: (json['changes'] is List ? json['changes']! as List : const []).toList(growable: false),
          status: _string(json, 'status'),
        );
      case 'mcpToolCall':
        return McpToolCallItem(
          id: _id(json),
          server: _string(json, 'server'),
          tool: _string(json, 'tool'),
          arguments: json['arguments'],
          status: _string(json, 'status'),
          result: json['result'],
          error: json['error'],
          durationMs: json['durationMs'] is int ? json['durationMs']! as int : null,
        );
      case 'webSearch':
        return WebSearchItem(id: _id(json), query: _string(json, 'query'));
      case 'imageView':
        return ImageViewItem(id: _id(json), path: _string(json, 'path'));
      case 'plan':
        return PlanTurnItem(id: _id(json), plan: json['plan']);
      default:
        return UnknownItem(id: _id(json));
    }
  }
}

String _id(Map<String, Object?> json) => json['id'] is String ? json['id']! as String : '';
String _string(Map<String, Object?> json, String key) => json[key] is String ? json[key]! as String : '';
String? _optString(Map<String, Object?> json, String key) => json[key] is String ? json[key]! as String : null;
String _reasoningText(Object? value) {
  // ReasoningText is a single string or an array of strings on the wire.
  if (value is String) return value;
  if (value is List) return value.whereType<String>().join('\n');
  return '';
}

final class UserMessageItem extends TurnItem {
  const UserMessageItem({required super.id, required this.content});
  final List<UserMessageContent> content;
}

final class AgentMessageItem extends TurnItem {
  const AgentMessageItem({required super.id, required this.text});
  final String text;
}

final class ReasoningItem extends TurnItem {
  const ReasoningItem({required super.id, required this.summary, required this.content});
  final String summary;
  /// The full reasoning trace — history renders `content` (keeps parity with the
  /// live reasoning-delta stream, which accumulates content).
  final String content;
}

final class CommandExecutionItem extends TurnItem {
  const CommandExecutionItem({
    required super.id,
    required this.command,
    required this.cwd,
    required this.status,
    this.processId,
    this.aggregatedOutput,
    this.exitCode,
    this.durationMs,
  });
  final String command;
  final String cwd;
  final String? processId;
  final String status;
  final String? aggregatedOutput;
  final int? exitCode;
  final int? durationMs;
}

final class FileChangeItem extends TurnItem {
  const FileChangeItem({required super.id, required this.changes, required this.status});
  final List<Object?> changes;
  final String status;
}

final class McpToolCallItem extends TurnItem {
  const McpToolCallItem({
    required super.id,
    required this.server,
    required this.tool,
    required this.arguments,
    required this.status,
    this.result,
    this.error,
    this.durationMs,
  });
  final String server;
  final String tool;
  final Object? arguments;
  final String status;
  final Object? result;
  final Object? error;
  final int? durationMs;
}

final class WebSearchItem extends TurnItem {
  const WebSearchItem({required super.id, required this.query});
  final String query;
}

final class ImageViewItem extends TurnItem {
  const ImageViewItem({required super.id, required this.path});
  final String path;
}

final class PlanTurnItem extends TurnItem {
  const PlanTurnItem({required super.id, required this.plan});
  final Object? plan;
}

/// Forward-compat bucket for future wire variants.
final class UnknownItem extends TurnItem {
  const UnknownItem({required super.id});
}

// ── User message content / user input ───────────────────────────────────────

sealed class UserMessageContent {
  const UserMessageContent();

  static UserMessageContent fromJson(Map<String, Object?> json) {
    if (json['type'] == 'image') {
      return ImageContent(
        // Wire fields are snake_case here (Rust enum variant without renames).
        imageUrl: json['image_url'] is String ? json['image_url']! as String : null,
        base64: json['base64'] is String ? json['base64']! as String : null,
        mimeType: json['mime_type'] is String ? json['mime_type']! as String : null,
      );
    }
    return TextContent(text: json['text'] is String ? json['text']! as String : '');
  }
}

final class TextContent extends UserMessageContent {
  const TextContent({required this.text});
  final String text;
}

final class ImageContent extends UserMessageContent {
  const ImageContent({this.imageUrl, this.base64, this.mimeType});
  final String? imageUrl;
  final String? base64;
  final String? mimeType;
}

/// Builds the harness `UserInput` text variant for `turn/start`.
Map<String, Object?> textUserInput(String text) => {
      'type': 'text',
      'text': text,
      'textElements': const <Object?>[],
    };

/// Builds the `UserInput` image variant (data: or http(s) URL; mobile picks
/// gallery images and sends data URLs — the server never sees a native path).
Map<String, Object?> imageUserInput(String imageUrl) => {
      'type': 'image',
      'imageUrl': imageUrl,
      'detail': 'auto',
    };

/// `turn/start` request params. `effort`/`permissionMode`/`agentType` ride
/// the composer state (agentType `"plan"` marks a plan-mode turn).
Map<String, Object?> turnStartParams({
  required String threadId,
  required List<Map<String, Object?>> input,
  required String model,
  String? effort,
  String? permissionMode,
  String? agentType,
}) => {
      'threadId': threadId,
      'input': input,
      'model': model,
      'effort': ?effort,
      'permissionMode': ?permissionMode,
      'agentType': ?agentType,
    };

/// `thread/start` request params (optional model selection).
Map<String, Object?> threadStartParams({String? model}) => {
      'model': ?model,
    };

/// `thread/resume` request params.
Map<String, Object?> threadResumeParams({String? threadId}) => {
      'threadId': ?threadId,
    };

/// `turn/interrupt` request params.
Map<String, Object?> turnInterruptParams({required String threadId, required String turnId}) => {
      'threadId': threadId,
      'turnId': turnId,
    };

/// `thread/fork` request params (`sandboxOverride` stays a wire string — the
/// mobile client never sets it).
Map<String, Object?> threadForkParams({required String threadId, String? modelOverride}) => {
      'threadId': threadId,
      'modelOverride': ?modelOverride,
    };

/// `thread/rollback` request params — `toTurnId` is the turn to roll back TO
/// (the controller sends `n - 1` for a user bubble at turn `n`).
Map<String, Object?> threadRollbackParams({required String threadId, required String toTurnId}) => {
      'threadId': threadId,
      'toTurnId': toTurnId,
    };

/// `thread/compact/start` request params.
Map<String, Object?> threadCompactStartParams({required String threadId, String? modelOverride}) => {
      'threadId': threadId,
      'modelOverride': ?modelOverride,
    };

/// `thread/compact/start` result.
class ThreadCompactStartResult {
  ThreadCompactStartResult({required this.thread, required this.removedMessages, required this.outputTokens});
  final Thread thread;
  final int removedMessages;
  final int outputTokens;

  static ThreadCompactStartResult fromJson(Map<String, Object?> json) => ThreadCompactStartResult(
        thread: Thread.fromJson(json['thread'] is Map<String, Object?> ? json['thread']! as Map<String, Object?> : const {}),
        removedMessages: json['removedMessages'] is int ? json['removedMessages']! as int : 0,
        outputTokens: json['outputTokens'] is int ? json['outputTokens']! as int : 0,
      );
}

// ── Model catalog (model/list) ──────────────────────────────────────────────

/// One selectable reasoning effort with its user-facing description.
class ReasoningEffortOption {
  ReasoningEffortOption({required this.reasoningEffort, required this.description});
  final String reasoningEffort;
  final String description;

  static ReasoningEffortOption fromJson(Map<String, Object?> json) => ReasoningEffortOption(
        reasoningEffort: json['reasoningEffort'] is String ? json['reasoningEffort']! as String : '',
        description: json['description'] is String ? json['description']! as String : '',
      );
}

/// A model entry from `model/list` (local or cloud).
class ModelInfo {
  ModelInfo({
    required this.id,
    required this.model,
    required this.displayName,
    required this.description,
    required this.supportedReasoningEfforts,
    required this.defaultReasoningEffort,
    required this.isDefault,
  });
  final String id;
  final String model;
  final String displayName;
  final String description;
  final List<ReasoningEffortOption> supportedReasoningEfforts;
  final String defaultReasoningEffort;
  final bool isDefault;

  static ModelInfo fromJson(Map<String, Object?> json) => ModelInfo(
        id: json['id'] is String ? json['id']! as String : '',
        model: json['model'] is String ? json['model']! as String : '',
        displayName: json['displayName'] is String ? json['displayName']! as String : '',
        description: json['description'] is String ? json['description']! as String : '',
        supportedReasoningEfforts: (json['supportedReasoningEfforts'] is List ? json['supportedReasoningEfforts']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(ReasoningEffortOption.fromJson)
            .toList(growable: false),
        defaultReasoningEffort: json['defaultReasoningEffort'] is String ? json['defaultReasoningEffort']! as String : '',
        isDefault: json['isDefault'] is bool ? json['isDefault']! as bool : false,
      );
}

/// `model/list` result (paged; the mobile picker loads the first page).
class ModelListResult {
  ModelListResult({required this.data, this.nextCursor});
  final List<ModelInfo> data;
  final String? nextCursor;

  static ModelListResult fromJson(Map<String, Object?> json) => ModelListResult(
        data: (json['data'] is List ? json['data']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(ModelInfo.fromJson)
            .toList(growable: false),
        nextCursor: json['nextCursor'] is String ? json['nextCursor']! as String : null,
      );
}

// ── Command registry (command/list) ─────────────────────────────────────────

/// How a `/`-command dispatches on the client (host callback vs prompt text).
enum CommandKind {
  control('control'),
  prompt('prompt');

  const CommandKind(this.wire);
  final String wire;

  static CommandKind fromWire(String? value) =>
      CommandKind.values.where((k) => k.wire == value).firstOrNull ?? CommandKind.prompt;
}

/// Where a command was registered.
enum CommandSource {
  builtin('builtin'),
  skill('skill');

  const CommandSource(this.wire);
  final String wire;

  static CommandSource fromWire(String? value) =>
      CommandSource.values.where((s) => s.wire == value).firstOrNull ?? CommandSource.builtin;
}

/// A user-facing `/`-command surfaced by `command/list`. `controlAction` is
/// the host-callback key for `control`-kind commands (e.g. "compact").
class CommandInfo {
  CommandInfo({
    required this.name,
    required this.aliases,
    required this.description,
    required this.kind,
    required this.source,
    this.controlAction,
  });
  final String name;
  final List<String> aliases;
  final String description;
  final CommandKind kind;
  final CommandSource source;
  final String? controlAction;

  static CommandInfo fromJson(Map<String, Object?> json) => CommandInfo(
        name: json['name'] is String ? json['name']! as String : '',
        aliases: (json['aliases'] is List ? json['aliases']! as List : const []).whereType<String>().toList(growable: false),
        description: json['description'] is String ? json['description']! as String : '',
        kind: CommandKind.fromWire(json['kind'] is String ? json['kind']! as String : null),
        source: CommandSource.fromWire(json['source'] is String ? json['source']! as String : null),
        controlAction: json['controlAction'] is String ? json['controlAction']! as String : null,
      );
}

// ── Notification params ─────────────────────────────────────────────────────

/// Params shared by most notifications (ids scoped to the emitting thread/turn).
class ThreadScopedParams {
  ThreadScopedParams({required this.threadId, this.turnId});
  final String threadId;
  final String? turnId;
}

class AgentMessageDeltaParams extends ThreadScopedParams {
  AgentMessageDeltaParams({required super.threadId, super.turnId, required this.itemId, required this.delta});
  final String itemId;
  final String delta;
}

class ReasoningTextDeltaParams extends ThreadScopedParams {
  ReasoningTextDeltaParams({required super.threadId, super.turnId, required this.itemId, required this.delta});
  final String itemId;
  final String delta;
}

class OutputDeltaParams extends ThreadScopedParams {
  OutputDeltaParams({required super.threadId, super.turnId, required this.itemId, required this.delta});
  final String itemId;
  final String delta;
}

class ItemLifecycleParams extends ThreadScopedParams {
  ItemLifecycleParams({required super.threadId, super.turnId, required this.item});
  final TurnItem item;
}

class TurnLifecycleParams extends ThreadScopedParams {
  TurnLifecycleParams({required super.threadId, required this.turn});
  final Turn turn;
}

/// Token usage reported at turn completion.
class TurnUsage {
  TurnUsage({required this.promptTokens, required this.completionTokens, required this.totalTokens, this.cachedTokens, required this.estimated});
  final int promptTokens;
  final int completionTokens;
  final int totalTokens;
  final int? cachedTokens;
  final bool estimated;

  static TurnUsage? fromJson(Object? json) {
    if (json is! Map<String, Object?>) return null;
    return TurnUsage(
      promptTokens: json['promptTokens'] is int ? json['promptTokens']! as int : 0,
      completionTokens: json['completionTokens'] is int ? json['completionTokens']! as int : 0,
      totalTokens: json['totalTokens'] is int ? json['totalTokens']! as int : 0,
      cachedTokens: json['cachedTokens'] is int ? json['cachedTokens']! as int : null,
      estimated: json['estimated'] is bool ? json['estimated']! as bool : false,
    );
  }
}

class TurnCompletedParams extends TurnLifecycleParams {
  TurnCompletedParams({required super.threadId, required super.turn, this.usage});
  final TurnUsage? usage;
}

class ErrorParams {
  ErrorParams({required this.code, required this.message, this.threadId, this.turnId, this.itemId});
  final String code;
  final String message;
  final String? threadId;
  final String? turnId;
  final String? itemId;
}

/// One file change inside a `fileChange` approval request.
class FileChangeApprovalChange {
  FileChangeApprovalChange({required this.path, required this.type, this.diff});
  final String path;
  final String type;
  final String? diff;
}

/// A pending human-approval request (command / fileChange / plan snapshot).
class ApprovalRequest {
  ApprovalRequest({
    required this.itemId,
    required this.threadId,
    required this.kind,
    this.command,
    this.cwd,
    this.changes = const [],
    this.reason,
    this.category,
    this.allowedScopes = const [],
    this.planSnapshot,
    this.status = ApprovalStatus.pending,
  });

  final String itemId;
  final String threadId;
  final ApprovalKind kind;
  final String? command;
  final String? cwd;
  final List<FileChangeApprovalChange> changes;
  final String? reason;
  final String? category;
  final List<ApprovalScope> allowedScopes;
  final Object? planSnapshot;
  ApprovalStatus status;
}

enum ApprovalKind { command, fileChange, plan }
enum ApprovalStatus { pending, approved, denied }

class CommandApprovalParams extends ThreadScopedParams {
  CommandApprovalParams({
    required super.threadId,
    super.turnId,
    required this.itemId,
    required this.command,
    required this.cwd,
    this.reason,
    this.category,
    required this.allowedScopes,
    this.planSnapshot,
  });
  final String itemId;
  final String command;
  final String cwd;
  final String? reason;
  final String? category;
  final List<ApprovalScope> allowedScopes;
  final Object? planSnapshot;
}

class FileApprovalParams extends ThreadScopedParams {
  FileApprovalParams({
    required super.threadId,
    super.turnId,
    required this.itemId,
    required this.changes,
    required this.allowedScopes,
  });
  final String itemId;
  final List<FileChangeApprovalChange> changes;
  final List<ApprovalScope> allowedScopes;
}

class ModelLoadDeltaParams {
  ModelLoadDeltaParams({required this.threadId, required this.phase, this.modelId, this.downloadedBytes, this.totalBytes});
  final String threadId;
  final String phase;
  final String? modelId;
  final int? downloadedBytes;
  final int? totalBytes;
}

class CompactedParams {
  CompactedParams({required this.threadId, this.status});
  final String threadId;
  final String? status;
}
