/**
 * Framework-free conversation controller for the harness control plane.
 *
 * Owns the full assistant-page conversation state machine that previously
 * lived inside the React hook (`use-harness-conversation`): session restore
 * (open retry + `thread/resume` projection), the out-of-band notification
 * projections (approval queue, live command output, model-load indicator,
 * compaction markers, turn usage, command registry), and the user actions
 * (`approval/resolve`, `thread/compact/start`, `thread/fork`,
 * `thread/rollback`, plan mode).
 *
 * Observable as an external store: `getState()` returns an immutable snapshot
 * (reference-stable until the next change) and `subscribe(listener)` follows
 * the plain `Set<listener>` emitter pattern used by `ui-state/server-storage`.
 * React binds to it via `useSyncExternalStore` in a thin hook; sending (the
 * AI-SDK chunk stream) stays with `HarnessChatTransport` — `send()` below is
 * the programmatic equivalent, sharing the same turn-input builder.
 *
 * Lifecycle: one controller per slab session. A session change means
 * constructing a new controller (the React side keys it on `sessionId` via
 * `useMemo`); a fresh instance starts with pristine state, which is why the
 * old hook's manual session-switch reset does not exist here. `start()`
 * triggers the restore machine (idempotent — a newer run invalidates an older
 * one), `dispose()` cancels in-flight work and closes the owned client.
 */

import type { UIMessage } from "ai"

import { HarnessClient } from "./harness-client"
import { turnItemsToMessages } from "./turn-items"
import { buildTurnInput } from "./turn-input"
import { HARNESS_NOTIFICATION } from "./types"
import type {
  ApprovalScope,
  CommandExecutionOutputDeltaParams,
  CommandExecutionRequestApprovalParams,
  CommandInfo,
  ContextCompactedParams,
  ContextCompactingParams,
  FileChangeApprovalChange,
  FileChangeOutputDeltaParams,
  FileChangeRequestApprovalParams,
  JsonRpcNotification,
  ModelLoadDeltaParams,
  ModelLoadPhase,
  OperationCategory,
  PermissionMode,
  Plan,
  ReasoningEffort,
  Thread,
  ThreadStatusChangedParams,
  TurnCompletedParams,
  TurnStartParams,
  TurnStartResult,
  TurnUsage,
  UserInput,
} from "./types"

// ── Public types (moved from the ui hook verbatim) ──────────────────────────

/** A pending human-approval request surfaced from the harness (commands / file changes / plans). */
export type ApprovalRequest = {
  itemId: string
  threadId: string
  kind: "command" | "fileChange" | "plan"
  command?: string
  cwd?: string
  changes?: FileChangeApprovalChange[]
  reason?: string
  category?: OperationCategory
  /** Persistence scopes the server allows the user to pick. */
  allowedScopes?: ApprovalScope[]
  /** Full plan snapshot, present only on `present_plan` approvals (rich card). */
  planSnapshot?: Plan
  status: "pending" | "approved" | "denied"
}

export type ApprovalStatus = "pending" | "approved" | "denied"

/**
 * Transient model-load indicator state, driven by `model/load/delta` +
 * `model/load/completed` notifications emitted from `turn/start`. `null` when
 * no load is in progress (the indicator is hidden).
 */
export type ModelLoadState = {
  phase: ModelLoadPhase
  modelId?: string
  downloadedBytes?: number
  totalBytes?: number
} | null

/** Whether a compaction marker represents an automatic or manual (`/compact`) run. */
export type CompactionMode = "auto" | "manual"

/**
 * A resident background task started via `shell background=true`, tracked from
 * `backgroundTask/updated` notifications. Terminal states stay listed (the
 * registry bounds how many exist) so the timeline shows how tasks ended.
 */
export interface BackgroundTaskInfo {
    taskId: string
    status: "running" | "exited" | "stopped" | "failed"
    exitCode?: number | null
    pid?: number | null
    command?: string | null
}
/** `compacting` = in-progress (rendered as a Shimmer); `compacted` = done. */
export type CompactionPhase = "compacting" | "compacted"

/**
 * A session-scoped compaction divider rendered in the message stream. Survives
 * the pane remount after a manual compact (state lives in this controller,
 * above the keyed pane) but is cleared on a full reload — not persisted to the
 * backend.
 */
export interface CompactionMarker {
  /** Stable id (`${mode}:${threadId}:${nonce}`) so phase flips keep the same row. */
  id: string
  mode: CompactionMode
  phase: CompactionPhase
  threadId: string
}

/** A failed `/compact` or `/fork` action (distinct from a restore `error`). */
export type ActionError = { kind: "compact" | "fork"; message: string }

/**
 * Wire values of the authoritative thread status pushed by
 * `thread/statusChanged` — the SERVER-side source of truth the UI should
 * derive busy/terminal state from (instead of a local AI-SDK heuristic that
 * can diverge after interrupts).
 */
export type ThreadStatusString =
  | "pending"
  | "running"
  | "interrupting"
  | "interrupted"
  | "completed"
  | "errored"
  | "shutdown"

/** Terminal thread statuses — no further work until a new turn starts. */
const TERMINAL_THREAD_STATUSES: ReadonlySet<ThreadStatusString> = new Set([
  "interrupted",
  "completed",
  "errored",
  "shutdown",
])

/** Immutable snapshot exposed via {@link ConversationController.getState}. */
export interface ConversationState {
  /** Restored conversation, seeded as `useChat` `initialMessages`. */
  restoredMessages: UIMessage[]
  /** Harness thread id of the restored conversation (null for a fresh session). */
  restoredThreadId: string | null
  /** Session id once its conversation has been restored (for sidebar highlight). */
  activeConversation: string | undefined
  /** Bumped after each restore completes; used as a remount key for the chat pane. */
  restoreVersion: number
  /** True while connecting / restoring. */
  isHistoryLoading: boolean
  /** Restore error message, if any. */
  error: string | null
  /** A failed `/compact` or `/fork` action, surfaced separately from `error`. */
  actionError: ActionError | null
  /** Approval requests still awaiting a user decision (rendered in the banner). */
  approvals: ApprovalRequest[]
  /** itemId → approval status, for the in-stream tool-card status badge. */
  approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
  /** itemId → accumulated live command output (stdout/stderr deltas so far). */
  liveOutputByItemId: ReadonlyMap<string, string>
  /** itemId → live `apply_patch` progress lines (one JSON object per applied file). */
  livePatchByItemId: ReadonlyMap<string, string[]>
  /** Transient model-load indicator state (null when idle). */
  modelLoad: ModelLoadState
  /** Token usage for the most recent completed turn (null until the first turn completes). */
  turnUsage: TurnUsage | null
  /** `thread.createdAt` (Unix ms) of the restored thread; null for fresh sessions. */
  historyCreatedAt: number | null
  /** Command registry snapshot from `command/list` (drives the `/`-menu + dispatch). */
  commands: CommandInfo[]
  /** Session-scoped compaction markers rendered as in-stream dividers. */
  compactionMarkers: CompactionMarker[]
  /** True while a manual `/compact` round-trip is in flight. */
  isCompacting: boolean
  /** True while a `/fork` round-trip is in flight. */
  isForking: boolean
  /** True while a `thread/rollback` round-trip is in flight. */
  isRollingBack: boolean
  /** userMessage itemId → its numeric turn index (drives the rollback affordance). */
  userMessageTurnIndex: ReadonlyMap<string, number>
  /** Whether plan mode is active (turn runs as the read-only plan agent). */
  planMode: boolean
  /**
   * Authoritative thread status from `thread/statusChanged` (null before the
   * first event). The busy/submit gating should derive from THIS, not from a
   * local AI-SDK status heuristic.
   */
  threadStatus: ThreadStatusString | null
  /**
   * Why the last run ended abnormally (`interrupted` / `max_turns_reached` /
   * `budget_exhausted` / `repetition_detected` / `error`), from the terminal
   * `turn/completed` reason. Null after a clean completion.
   */
  abortReason: string | null
  /** Steering input queued on the server for the running turn (0 when none). */
  queuedCount: number
  /** Texts of the queued steering inputs, in send order (drives queued chips). */
  queuedTexts: readonly string[]
  /** Resident background tasks (shell background=true), latest last. */
  backgroundTasks: readonly BackgroundTaskInfo[]
}

/** Options accepted by the {@link ConversationController} constructor. */
export interface ConversationControllerOptions {
  /** Slab session id; a change means constructing a new controller. */
  sessionId?: string
  /** Inject a client (tests); by default one is constructed and owned. */
  client?: HarnessClient
  /** Model id sent on `turn/start` (defaults to "slab-llama"). */
  model?: string
  /** Passed through to {@link HarnessClient} when it is constructed here. */
  baseURL?: string
  /** Passed through to {@link HarnessClient} (tests pass a fake). */
  WebSocketCtor?: typeof WebSocket
}

/** Per-call options for {@link ConversationController.send}. */
export interface TurnSendOptions {
  /** Overrides the controller's default model for this turn. */
  model?: string
  effort?: ReasoningEffort
  permissionMode?: PermissionMode
  /** Built-in agent type (`"plan"` when plan mode is active). */
  agentType?: "plan"
}

/**
 * How many times the harness transport `open()` is retried before a restore
 * fails. `slab-server` spawns asynchronously, so the first WebSocket dial can
 * race the server being ready; a few backed-off retries recover without a
 * manual refresh. Only the transport open is retried — `thread/resume` failures
 * are surfaced immediately (they are either a real error or a "no thread" fresh
 * session).
 */
export const MAX_RESTORE_ATTEMPTS = 3
export const RESTORE_BACKOFF_MS = 400

// ── Pure helpers (moved from the ui hook verbatim) ──────────────────────────

/** Highest numeric turn id in a thread (-1 when there are no turns). */
function computeLastTurnIndex(thread: Thread): number {
  let max = -1
  for (const turn of thread.turns) {
    const index = Number(turn.id)
    if (!Number.isNaN(index) && index > max) max = index
  }
  return max
}

/**
 * Map each `userMessage` item id to the numeric turn index of the turn that owns
 * it. Drives the per-user-bubble rollback affordance: rolling back to that
 * message retracts it and every later turn (`thread/rollback` with
 * `toTurnId = turnIndex - 1` keeps turns `0..turnIndex-1`).
 */
export function buildUserMessageTurnIndex(thread: Thread): Map<string, number> {
  const map = new Map<string, number>()
  for (const turn of thread.turns) {
    const idx = Number(turn.id)
    if (Number.isNaN(idx)) continue
    for (const item of turn.items) {
      if (item.type === "userMessage") map.set(item.id, idx)
    }
  }
  return map
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/**
 * Cache for the derived approval-status projection, keyed by the immutable
 * `approvals` map instance: unrelated commits (thread status, live output, …)
 * keep the SAME projection identity so React consumers skip re-rendering.
 */
const approvalStatusCache = new WeakMap<
  Map<string, ApprovalRequest>,
  Map<string, ApprovalStatus>
>()

const EMPTY_SNAPSHOT: ConversationState = {
  restoredMessages: [],
  restoredThreadId: null,
  activeConversation: undefined,
  restoreVersion: 0,
  isHistoryLoading: false,
  error: null,
  actionError: null,
  approvals: [],
  approvalStatusByItemId: new Map(),
  liveOutputByItemId: new Map(),
  livePatchByItemId: new Map(),
  modelLoad: null,
  turnUsage: null,
  historyCreatedAt: null,
  commands: [],
  compactionMarkers: [],
  isCompacting: false,
  isForking: false,
  isRollingBack: false,
  userMessageTurnIndex: new Map(),
  planMode: false,
  threadStatus: null,
  abortReason: null,
  queuedCount: 0,
  queuedTexts: [],
  backgroundTasks: [],
}

// ── Controller ──────────────────────────────────────────────────────────────

export class ConversationController {
  /** The owned, long-lived harness client (also feeds `HarnessChatTransport`). */
  readonly client: HarnessClient

  private readonly sessionId: string | undefined
  private readonly model: string

  /** Snapshot cache — rebuilt only on state changes so the reference is stable. */
  private snapshot: ConversationState = EMPTY_SNAPSHOT
  private readonly listeners = new Set<() => void>()
  /** Bumped by every restore run / dispose; stale runs exit silently. */
  private generation = 0
  private unsubscribeNotifications: (() => void) | null = null

  // Mutable state (mirrors the former hook's useState set).
  private restoredMessages: UIMessage[] = []
  private restoredThreadId: string | null = null
  private activeConversation: string | undefined
  private restoreVersion = 0
  private isHistoryLoading = false
  private error: string | null = null
  private actionError: ActionError | null = null
  private approvals = new Map<string, ApprovalRequest>()
  private liveOutput = new Map<string, string>()
  private livePatch = new Map<string, string[]>()
  private modelLoad: ModelLoadState = null
  private turnUsage: TurnUsage | null = null
  private historyCreatedAt: number | null = null
  private commands: CommandInfo[] = []
  private compactionMarkers: CompactionMarker[] = []
  private isCompacting = false
  private isForking = false
  private isRollingBack = false
  private userMessageTurnIndex = new Map<string, number>()
  private planMode = false
  private threadStatus: ThreadStatusString | null = null
  private abortReason: string | null = null
  private queuedTexts: string[] = []
  private backgroundTasks: BackgroundTaskInfo[] = []
  /**
   * The live AI-SDK stream never replays user messages (the wire has no
   * `message/appended` notification), so when queued steering is drained or
   * persisted server-side the only way those inputs reappear is a history
   * refresh. Set when a refresh is owed but the triggering run has not ended
   * yet; the terminal event consumes it.
   */
  private pendingResync = false

  constructor(options: ConversationControllerOptions = {}) {
    this.sessionId = options.sessionId
    this.model = options.model ?? "slab-llama"
    this.client =
      options.client ??
      new HarnessClient({
        sessionId: options.sessionId ?? "",
        baseURL: options.baseURL,
        WebSocketCtor: options.WebSocketCtor,
      })
    this.unsubscribeNotifications = this.client.onNotification(this.handleNotification)
  }

  /** Current immutable snapshot (reference-stable until the next change). */
  readonly getState = (): ConversationState => this.snapshot

  /** External-store subscription. Bound per instance; reference-stable. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  /** Kick off the restore machine (mount hook for the React side). */
  start(): void {
    // `dispose()` tears the notification subscription down; a `start()` after
    // that (route-transition remount reusing a memoized controller) MUST
    // re-register it or the controller silently ignores every notification —
    // approvals never render, thread status never updates — while the
    // transport (sharing the same client) keeps streaming normally.
    if (!this.unsubscribeNotifications) {
      this.unsubscribeNotifications = this.client.onNotification(this.handleNotification)
    }
    void this.reconnect()
  }

  /**
   * (Re)run the restore machine: `open()` with backed-off retries →
   * `command/list` (fire-and-forget) → `thread/resume` projection. Also the
   * reconnection entry point after an unexpected socket close. A newer run (or
   * `dispose()`) invalidates an in-flight one.
   */
  async reconnect(): Promise<void> {
    const generation = ++this.generation
    const isCurrent = () => this.generation === generation
    // Structural fingerprint of the restored history before this run — the
    // remount key (restoreVersion) only bumps when the history actually
    // changed, so a no-op re-read (e.g. a resync whose rollout append already
    // matched) keeps the virtual list and skips entrance-animation replays.
    const prevThreadId = this.restoredThreadId
    const prevMessageIds = this.restoredMessages.map((message) => message.id)

    // No session bound: reset the conversation projection (fresh session) and
    // bump the remount key, mirroring the former hook's no-session branch.
    if (!this.sessionId) {
      this.client.currentThreadId = null
      this.client.lastTurnIndex = -1
      this.restoredMessages = []
      this.restoredThreadId = null
      this.activeConversation = undefined
      this.userMessageTurnIndex = new Map()
      this.error = null
      this.isHistoryLoading = false
      this.backgroundTasks = []
      this.restoreVersion += 1
      this.commit()
      return
    }

    this.isHistoryLoading = true
    this.error = null
    this.commit()

    try {
      // slab-server spawns asynchronously, so the harness WebSocket can fail
      // to dial on the first attempt. Retry the transport open with backoff
      // rather than failing the whole restore. Resume errors are NOT retried —
      // they are either a real failure or a "no thread" fresh session.
      for (let attempt = 1; attempt <= MAX_RESTORE_ATTEMPTS; attempt += 1) {
        if (!isCurrent()) return
        try {
          await this.client.open()
          break
        } catch (openError) {
          if (!isCurrent()) return
          if (attempt === MAX_RESTORE_ATTEMPTS) throw openError
          await sleep(RESTORE_BACKOFF_MS * attempt)
        }
      }

      // Fetch the command registry snapshot (drives the `/`-menu + dispatch).
      // Fire-and-forget: commands must not gate the restore path, and a
      // failure just leaves the menu on its last (possibly empty) snapshot.
      void this.client
        .commandList()
        .then((res) => {
          if (!isCurrent()) return
          this.commands = res.data
          this.commit()
        })
        .catch(() => {})

      try {
        const { thread } = await this.client.threadResume({})
        const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
        this.client.currentThreadId = thread.id
        this.client.lastTurnIndex = computeLastTurnIndex(thread)
        if (!isCurrent()) return
        // An identical re-read (same thread, same message-id sequence) keeps
        // the PRIOR message objects: downstream memoized rows (keyed on
        // message identity) skip re-rendering and the remount version stays.
        const identicalReread =
          this.restoredThreadId === thread.id &&
          messages.length === prevMessageIds.length &&
          messages.every((message, index) => message.id === prevMessageIds[index])
        if (!identicalReread) this.restoredMessages = messages
        // Background tasks are thread-scoped; a thread switch drops the list.
        if (this.restoredThreadId !== thread.id) this.backgroundTasks = []
        this.userMessageTurnIndex = buildUserMessageTurnIndex(thread)
        this.restoredThreadId = thread.id
        this.historyCreatedAt = thread.createdAt
        this.activeConversation = this.sessionId
        this.commit()
      } catch (resumeError) {
        // A fresh session has no thread to resume — start empty and let the
        // first turn lazily create the thread.
        const message = resumeError instanceof Error ? resumeError.message : String(resumeError)
        if (!/no thread to resume/i.test(message)) throw resumeError
        this.client.currentThreadId = null
        this.client.lastTurnIndex = -1
        if (!isCurrent()) return
        this.restoredMessages = []
        this.restoredThreadId = null
        this.activeConversation = this.sessionId
        this.userMessageTurnIndex = new Map()
        this.commit()
      }
    } catch (restoreError) {
      if (!isCurrent()) return
      this.error =
        restoreError instanceof Error ? restoreError.message : "failed to restore conversation"
      this.commit()
    } finally {
      if (isCurrent()) {
        this.isHistoryLoading = false
        // Remount only on a structural change (thread swap or a different
        // message-id sequence); an identical re-read keeps the version.
        const structurallyChanged =
          this.restoredThreadId !== prevThreadId ||
          this.restoredMessages.length !== prevMessageIds.length ||
          this.restoredMessages.some((message, index) => message.id !== prevMessageIds[index])
        if (structurallyChanged) this.restoreVersion += 1
        this.commit()
      }
    }
  }

  /**
   * Programmatically start a turn for the given user message: ensures the
   * socket is open, lazily binds a thread (`thread/start` on a fresh session),
   * and fires `turn/start` with the shared turn-input mapping. The interactive
   * send path stays with `useChat` + `HarnessChatTransport` (chunk streaming);
   * this is the non-AI-SDK equivalent.
   *
   * Arrow-bound (detachable): safe to destructure and call as a bare
   * reference — `sendSteering` and the React hook rely on that.
   */
  readonly send = async (
    message: UIMessage,
    options?: TurnSendOptions,
  ): Promise<TurnStartResult> => {
    const model = options?.model ?? this.model
    await this.client.open()

    // Bind a thread if none is bound yet (fresh session, no prior resume).
    if (this.client.currentThreadId === null) {
      const started = await this.client.threadStart({ model })
      this.client.currentThreadId = started.thread.id
    }
    const threadId = this.client.currentThreadId
    if (!threadId) throw new Error("no harness thread bound")

    const input = buildTurnInput([message])
    const params: TurnStartParams = {
      threadId,
      // Always send at least a text part so the turn has well-formed input.
      input:
        input.length > 0
          ? input
          : ([{ text: "", textElements: [], type: "text" }] satisfies UserInput[]),
      model,
    }
    if (options?.effort) params.effort = options.effort
    if (options?.permissionMode) params.permissionMode = options.permissionMode
    if (options?.agentType) params.agentType = options.agentType
    return this.client.turnStart(params)
  }

  /**
   * Steering send: deliver user input to a RUNNING turn. The server queues it
   * for the next iteration boundary (`turn/start` returns `queued: true`), the
   * still-open AI-SDK stream keeps rendering the run's continued output, and
   * the text is tracked locally as a queued chip until the run ends. If the
   * run ended in the race window (server answered `queued: false`), refresh
   * the restored history so the pane converges on the new run's rollout.
   */
  readonly sendSteering = async (
    message: UIMessage,
    options?: TurnSendOptions,
  ): Promise<TurnStartResult> => {
    const text =
      message.parts
        ?.filter((part): part is { type: "text"; text: string } => part.type === "text")
        .map((part) => part.text)
        .join(" ") ?? ""
    const result = await this.send(message, options)
    if (result.queued) {
      this.queuedTexts = text.trim() ? [...this.queuedTexts, text.trim()] : [...this.queuedTexts]
      this.commit()
    } else if (
      this.threadStatus !== null &&
      TERMINAL_THREAD_STATUSES.has(this.threadStatus)
    ) {
      // Lost the idle-window race and the run already ended: refresh now so
      // the history converges on the finished rollout.
      void this.reconnect()
    } else {
      // Lost the idle-window race: the input STARTED a new run that no local
      // stream is subscribed to. Defer the refresh to that run's terminal
      // event so the pane converges on the COMPLETED rollout rather than a
      // mid-run snapshot (its live events are unreachable from here).
      this.pendingResync = true
    }
    return result
  }

  /**
   * Clear the queued-steering texts on a run-terminal event. When anything was
   * queued (or a resync was owed from a lost steering race), also refresh the
   * restored history so the drained/persisted steering inputs materialize as
   * real message rows — the live stream never replays them. Caller commits.
   */
  private clearQueuedAndResync(): void {
    const shouldResync = this.queuedTexts.length > 0 || this.pendingResync
    this.pendingResync = false
    this.queuedTexts = []
    if (shouldResync) void this.reconnect()
  }

  /**
   * Interrupt the live turn on the bound thread (best-effort). Arrow-bound
   * (detachable): the React hook destructures this as the Stop control.
   */
  readonly interrupt = async (): Promise<void> => {
    const threadId = this.client.currentThreadId
    if (!threadId) return
    // Teardown persists any undelivered queued inputs into the rollout; flag a
    // resync so the terminal event materializes them as real rows.
    if (this.queuedTexts.length > 0) this.pendingResync = true
    this.queuedTexts = []
    this.commit()
    try {
      await this.client.turnInterrupt({ threadId, turnId: "0" })
    } catch (interruptError) {
      // Best-effort control: a thread that already terminated server-side
      // ("thread not found") is not actionable; anything else stays
      // observable instead of being silently swallowed by the caller.
      const message =
        interruptError instanceof Error ? interruptError.message : String(interruptError)
      if (!/thread not found/i.test(message)) {
        console.warn("[harness] turn/interrupt failed:", message)
      }
    }
  }

  /** Toggle plan mode. Resolving a plan approval clears it atomically. */
  readonly setPlanMode = (enabled: boolean): void => {
    this.planMode = enabled
    this.commit()
  }

  /** Resolve a pending approval via `approval/resolve` with a persistence scope. */
  readonly resolveApproval = async (
    itemId: string,
    approved: boolean,
    scope: ApprovalScope,
  ): Promise<void> => {
    const entry = this.approvals.get(itemId)
    if (!entry) return
    // Optimistically mark resolved so the banner/card update immediately.
    this.approvals = new Map(this.approvals)
    this.approvals.set(itemId, { ...entry, status: approved ? "approved" : "denied" })
    this.commit()
    // Approving a plan clears plan mode: the next turn/start carries no
    // `agentType`, so it runs as the default agent with the full tool set.
    // Rejection keeps plan mode on.
    if (approved && entry.kind === "plan") {
      this.planMode = false
      this.commit()
    }
    try {
      const result = await this.client.approvalResolve({
        threadId: entry.threadId,
        itemId,
        approved,
        scope,
      })
      // If the server couldn't deliver the decision (e.g. the pending entry
      // was gone), revert to pending so the user sees it wasn't actioned.
      if (result.delivered === false) {
        this.revertApprovalToPending(itemId)
        throw new Error("approval not delivered")
      }
    } catch (resolveError) {
      // Revert to pending if the server rejected the resolution.
      this.revertApprovalToPending(itemId)
      throw resolveError
    }
  }

  /** Manually compact the current (or given) thread via `thread/compact/start`. */
  readonly compactThread = async (threadId?: string): Promise<void> => {
    const tid = threadId ?? this.client.currentThreadId
    if (!tid) {
      this.actionError = { kind: "compact", message: "no active thread" }
      this.commit()
      return
    }
    this.error = null
    this.actionError = null
    this.isCompacting = true
    const markerId = `manual:${tid}:${Date.now()}`
    this.compactionMarkers = [
      ...this.compactionMarkers,
      { id: markerId, mode: "manual", phase: "compacting", threadId: tid },
    ]
    this.commit()
    try {
      await this.client.threadCompactStart({ threadId: tid })
      // Re-resume so the pane re-renders with the compacted history.
      const { thread } = await this.client.threadResume({ threadId: tid })
      const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
      this.client.lastTurnIndex = computeLastTurnIndex(thread)
      this.restoredMessages = messages
      this.userMessageTurnIndex = buildUserMessageTurnIndex(thread)
      // historyCreatedAt is unchanged (same thread). Flip the marker to done;
      // it survives the restoreVersion remount because it lives here.
      this.compactionMarkers = this.compactionMarkers.map((m) =>
        m.id === markerId ? { ...m, phase: "compacted" } : m,
      )
      this.restoreVersion += 1
      this.commit()
    } catch (compactError) {
      // Drop the marker — no compaction happened. Surface as an action error
      // (separate from restore errors) so the user sees why nothing changed.
      this.compactionMarkers = this.compactionMarkers.filter((m) => m.id !== markerId)
      this.actionError = {
        kind: "compact",
        message: compactError instanceof Error ? compactError.message : "compact failed",
      }
      this.commit()
    } finally {
      this.isCompacting = false
      this.commit()
    }
  }

  /** Fork the current (or given) thread via `thread/fork`, then switch to the child. */
  readonly forkThread = async (threadId?: string): Promise<void> => {
    const tid = threadId ?? this.client.currentThreadId
    if (!tid) {
      this.actionError = { kind: "fork", message: "no active thread" }
      this.commit()
      return
    }
    this.error = null
    this.actionError = null
    this.isForking = true
    this.commit()
    try {
      // Fork returns a child thread under the same slab session; rebind the
      // live socket to the child and re-render its (copied) history. The
      // parent thread is retained on disk but is not reachable from the UI
      // until a future thread-picker.
      const { thread: child } = await this.client.threadFork({ threadId: tid })
      const { thread } = await this.client.threadResume({ threadId: child.id })
      const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
      this.client.currentThreadId = thread.id
      this.client.lastTurnIndex = computeLastTurnIndex(thread)
      this.restoredMessages = messages
      this.userMessageTurnIndex = buildUserMessageTurnIndex(thread)
      this.restoredThreadId = thread.id
      this.historyCreatedAt = thread.createdAt
      this.restoreVersion += 1
      this.commit()
    } catch (forkError) {
      this.actionError = {
        kind: "fork",
        message: forkError instanceof Error ? forkError.message : "fork failed",
      }
      this.commit()
    } finally {
      this.isForking = false
      this.commit()
    }
  }

  /**
   * Retract `turnIndex` and every later turn via `thread/rollback` (turn 0 is
   * a no-op).
   */
  readonly rollbackFromTurn = async (turnIndex: number): Promise<void> => {
    const tid = this.client.currentThreadId
    // turnIndex is the first turn to remove (the user message being retracted
    // and everything after it). Turn 0 cannot be retracted this way.
    if (!tid || turnIndex <= 0) return
    this.error = null
    this.isRollingBack = true
    this.commit()
    try {
      await this.client.threadRollback({ threadId: tid, toTurnId: String(turnIndex - 1) })
      const { thread } = await this.client.threadResume({ threadId: tid })
      const messages = turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
      this.client.lastTurnIndex = computeLastTurnIndex(thread)
      this.restoredMessages = messages
      this.userMessageTurnIndex = buildUserMessageTurnIndex(thread)
      this.restoreVersion += 1
      this.commit()
    } catch (rollbackError) {
      this.error = rollbackError instanceof Error ? rollbackError.message : "rollback failed"
      this.commit()
    } finally {
      this.isRollingBack = false
      this.commit()
    }
  }

  /** Cancel in-flight restore work, drop listeners, and close the client. Idempotent. */
  dispose(): void {
    this.generation += 1
    this.unsubscribeNotifications?.()
    this.unsubscribeNotifications = null
    this.listeners.clear()
    this.client.close()
  }

  // ── internals ─────────────────────────────────────────────────────────────

  private revertApprovalToPending(itemId: string): void {
    const existing = this.approvals.get(itemId)
    if (!existing) return
    this.approvals = new Map(this.approvals)
    this.approvals.set(itemId, { ...existing, status: "pending" })
    this.commit()
  }

  /** Rebuild the snapshot (deriving the read-only projections) and notify. */
  private commit(): void {
    const approvals = Array.from(this.approvals.values())
    // The per-item maps (`liveOutput` / `livePatch` / `userMessageTurnIndex`)
    // are replaced on mutation, never edited in place, so the snapshot can
    // carry the references directly: unrelated commits keep the identities
    // stable and React consumers (memoized rows, context providers) skip
    // re-rendering. The approval-status projection is cached per `approvals`
    // instance for the same reason.
    let approvalStatusByItemId = approvalStatusCache.get(this.approvals)
    if (!approvalStatusByItemId) {
      approvalStatusByItemId = new Map<string, ApprovalStatus>()
      for (const [id, req] of this.approvals) approvalStatusByItemId.set(id, req.status)
      approvalStatusCache.set(this.approvals, approvalStatusByItemId)
    }

    this.snapshot = {
      restoredMessages: this.restoredMessages,
      restoredThreadId: this.restoredThreadId,
      activeConversation: this.activeConversation,
      restoreVersion: this.restoreVersion,
      isHistoryLoading: this.isHistoryLoading,
      error: this.error,
      actionError: this.actionError,
      approvals: approvals.filter((a) => a.status === "pending"),
      approvalStatusByItemId,
      liveOutputByItemId: this.liveOutput,
      livePatchByItemId: this.livePatch,
      modelLoad: this.modelLoad,
      turnUsage: this.turnUsage,
      historyCreatedAt: this.historyCreatedAt,
      commands: this.commands,
      compactionMarkers: this.compactionMarkers,
      isCompacting: this.isCompacting,
      isForking: this.isForking,
      isRollingBack: this.isRollingBack,
      userMessageTurnIndex: this.userMessageTurnIndex,
      planMode: this.planMode,
      threadStatus: this.threadStatus,
      abortReason: this.abortReason,
      queuedCount: this.queuedTexts.length,
      queuedTexts: [...this.queuedTexts],
      backgroundTasks: this.backgroundTasks,
    }
    for (const listener of this.listeners) listener()
  }

  /** The 8-notification projection previously living in the hook's effect. */
  private readonly handleNotification = (notification: JsonRpcNotification): void => {
    const { method } = notification

    // Item finalization: drop the per-item live accumulations (streamed
    // output / patch lines). The finalized item carries its own content, and
    // keeping the streamed copies would grow the maps without bound over a
    // long session (the C6 memory leak). Resolved approval entries stay —
    // the in-card Approved/Denied badge renders from them.
    if (method === HARNESS_NOTIFICATION.ITEM_COMPLETED) {
      const params = (notification.params ?? {}) as {
        threadId?: string
        item?: { id?: string }
      }
      if (params.threadId !== undefined && params.threadId !== this.client.currentThreadId) {
        return
      }
      const itemId = params.item?.id
      if (itemId === undefined) return
      const hadOutput = this.liveOutput.has(itemId)
      const hadPatch = this.livePatch.has(itemId)
      if (!hadOutput && !hadPatch) return
      if (hadOutput) {
        this.liveOutput = new Map(this.liveOutput)
        this.liveOutput.delete(itemId)
      }
      if (hadPatch) {
        this.livePatch = new Map(this.livePatch)
        this.livePatch.delete(itemId)
      }
      this.commit()
      return
    }

    // Accumulate live command output (stdout/stderr deltas) so the terminal
    // card can render output as it streams, before `item/completed` finalizes.
    if (method === HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_OUTPUT_DELTA) {
      const params = (notification.params ?? {}) as CommandExecutionOutputDeltaParams
      if (params.threadId !== this.client.currentThreadId) return
      const existing = this.liveOutput.get(params.itemId) ?? ""
      // Bound per-item accumulation so a runaway command can't exhaust memory.
      if (existing.length + params.delta.length > 256 * 1024) return
      this.liveOutput = new Map(this.liveOutput)
      this.liveOutput.set(params.itemId, existing + params.delta)
      this.commit()
      return
    }

    // Accumulate live apply_patch progress (one JSON line per committed file)
    // so the file-change card can show files as they apply, before
    // `item/completed` finalizes the change set.
    if (method === HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_OUTPUT_DELTA) {
      const params = (notification.params ?? {}) as FileChangeOutputDeltaParams
      if (params.threadId !== this.client.currentThreadId) return
      const existing = this.livePatch.get(params.itemId) ?? []
      // Bound per-item accumulation so a huge patch can't exhaust memory.
      if (existing.length >= 1024) return
      this.livePatch = new Map(this.livePatch)
      this.livePatch.set(params.itemId, [...existing, params.delta])
      this.commit()
      return
    }

    // Transient model-load indicator: set on each delta, clear on completed.
    // The load is per-turn and short-lived; a `completed` always resets it, so
    // no thread filtering is needed (one active turn on the socket at a time).
    if (method === HARNESS_NOTIFICATION.MODEL_LOAD_DELTA) {
      const params = (notification.params ?? {}) as ModelLoadDeltaParams
      this.modelLoad = {
        phase: params.phase,
        modelId: params.modelId,
        downloadedBytes: params.downloadedBytes,
        totalBytes: params.totalBytes,
      }
      this.commit()
      return
    }
    if (method === HARNESS_NOTIFICATION.MODEL_LOAD_COMPLETED) {
      this.modelLoad = null
      this.commit()
      return
    }

    // Context-compaction lifecycle: an in-progress auto-compaction adds a
    // "compacting" marker; the terminal event either flips it to "compacted"
    // or removes it (status "skipped" = a started compaction that didn't shrink).
    if (method === HARNESS_NOTIFICATION.CONTEXT_COMPACTING) {
      const params = (notification.params ?? {}) as ContextCompactingParams
      if (params.threadId !== this.client.currentThreadId) return
      // Only one auto-compacting marker per thread at a time.
      if (
        this.compactionMarkers.some(
          (m) =>
            m.mode === "auto" &&
            m.phase === "compacting" &&
            m.threadId === params.threadId,
        )
      ) {
        return
      }
      this.compactionMarkers = [
        ...this.compactionMarkers,
        {
          id: `auto:${params.threadId}:${Date.now()}`,
          mode: "auto",
          phase: "compacting",
          threadId: params.threadId,
        },
      ]
      this.commit()
      return
    }
    if (method === HARNESS_NOTIFICATION.CONTEXT_COMPACTED) {
      const params = (notification.params ?? {}) as ContextCompactedParams
      if (params.threadId !== this.client.currentThreadId) return
      if (params.status === "skipped") {
        // Started but did not compact — drop the in-progress marker.
        this.compactionMarkers = this.compactionMarkers.filter(
          (m) =>
            !(m.mode === "auto" && m.threadId === params.threadId && m.phase === "compacting"),
        )
      } else {
        this.compactionMarkers = this.compactionMarkers.map((m) =>
          m.mode === "auto" && m.threadId === params.threadId && m.phase === "compacting"
            ? { ...m, phase: "compacted" }
            : m,
        )
      }
      this.commit()
      return
    }

    // Resident background task lifecycle (shell background=true): track the
    // latest state per task. Terminal states stay listed — the timeline shows
    // how tasks ended and the count is bounded by the registry.
    if (method === HARNESS_NOTIFICATION.BACKGROUND_TASK_UPDATED) {
      const params = (notification.params ?? {}) as {
        threadId?: string
        taskId?: string
        status?: string
        exitCode?: number | null
        pid?: number | null
        command?: string | null
      }
      if (params.threadId !== undefined && params.threadId !== this.client.currentThreadId) {
        return
      }
      const taskId = params.taskId
      if (taskId === undefined) return
      const info: BackgroundTaskInfo = {
        taskId,
        status: (params.status ?? "running") as BackgroundTaskInfo["status"],
        exitCode: params.exitCode ?? null,
        pid: params.pid ?? null,
        command: params.command ?? null,
      }
      const existing = this.backgroundTasks.findIndex((task) => task.taskId === taskId)
      if (existing >= 0) {
        const next = [...this.backgroundTasks]
        next[existing] = info
        this.backgroundTasks = next
      } else {
        this.backgroundTasks = [...this.backgroundTasks, info]
      }
      this.commit()
      return
    }

    // Authoritative thread status from the server — the UI's source of truth
    // for busy/terminal state (replaces deriving busy from a local AI-SDK
    // status heuristic that diverges after interrupts).
    if (method === HARNESS_NOTIFICATION.THREAD_STATUS_CHANGED) {
      const params = (notification.params ?? {}) as ThreadStatusChangedParams
      if (params.threadId !== this.client.currentThreadId) return
      this.threadStatus = params.status as ThreadStatusString
      if (this.threadStatus === "running") this.abortReason = null
      if (TERMINAL_THREAD_STATUSES.has(this.threadStatus)) {
        // The run ended; anything still queued client-side was drained into
        // the run or persisted server-side (steering leftovers land in the
        // rollout history).
        this.clearQueuedAndResync()
      }
      this.commit()
      return
    }

    // Capture the finalized token usage + the run's termination reason for
    // the just-completed turn, and clear the queued-steering display (the
    // inputs were either consumed mid-run or persisted on teardown).
    if (method === HARNESS_NOTIFICATION.TURN_COMPLETED) {
      const params = (notification.params ?? {}) as TurnCompletedParams
      this.turnUsage = params.usage ?? null
      const reason = typeof params.reason === "string" ? params.reason : null
      this.abortReason = reason && reason !== "completed" ? reason : null
      this.clearQueuedAndResync()
      this.commit()
      return
    }

    const isCommandApproval =
      method === HARNESS_NOTIFICATION.ITEM_COMMAND_EXECUTION_REQUEST_APPROVAL
    const isFileApproval = method === HARNESS_NOTIFICATION.ITEM_FILE_CHANGE_REQUEST_APPROVAL
    if (!isCommandApproval && !isFileApproval) return

    const params = (notification.params ?? {}) as
      | CommandExecutionRequestApprovalParams
      | FileChangeRequestApprovalParams
    // Only track approvals for the currently bound thread on this socket.
    if (params.threadId !== this.client.currentThreadId) return

    if (this.approvals.has(params.itemId)) return
    this.approvals = new Map(this.approvals)
    if (isCommandApproval) {
      const command = params as CommandExecutionRequestApprovalParams
      // A `present_plan` approval carries a full plan snapshot; render it as
      // a plan approval (rich card) instead of a plain command approval.
      const isPlan = command.planSnapshot !== undefined
      this.approvals.set(params.itemId, {
        itemId: params.itemId,
        threadId: params.threadId,
        kind: isPlan ? "plan" : "command",
        command: command.command,
        cwd: command.cwd,
        reason: command.reason,
        category: command.category,
        allowedScopes: command.allowedScopes,
        // The wire carries the raw serialized `slab_agent::Plan` (snake_case
        // fields); narrow it for the rich plan card.
        planSnapshot: command.planSnapshot as Plan | undefined,
        status: "pending",
      })
    } else {
      const file = params as FileChangeRequestApprovalParams
      this.approvals.set(params.itemId, {
        itemId: params.itemId,
        threadId: params.threadId,
        kind: "fileChange",
        changes: file.changes,
        allowedScopes: file.allowedScopes,
        status: "pending",
      })
    }
    this.commit()
  }
}
