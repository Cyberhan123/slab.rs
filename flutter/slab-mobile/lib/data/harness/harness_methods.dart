/// Harness method / notification constants.
///
/// Values must match `crates/slab-proto` `method` consts and the TS
/// `HARNESS_METHOD` / `HARNESS_NOTIFICATION` (packages/slab-core/src/harness/
/// constants.ts); `bun run gen:harness` fails when the values drift.
library;

/// Client → server requests.
final class HarnessMethod {
  HarnessMethod._();

  static const String initialize = 'initialize';
  static const String threadStart = 'thread/start';
  static const String threadResume = 'thread/resume';
  static const String threadFork = 'thread/fork';
  static const String threadRollback = 'thread/rollback';
  static const String threadCompactStart = 'thread/compact/start';
  static const String threadArchive = 'thread/archive';
  static const String threadList = 'thread/list';
  static const String turnStart = 'turn/start';
  static const String turnInterrupt = 'turn/interrupt';
  static const String modelList = 'model/list';
  static const String skillsList = 'skills/list';
  static const String commandList = 'command/list';
  static const String approvalResolve = 'approval/resolve';
  static const String shutdown = 'shutdown';
  static const String workspaceMigrate = 'workspace/migrate';
}

/// Server → client notifications (agent event delivery).
final class HarnessNotification {
  HarnessNotification._();

  static const String threadStarted = 'thread/started';
  static const String turnStarted = 'turn/started';
  static const String turnCompleted = 'turn/completed';
  static const String itemStarted = 'item/started';
  static const String itemCompleted = 'item/completed';
  static const String itemAgentMessageDelta = 'item/agentMessage/delta';
  static const String itemReasoningTextDelta = 'item/reasoning/textDelta';
  static const String itemReasoningSummaryTextDelta = 'item/reasoning/summaryTextDelta';
  static const String itemCommandExecutionOutputDelta = 'item/commandExecution/outputDelta';
  static const String itemFileChangeOutputDelta = 'item/fileChange/outputDelta';
  static const String itemCommandExecutionRequestApproval =
      'item/commandExecution/requestApproval';
  static const String itemFileChangeRequestApproval = 'item/fileChange/requestApproval';
  static const String error = 'error';
  static const String accountUpdated = 'account/updated';
  static const String accountLoginCompleted = 'account/loginCompleted';
  static const String modelLoadDelta = 'model/load/delta';
  static const String modelLoadCompleted = 'model/load/completed';
  static const String contextCompacting = 'context/compacting';
  static const String contextCompacted = 'context/compacted';
}

/// Every notification method the mobile client routes explicitly.
const Set<String> kKnownNotifications = {
  HarnessNotification.threadStarted,
  HarnessNotification.turnStarted,
  HarnessNotification.turnCompleted,
  HarnessNotification.itemStarted,
  HarnessNotification.itemCompleted,
  HarnessNotification.itemAgentMessageDelta,
  HarnessNotification.itemReasoningTextDelta,
  HarnessNotification.itemReasoningSummaryTextDelta,
  HarnessNotification.itemCommandExecutionOutputDelta,
  HarnessNotification.itemFileChangeOutputDelta,
  HarnessNotification.itemCommandExecutionRequestApproval,
  HarnessNotification.itemFileChangeRequestApproval,
  HarnessNotification.error,
  HarnessNotification.accountUpdated,
  HarnessNotification.accountLoginCompleted,
  HarnessNotification.modelLoadDelta,
  HarnessNotification.modelLoadCompleted,
  HarnessNotification.contextCompacting,
  HarnessNotification.contextCompacted,
};
