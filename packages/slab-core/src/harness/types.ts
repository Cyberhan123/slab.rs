/**
 * Harness protocol types — hand-written TypeScript mirror of the authoritative
 * Rust contract in `crates/slab-proto/src/harness/` (ts-rs codegen was removed;
 * the compiled Rust types are the source of truth). All wire fields are
 * camelCase; optional fields are omitted on the wire (not `null`).
 *
 * The harness control plane is a WebSocket JSON-RPC 2.0 connection at
 * `/v1/agents/harness?token=<sessionId>`. The client sends requests, the server
 * responds, and the server pushes agent events as notifications.
 */

// ── JSON-RPC 2.0 envelope ───────────────────────────────────────────────────

export type RequestId = string | number

export interface JsonRpcRequest<P = unknown> {
  jsonrpc: "2.0"
  id: RequestId
  method: string
  params?: P
}

export interface JsonRpcResponse {
  jsonrpc: "2.0"
  id: RequestId
  result: unknown
}

export interface JsonRpcErrorResponse {
  jsonrpc: "2.0"
  id: RequestId
  error: JsonRpcErrorBody
}

export interface JsonRpcErrorBody {
  code: number
  message: string
  data?: unknown
}

export interface JsonRpcNotification<P = unknown> {
  jsonrpc: "2.0"
  method: string
  params?: P
}

export type JsonRpcMessage = JsonRpcRequest | JsonRpcResponse | JsonRpcErrorResponse | JsonRpcNotification

// ── Method + error-code constants (mirror `slab_proto::harness::method`) ────

/** Client → server requests. */
export const HARNESS_METHOD = {
  INITIALIZE: "initialize",
  THREAD_START: "thread/start",
  THREAD_RESUME: "thread/resume",
  THREAD_FORK: "thread/fork",
  THREAD_ROLLBACK: "thread/rollback",
  THREAD_COMPACT_START: "thread/compact/start",
  THREAD_ARCHIVE: "thread/archive",
  THREAD_LIST: "thread/list",
  TURN_START: "turn/start",
  TURN_INTERRUPT: "turn/interrupt",
  MODEL_LIST: "model/list",
  SKILLS_LIST: "skills/list",
  COMMAND_LIST: "command/list",
  APPROVAL_RESOLVE: "approval/resolve",
  SHUTDOWN: "shutdown",
  WORKSPACE_MIGRATE: "workspace/migrate",
} as const

/** Server → client notifications (agent event delivery). */
export const HARNESS_NOTIFICATION = {
  THREAD_STARTED: "thread/started",
  TURN_STARTED: "turn/started",
  TURN_COMPLETED: "turn/completed",
  ITEM_STARTED: "item/started",
  ITEM_COMPLETED: "item/completed",
  ITEM_AGENT_MESSAGE_DELTA: "item/agentMessage/delta",
  ITEM_REASONING_TEXT_DELTA: "item/reasoning/textDelta",
  ITEM_REASONING_SUMMARY_TEXT_DELTA: "item/reasoning/summaryTextDelta",
  ITEM_COMMAND_EXECUTION_OUTPUT_DELTA: "item/commandExecution/outputDelta",
  ITEM_FILE_CHANGE_OUTPUT_DELTA: "item/fileChange/outputDelta",
  ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL: "item/commandExecution/requestApproval",
  ITEM_FILE_CHANGE_REQUEST_APPROVAL: "item/fileChange/requestApproval",
  ERROR: "error",
  ACCOUNT_UPDATED: "account/updated",
  ACCOUNT_LOGIN_COMPLETED: "account/loginCompleted",
  // Model load lifecycle, emitted directly from the turn/start handler.
  MODEL_LOAD_DELTA: "model/load/delta",
  MODEL_LOAD_COMPLETED: "model/load/completed",
  // Context-compaction lifecycle, emitted from the agent turn loop via EventMsg.
  CONTEXT_COMPACTING: "context/compacting",
  CONTEXT_COMPACTED: "context/compacted",
} as const

/** Harness-specific error codes (reserved; the dispatcher currently emits `-32000`). */
export const HARNESS_ERROR_CODE = {
  APPLICATION_ERROR: -32000,
  NOT_INITIALIZED: -32001,
  THREAD_NOT_FOUND: -32002,
  TURN_IN_PROGRESS: -32003,
  NOT_IMPLEMENTED: -32004,
} as const

// ── Policy enums ────────────────────────────────────────────────────────────

export type ReasoningEffort = "off" | "low" | "medium" | "high" | "xhigh"

export type ApprovalPolicy = "never" | "on-request" | "on-failure" | "untrusted"

/** Per-session permission mode. `approve_for_me` = acceptEdits: auto-allow what the baseline permits, prompt the rest. */
export type PermissionMode =
  | "request_approval"
  | "approve_for_me"
  | "full_control"
  | "custom"

// ── Plan value type (mirrors `slab_agent::Plan`, snake_case wire fields) ────

export type PlanStatus = "pending" | "in_progress" | "completed" | "blocked"

export interface PlanItem {
  step: string
  status: PlanStatus
  depends_on?: string[]
  result_ref?: string
}

export interface PlanCounts {
  pending: number
  in_progress: number
  completed: number
  blocked: number
}

/** A structured plan authored in Plan mode. Fields are snake_case on the wire. */
export interface Plan {
  plan_id: string
  summary?: string
  items: PlanItem[]
  counts: PlanCounts
  current_step?: number
}

/** Persistence scope chosen when approving a prompt. */
export type ApprovalScope = "run_once" | "always_in_workspace" | "always" | "deny"

/** Operation category for an approval prompt. */
export type OperationCategory = "shell" | "file_edit" | "network" | "read_only"

export type SandboxMode = "read-only" | "workspace-write" | "danger-full-access"

export type NetworkAccess = "restricted" | "enabled"

export type SandboxPolicy =
  | { type: "dangerFullAccess" }
  | { type: "readOnly" }
  | { type: "externalSandbox"; networkAccess?: NetworkAccess }
  | {
      type: "workspaceWrite"
      writableRoots?: string[]
      networkAccess?: boolean
      excludeTmpdirEnvVar?: boolean
      excludeSlashTmp?: boolean
    }

// ── Thread / Turn / TurnItem ────────────────────────────────────────────────

export interface GitInfo {
  branch: string
  sha: string
  isDirty: boolean
}

export interface Thread {
  id: string
  preview: string
  modelProvider: string
  /** Unix epoch milliseconds. */
  createdAt: number
  path?: string
  cwd?: string
  cliVersion?: string
  source?: string
  gitInfo?: GitInfo
  turns: Turn[]
}

/** Open string set: `completed` | `interrupted` | `failed` | `inProgress`. */
export type TurnStatus = string

export interface TurnError {
  code?: string
  message?: string
  additionalDetails?: unknown
}

export interface Turn {
  id: string
  items: TurnItem[]
  status: TurnStatus
  error?: TurnError
}

/** Reasoning text — a single string or an array of strings. */
export type ReasoningText = string | string[]

export type UserMessageContent =
  | { type: "text"; text: string }
  | { type: "image"; imageUrl?: string; base64?: string; mimeType?: string }

/**
 * A discrete artifact within a turn, discriminated by `type`. Mirrors
 * `TurnItem` in `slab-proto/src/harness/item.rs` (camelCase canonical form).
 */
export type TurnItem =
  | { type: "userMessage"; id: string; content: UserMessageContent[] }
  | { type: "agentMessage"; id: string; text: string }
  | { type: "reasoning"; id: string; summary: ReasoningText; content: ReasoningText }
  | {
      type: "commandExecution"
      id: string
      command: string
      cwd: string
      processId?: string
      status: string
      aggregatedOutput?: string
      exitCode?: number
      durationMs?: number
    }
  | { type: "fileChange"; id: string; changes: unknown[]; status: string }
  | {
      type: "mcpToolCall"
      id: string
      server: string
      tool: string
      arguments: unknown
      status: string
      result?: unknown
      error?: unknown
      durationMs?: number
    }
  | { type: "webSearch"; id: string; query: string }
  | { type: "imageView"; id: string; path: string }
  | { type: "plan"; id: string; plan: Plan }

// ── UserInput (turn/start `input`) ──────────────────────────────────────────

export interface ByteRange {
  start: number
  end: number
}

export interface TextElement {
  byteRange: ByteRange
  placeholder?: string
}

export type ImageDetail = "low" | "high" | "auto"

export type UserInput =
  | { type: "text"; text: string; textElements: TextElement[] }
  | { type: "image"; imageUrl: string; detail?: ImageDetail }
  | { type: "localImage"; path: string; detail?: ImageDetail }
  | { type: "skill"; name: string; path: string }
  | { type: "mention"; name: string; path: string }

// ── Request params / results ────────────────────────────────────────────────

export interface ClientInfo {
  name: string
  title?: string
  version: string
}

export interface InitializeParams {
  clientInfo: ClientInfo
}

export interface ServerInfo {
  name: string
  version: string
}

export interface ServerCapabilities {
  experimental?: unknown
}

export interface InitializeResult {
  serverInfo?: ServerInfo
  protocolVersion?: string
  capabilities?: ServerCapabilities
}

export interface ThreadStartParams {
  model?: string
  modelProvider?: string
  cwd?: string
  approvalPolicy?: ApprovalPolicy
  sandbox?: SandboxMode
  permissionMode?: PermissionMode
  /** Built-in agent type for this thread (e.g. "plan"). Unset = default agent. */
  agentType?: string
  baseInstructions?: string
  developerInstructions?: string
  experimentalRawEvents?: boolean
  config?: unknown
}

export interface ThreadStartResult {
  thread: Thread
  model: string
  modelProvider: string
  cwd: string
  approvalPolicy: ApprovalPolicy
  sandbox: SandboxPolicy
  reasoningEffort?: ReasoningEffort
}

export interface ThreadResumeParams {
  threadId?: string
  path?: string
}

export interface ThreadResumeResult {
  thread: Thread
}

export interface TurnStartParams {
  threadId: string
  input: UserInput[]
  cwd?: string
  approvalPolicy?: ApprovalPolicy
  sandboxPolicy?: SandboxPolicy
  permissionMode?: PermissionMode
  /** Built-in agent type to run this turn as (e.g. "plan"). Unset = default agent. */
  agentType?: string
  model?: string
  effort?: ReasoningEffort
  outputSchema?: unknown
}

export interface TurnStartResult {
  turn: Turn
}

export interface TurnInterruptParams {
  threadId: string
  turnId: string
}

export interface TurnInterruptResult {
  status?: string
}

export interface ApprovalResolveParams {
  threadId: string
  itemId: string
  approved: boolean
  /** Persistence scope. Omitted by older clients (server treats as `run_once`). */
  scope?: ApprovalScope
}

export interface ApprovalResolveResult {
  delivered?: boolean
  status?: string
}

export interface ShutdownParams {
  threadId: string
}

export interface ShutdownResult {
  status?: string
}

export interface ThreadForkParams {
  threadId: string
  modelOverride?: string
  sandboxOverride?: SandboxMode
}

export interface ThreadForkResult {
  thread: Thread
}

export interface ThreadRollbackParams {
  threadId: string
  toTurnId: string
}

export interface ThreadRollbackResult {
  thread: Thread
}

export interface ThreadCompactStartParams {
  threadId: string
  /** Optional override of the summarization model. */
  modelOverride?: string
}

export interface ThreadCompactStartResult {
  thread: Thread
  removedMessages: number
  outputTokens: number
}

export interface ThreadArchiveParams {
  threadId: string
}

export interface ThreadArchiveResult {
  thread: Thread
}

export interface ThreadListParams {
  cursor?: string
  limit?: number
  modelProviders?: string[]
}

export interface ThreadListResult {
  data: Thread[]
  nextCursor?: string
}

export interface ReasoningEffortOption {
  reasoningEffort: ReasoningEffort
  description: string
}

export interface ModelInfo {
  id: string
  model: string
  displayName: string
  description: string
  supportedReasoningEfforts: ReasoningEffortOption[]
  defaultReasoningEffort: ReasoningEffort
  isDefault: boolean
}

export interface ModelListParams {
  modelProviders?: string[]
}

export interface ModelListResult {
  data: ModelInfo[]
  nextCursor?: string
}

export type SkillSource = "workspace" | "global"

export interface SkillInfo {
  name: string
  description: string
  path: string
  source: SkillSource
}

export interface SkillsListParams {}

export interface SkillsListResult {
  data: SkillInfo[]
}

/** How a user-facing `/`-command dispatches on the client. */
export type CommandKind = "control" | "prompt"

/** Where a command was declared. */
export type CommandSource = "builtin" | "skill"

/** A user-facing `/`-command surfaced by `command/list`. */
export interface CommandInfo {
  /** Trigger name, without the leading `/`. */
  name: string
  aliases: string[]
  description: string
  kind: CommandKind
  source: CommandSource
  /** Control-kind action key mapped to a host callback (`"compact"` | `"fork"`). */
  controlAction?: string
}

export interface CommandListParams {}

export interface CommandListResult {
  data: CommandInfo[]
}

export interface WorkspaceMigrateParams {
  workspaceRoot?: string
}

export interface WorkspaceMigrateResult {
  projectId?: string
  suspendedCount: number
}

// ── Notification params (server → client) ──────────────────────────────────

export interface ThreadStartedParams {
  thread: Thread
}

export interface TurnStartedParams {
  threadId: string
  turn: Turn
}

export interface TurnCompletedParams {
  threadId: string
  turn: Turn
  /** Token usage for the turn, when the backend reported any. */
  usage?: TurnUsage
}

/** Token-usage snapshot reported at turn completion (camelCase, mirrors the
 * Rust `TurnUsage` in `slab-proto/src/harness/notification.rs`). */
export interface TurnUsage {
  promptTokens: number
  completionTokens: number
  totalTokens: number
  cachedTokens?: number
  estimated?: boolean
}

export interface ItemStartedParams {
  item: TurnItem
  threadId: string
  turnId: string
}

export interface ItemCompletedParams {
  item: TurnItem
  threadId: string
  turnId: string
}

export interface AgentMessageDeltaParams {
  threadId: string
  turnId: string
  itemId: string
  delta: string
}

export interface ReasoningTextDeltaParams {
  threadId: string
  turnId: string
  itemId: string
  contentIndex: number
  delta: string
}

export interface ReasoningSummaryTextDeltaParams {
  threadId: string
  turnId: string
  itemId: string
  summaryIndex?: number
  delta: string
}

export interface CommandExecutionOutputDeltaParams {
  threadId: string
  turnId: string
  itemId: string
  delta: string
}

export interface FileChangeOutputDeltaParams {
  threadId: string
  turnId: string
  itemId: string
  delta: string
}

export interface CommandExecutionRequestApprovalParams {
  threadId: string
  turnId: string
  itemId: string
  command: string
  cwd: string
  reason?: string
  category?: OperationCategory
  /** Scopes the client may offer; empty for servers that only support approve/reject. */
  allowedScopes?: ApprovalScope[]
  /** Full plan snapshot, present only on `present_plan` approvals (rich card). */
  planSnapshot?: Plan
}

export interface FileChangeApprovalChange {
  path: string
  type: string
  diff?: string
}

export interface FileChangeRequestApprovalParams {
  threadId: string
  turnId: string
  itemId: string
  changes: FileChangeApprovalChange[]
  allowedScopes?: ApprovalScope[]
}

export interface ErrorParams {
  threadId?: string
  turnId?: string
  itemId?: string
  code: string
  message: string
  data?: unknown
}

/**
 * `model/load/delta` — coarse progress for a model load driven by the
 * `turn/start` handler. Carries `threadId` (required: the client routes by it)
 * and deliberately NO numeric `turnId` (the transport's replay guard drops any
 * notification whose `turnId` parses `<= threshold`). Handled out-of-band by the
 * conversation controller, NOT turned into AI-SDK message parts.
 */
export type ModelLoadPhase = "downloading" | "loading"

export interface ModelLoadDeltaParams {
  threadId: string
  modelId?: string
  phase: ModelLoadPhase
  downloadedBytes?: number
  totalBytes?: number
  message?: string
}

export interface ModelLoadError {
  code: string
  message: string
}

/** `model/load/completed` — terminal load result (`status: "ready" | "error"`). */
export interface ModelLoadCompletedParams {
  threadId: string
  modelId: string
  backend?: string
  status: "ready" | "error"
  contextLength?: number
  trainingContextLength?: number
  error?: ModelLoadError
}

/**
 * `context/compacting` — an auto-compaction summarization has begun (after the
 * policy threshold gate passed). Handled out-of-band by the conversation controller to
 * show an in-progress "compacting context" indicator (NOT an AI-SDK message part).
 */
export interface ContextCompactingParams {
  threadId: string
}

/**
 * `context/compacted` — terminal compaction result. `status: "compacted"` flips
 * the in-progress indicator to done; `"skipped"` clears it (a started compaction
 * that did not shrink the set, so no dangling shimmer).
 */
export interface ContextCompactedParams {
  threadId: string
  /** `"compacted"` (default) or `"skipped"`. */
  status?: "compacted" | "skipped"
  removedMessages?: number
  outputTokens?: number
}

/**
 * Union of every server → client notification, discriminated by `method`.
 * Matches `ServerNotification` in `slab-proto/src/harness/notification.rs`.
 */
export type ServerNotification =
  | { method: typeof HARNESS_NOTIFICATION.THREAD_STARTED; params: ThreadStartedParams }
  | { method: typeof HARNESS_NOTIFICATION.TURN_STARTED; params: TurnStartedParams }
  | { method: typeof HARNESS_NOTIFICATION.TURN_COMPLETED; params: TurnCompletedParams }
  | { method: typeof HARNESS_NOTIFICATION.ITEM_STARTED; params: ItemStartedParams }
  | { method: typeof HARNESS_NOTIFICATION.ITEM_COMPLETED; params: ItemCompletedParams }
  | { method: typeof HARNESS_NOTIFICATION.ITEM_AGENT_MESSAGE_DELTA; params: AgentMessageDeltaParams }
  | { method: typeof HARNESS_NOTIFICATION.ITEM_REASONING_TEXT_DELTA; params: ReasoningTextDeltaParams }
  | {
      method: typeof HARNESS_NOTIFICATION.ITEM_REASONING_SUMMARY_TEXT_DELTA
      params: ReasoningSummaryTextDeltaParams
    }
  | {
      method: typeof HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_OUTPUT_DELTA
      params: CommandExecutionOutputDeltaParams
    }
  | {
      method: typeof HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_OUTPUT_DELTA
      params: FileChangeOutputDeltaParams
    }
  | {
      method: typeof HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
      params: CommandExecutionRequestApprovalParams
    }
  | {
      method: typeof HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_REQUEST_APPROVAL
      params: FileChangeRequestApprovalParams
    }
  | { method: typeof HARNESS_NOTIFICATION.ERROR; params: ErrorParams }
  | { method: typeof HARNESS_NOTIFICATION.ACCOUNT_UPDATED; params: unknown }
  | { method: typeof HARNESS_NOTIFICATION.ACCOUNT_LOGIN_COMPLETED; params: unknown }
  | { method: typeof HARNESS_NOTIFICATION.MODEL_LOAD_DELTA; params: ModelLoadDeltaParams }
  | { method: typeof HARNESS_NOTIFICATION.MODEL_LOAD_COMPLETED; params: ModelLoadCompletedParams }
  | { method: typeof HARNESS_NOTIFICATION.CONTEXT_COMPACTING; params: ContextCompactingParams }
  | { method: typeof HARNESS_NOTIFICATION.CONTEXT_COMPACTED; params: ContextCompactedParams }

/** A notification whose `method` we don't model explicitly. */
export interface UnknownNotification {
  method: string
  params?: unknown
}
