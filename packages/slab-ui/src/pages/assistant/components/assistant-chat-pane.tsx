"use client"

import { useChat } from "@ai-sdk/react"
import type { FileUIPart, UIMessage } from "ai"
import { MessageCircleDashedIcon, SquarePenIcon } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react"

import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { Card, CardContent, CardFooter } from "@slab/components/card"
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@slab/components/empty"
import {
    MessageScrollerProvider,
} from "@slab/components/message-scroller"

import MessageList from "@slab/ui/pages/assistant/components/message-list"
import { TokenUsageIndicator } from "@slab/ui/pages/assistant/components/token-usage-indicator"
import Sender from "@slab/ui/pages/assistant/components/sender.tsx"
import {
    LiveToolOutputContext,
    MessageInteractionContext,
} from "@slab/ui/pages/assistant/components/message-interaction-context"
import { resolveCommandDispatch } from "@slab/ui/pages/assistant/lib/assistant-commands"
import { useWorkspaceConfirmDialog } from "@slab/ui/pages/workspace/hooks/use-workspace-confirm"
import { useGreeting } from "../hooks/use-greeting"
import type {
    ApprovalScope,
    CommandInfo,
    TurnUsage,
} from "@slab/api/harness"
import type {
    ApprovalRequest,
    ApprovalStatus,
    BackgroundTaskInfo,
    CompactionMarker,
    HarnessChatTransport,
    LiveTextEntry,
    ModelLoadState,
    SubagentChildItem,
    SubagentTaskInfo,
    ThreadStatusString,
    TurnSendOptions,
} from "@slab/core/harness"
import { toast } from "sonner"

/** Static i18n keys per wire abort reason (static strings satisfy the key guards). */
const ABORT_REASON_LABEL_KEYS: Record<string, string> = {
    interrupted: "pages.assistant.turn.abortReason.interrupted",
    max_turns_reached: "pages.assistant.turn.abortReason.maxTurnsReached",
    budget_exhausted: "pages.assistant.turn.abortReason.budgetExhausted",
    repetition_detected: "pages.assistant.turn.abortReason.repetitionDetected",
    error: "pages.assistant.turn.abortReason.error",
}

export type AssistantChatPaneProps = {
    disabled: boolean
    initialMessages: UIMessage[]
    isHistoryLoading: boolean
    modelStatusLabel: string
    onBeforeSubmit: (value: string) => Promise<void>
    onBusyChange: (busy: boolean) => void
    onMessageCountChange: (count: number) => void
    transport: HarnessChatTransport<UIMessage>
    approvals: ApprovalRequest[]
    approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
    liveOutputByItemId: ReadonlyMap<string, string>
    livePatchByItemId: ReadonlyMap<string, string[]>
    /** Transient model-load indicator state (null when idle). */
    modelLoad: ModelLoadState
    /** Token usage for the most recent completed turn (null until first turn). */
    turnUsage: TurnUsage | null
    /** Context window size for the consumption bar (null when unknown). */
    contextWindow: number | null
    resolveApproval: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void>
    /** Manually compact the current thread (triggered by the `/compact` command). */
    onCompact: () => Promise<void>
    /** Fork the current thread (triggered by the `/fork` command), switching to the child. */
    onFork: () => Promise<void>
    /** `thread.createdAt` (Unix ms) for the history-restored marker label. */
    historyCreatedAt: number | null
    /** Command registry snapshot driving the `/`-menu + dispatch (`command/list`). */
    commands: CommandInfo[]
    /** Session-scoped compaction markers rendered as in-stream dividers. */
    compactionMarkers: CompactionMarker[]
    /** True while a manual `/compact` round-trip is in flight. */
    isCompacting: boolean
    /** True while a `/fork` round-trip is in flight. */
    isForking: boolean
    /** userMessage itemId → turn index (drives the per-bubble rollback affordance). */
    userMessageTurnIndex: ReadonlyMap<string, number>
    /** Retract a turn (and everything after it) via `thread/rollback`. */
    onRollbackFromTurn: (turnIndex: number) => Promise<void>
    /** Whether plan mode is active (turn runs as the read-only plan agent). */
    planMode: boolean
    /** Toggle plan mode on/off; drives the plan chip + `/plan`. */
    onPlanModeChange: (enabled: boolean) => void
    /** Authoritative thread status from `thread/statusChanged` (null before the first event). */
    threadStatus: ThreadStatusString | null
    /** Why the last run ended abnormally (null after a clean completion). */
    abortReason: string | null
    /** Steering inputs queued on the running turn; ghost bubbles at the stream tail. */
    queuedTexts: readonly string[]
    /** Resident background tasks; RUNNING ones render a status Marker at the tail. */
    backgroundTasks: readonly BackgroundTaskInfo[]
    /** taskId → live subagent delegation state (drives the delegate tool card). */
    subagentTasksByTaskId: ReadonlyMap<string, SubagentTaskInfo>
    /** childThreadId → relayed child turn items (live child activity in the card). */
    subagentChildItemsByChildId: ReadonlyMap<string, readonly SubagentChildItem[]>
    /** Steering send — submits while the turn runs queue at the iteration boundary. */
    onSteerSubmit: (text: string, options?: TurnSendOptions) => Promise<unknown>
    /**
     * Server-side interrupt for the Stop control (best-effort, errors swallowed
     * upstream). Composed with the local AI-SDK `stop()` so the UI unblocks
     * immediately while the server terminates the run authoritatively.
     */
    onInterrupt: () => void
    /** Leave for the new-chat landing (empty-state CTA; homepage navigation). */
    onStartNewChat?: () => void
    /**
     * A staged first message (new-chat dialog handoff): sent through the exact
     * manual-submit path once the conversation controller is ready. Consumed
     * via {@link onAutoSendConsumed} *before* sending so a pane remount cannot
     * double-deliver it.
     */
    autoSend?: {
        text: string
        files: FileUIPart[]
        metadata: { effort?: unknown; permissionMode?: unknown; agentType?: unknown }
        /** Delivery attempts used so far — distinguishes a re-staged retry. */
        attempts?: number
    } | null
    /** Called exactly once when {@link autoSend} is taken (clears the draft). */
    onAutoSendConsumed?: () => void
    /**
     * A claimed draft failed BEFORE the turn started server-side (gate throw,
     * transport failure). The page re-stages the draft (bounded) or toasts.
     */
    onAutoSendFailed?: (payload: { attempts: number }) => void
    /**
     * Fired from this pane's unmount cleanup while its `useChat` stream was
     * attached (`submitted`/`streaming`): the server run keeps going with no
     * local renderer — the controller flags a terminal-triggered resync so
     * the finished rollout remounts the pane with the reply.
     */
    onPaneDetached?: () => void
    /** Controller mirror of in-flight assistant/reasoning text (orphan tail). */
    liveTextByItemId: ReadonlyMap<string, LiveTextEntry>
    /**
     * Monotonic count of server-accepted local turns; snapshot before a draft
     * send and compare on failure (unchanged ⇒ the turn never started ⇒ the
     * draft may be re-staged).
     */
    localTurnStartSeq: number
    /** Live workspace selector rendered inside the Sender toolbar. */
    workspaceSlot?: ReactNode
}

export function AssistantChatPane({
    disabled,
    initialMessages,
    isHistoryLoading,
    modelStatusLabel,
    onBeforeSubmit,
    onBusyChange,
    onMessageCountChange,
    transport,
    approvals,
    approvalStatusByItemId,
    liveOutputByItemId,
    livePatchByItemId,
    modelLoad,
    turnUsage,
    contextWindow,
    resolveApproval,
    onCompact,
    onFork,
    historyCreatedAt,
    commands,
    compactionMarkers,
    isCompacting,
    isForking,
    userMessageTurnIndex,
    onRollbackFromTurn,
    planMode,
    onPlanModeChange,
    threadStatus,
    abortReason,
    queuedTexts,
    backgroundTasks,
    subagentTasksByTaskId,
    subagentChildItemsByChildId,
    onSteerSubmit,
    onInterrupt,
    onStartNewChat,
    autoSend,
    onAutoSendConsumed,
    onAutoSendFailed,
    onPaneDetached,
    liveTextByItemId,
    localTurnStartSeq,
    workspaceSlot,
}: AssistantChatPaneProps) {
    const { t } = useTranslation()
    const { messages, sendMessage, status, stop } = useChat({
        messages: initialMessages,
        transport,
    })
    const isBusy = status === "submitted" || status === "streaming"
    // Server-authoritative busy: the submit gate derives from THIS, not from
    // the local AI-SDK status (the two diverge after interrupts, and a normal
    // `sendMessage` against a busy server thread would misbehave).
    const serverBusy =
        threadStatus === "running" ||
        threadStatus === "interrupting" ||
        threadStatus === "pending"
    const steerable = serverBusy
    const abortReasonLabel = abortReason ? ABORT_REASON_LABEL_KEYS[abortReason] : undefined
    // In-flight assistant text mirrored by the controller: rendered as tail
    // bubbles ONLY when this pane's own stream is not attached to the run
    // (remount-orphaned or mid-run reload) — otherwise the same deltas would
    // double-render through useChat.
    const liveTailTexts =
        !isBusy && liveTextByItemId.size > 0 ? Array.from(liveTextByItemId.values()) : []
    const greeting = useGreeting()

    const { confirm: confirmRollback, dialog: rollbackConfirmDialog } = useWorkspaceConfirmDialog()
    const handleRollbackMessage = useCallback(
        async (messageId: string) => {
            const turnIndex = userMessageTurnIndex.get(messageId)
            // Only user messages with a turn index > 0 offer rollback (turn 0
            // can't be retracted — there is nothing before it to keep).
            if (turnIndex === undefined || turnIndex <= 0) return
            const ok = await confirmRollback({
                messageKey: "pages.assistant.message.confirmRollback",
                confirmKey: "pages.assistant.message.rollback",
                tone: "danger",
            })
            if (!ok) return
            await onRollbackFromTurn(turnIndex)
        },
        [confirmRollback, onRollbackFromTurn, userMessageTurnIndex],
    )

    // Slow-changing row-level interaction data (approval badges, rollback) is
    // kept OUT of the streaming context: an output delta then re-renders only
    // the active tool card instead of every visible row.
    const interactionValue = useMemo(
        () => ({
            approvalStatusByItemId,
            userMessageTurnIndex,
            rollbackToMessage: handleRollbackMessage,
            subagentTasksByTaskId,
            subagentChildItemsByChildId,
        }),
        [
            approvalStatusByItemId,
            userMessageTurnIndex,
            handleRollbackMessage,
            subagentTasksByTaskId,
            subagentChildItemsByChildId,
        ],
    )
    const liveToolOutputValue = useMemo(
        () => ({ liveOutputByItemId, livePatchByItemId }),
        [liveOutputByItemId, livePatchByItemId],
    )

    useEffect(() => {
        onBusyChange(isBusy)
    }, [isBusy, onBusyChange])

    useEffect(() => {
        onMessageCountChange(messages.length)
    }, [messages.length, onMessageCountChange])

    // New-chat dialog handoff: deliver the staged draft through the exact
    // manual-submit path once the controller is ready. The claim happens
    // BEFORE sending, so the `${conversation}:${restoreVersion}` pane remount
    // (and React StrictMode double-invocation) can never double-deliver; the
    // claim key guards against effect re-runs for the same staged message.
    // `attempts` is part of the key — a re-staged retry (same text) must not
    // be swallowed by the guard. A failure BEFORE the server accepted the
    // turn (gate throw, transport failure) reports via `onAutoSendFailed` so
    // the page can re-stage the draft (bounded) instead of silently losing
    // the first message.
    const claimedAutoSendRef = useRef<string | null>(null)
    const pendingAutoSendRef = useRef<{ seqBefore: number; attempts: number } | null>(null)
    const localTurnStartSeqRef = useRef(localTurnStartSeq)
    useEffect(() => {
        localTurnStartSeqRef.current = localTurnStartSeq
    }, [localTurnStartSeq])
    const readyForAutoSend =
        !!autoSend && !!onAutoSendConsumed && !disabled && !isHistoryLoading && !isBusy && !steerable
    useEffect(() => {
        if (!readyForAutoSend || !autoSend || !onAutoSendConsumed) return
        const claimKey = `${autoSend.text}:${autoSend.metadata?.effort ?? ""}:${autoSend.attempts ?? 0}`
        if (claimedAutoSendRef.current === claimKey) return
        claimedAutoSendRef.current = claimKey
        onAutoSendConsumed()
        pendingAutoSendRef.current = {
            seqBefore: localTurnStartSeqRef.current,
            attempts: autoSend.attempts ?? 0,
        }
        void (async () => {
            try {
                await onBeforeSubmit(autoSend.text)
            } catch {
                pendingAutoSendRef.current = null
                // The gate already toasted why the session isn't ready; the
                // draft is gone unless the page re-stages it.
                onAutoSendFailed?.({ attempts: autoSend.attempts ?? 0 })
                return
            }
            try {
                sendMessage({
                    text: autoSend.text,
                    files: autoSend.files,
                    metadata: autoSend.metadata,
                })
            } catch {
                pendingAutoSendRef.current = null
                onAutoSendFailed?.({ attempts: autoSend.attempts ?? 0 })
            }
        })()
    }, [readyForAutoSend, autoSend, onAutoSendConsumed, onAutoSendFailed, onBeforeSubmit, sendMessage])

    // AI-SDK routes transport/stream failures into the `status === "error"`
    // state (sendMessage's promise does not reject). Consume a pending draft
    // failure: when the accepted-turn sequence has NOT advanced, the turn
    // never started server-side and the page may re-stage the draft; when it
    // HAS advanced, a server-side run exists and the orphan-recovery path
    // owns its display — re-sending would double-deliver.
    useEffect(() => {
        if (status !== "error") return
        const pending = pendingAutoSendRef.current
        if (!pending) return
        pendingAutoSendRef.current = null
        if (localTurnStartSeqRef.current !== pending.seqBefore) return
        onAutoSendFailed?.({ attempts: pending.attempts })
    }, [status, onAutoSendFailed])

    // Orphan detection: unmounting while this pane's stream is attached means
    // the (still-running) server turn has no local renderer anymore. Tell the
    // controller so the terminal-triggered resync remounts the pane with the
    // finished rollout. Runs on unmount only; `onPaneDetached` is a stable
    // controller arrow.
    const statusRef = useRef(status)
    useEffect(() => {
        statusRef.current = status
    }, [status])
    useEffect(
        () => () => {
            if (statusRef.current === "submitted" || statusRef.current === "streaming") {
                onPaneDetached?.()
            }
        },
        [onPaneDetached],
    )

    return (
        <MessageScrollerProvider defaultScrollPosition="last-anchor">
            <div className="relative flex min-h-0 flex-1 flex-col bg-card">
                <Card className="h-full w-full gap-0 border-none shadow-none">
                    <CardContent className="flex-1 overflow-hidden p-0">
                        <div className="flex h-full flex-col">
                            <div className="min-h-0 flex-1">
                                {messages.length === 0 && !isHistoryLoading ? (
                                    <Empty className="h-full" data-testid="assistant-empty-state">
                                        <EmptyHeader>
                                            <EmptyMedia variant="icon">
                                                <MessageCircleDashedIcon />
                                            </EmptyMedia>
                                            <EmptyTitle>{greeting}</EmptyTitle>
                                            <EmptyDescription>
                                                {t("pages.assistant.hero.description")}
                                            </EmptyDescription>
                                        </EmptyHeader>
                                        {onStartNewChat ? (
                                            <EmptyDescription>
                                                <Button
                                                    variant="outline"
                                                    size="sm"
                                                    data-testid="assistant-new-chat-cta"
                                                    onClick={onStartNewChat}
                                                >
                                                    <SquarePenIcon className="size-4" />
                                                    {t("pages.assistant.newChat.cta")}
                                                </Button>
                                            </EmptyDescription>
                                        ) : null}
                                    </Empty>
                                ) : (
                                    <>
                                        <LiveToolOutputContext.Provider value={liveToolOutputValue}>
                                            <MessageInteractionContext.Provider value={interactionValue}>
                                                <MessageList
                                                    messages={messages}
                                                    isBusy={isBusy}
                                                    showHistoryMarker={initialMessages.length > 0}
                                                    historyCount={initialMessages.length}
                                                    historyCreatedAt={historyCreatedAt}
                                                    compactionMarkers={compactionMarkers}
                                                    modelLoad={modelLoad}
                                                    sessionLoading={isHistoryLoading}
                                                    queuedTexts={queuedTexts}
                                                    backgroundTasks={backgroundTasks}
                                                    liveTailTexts={liveTailTexts}
                                                />
                                            </MessageInteractionContext.Provider>
                                        </LiveToolOutputContext.Provider>
                                        {rollbackConfirmDialog}
                                    </>
                                )}
                            </div>
                        </div>
                    </CardContent>
                    <CardFooter className="flex-col gap-2">
                        <TokenUsageIndicator usage={turnUsage} contextWindow={contextWindow} />
                        {abortReasonLabel ? (
                            <p
                                className="w-full truncate text-xs text-muted-foreground"
                                data-testid="assistant-abort-reason"
                            >
                                {t(abortReasonLabel)}
                            </p>
                        ) : null}
                        <Sender
                            onSubmit={async (value, { files, effort, permissionMode, agentType, approvalModel, approvalPrompt }) => {
                                // Registry-driven dispatch: Control commands run a
                                // host action and never reach the model. `/plan` is
                                // intercepted by the Sender (toggle, never submitted);
                                // everything else (skills, plain text) reaches sendMessage.
                                const dispatch = resolveCommandDispatch(value, commands)
                                if (dispatch.action === "control") {
                                    // Both actions unconditionally bump the
                                    // restore version (remounting this pane);
                                    // while a run is live that would orphan
                                    // the stream — refuse until it settles.
                                    if (serverBusy || isBusy || isCompacting || isForking) {
                                        toast.info(t("pages.assistant.toast.sessionBusy"))
                                        return
                                    }
                                    if (dispatch.controlAction === "compact") {
                                        await onCompact()
                                        return
                                    }
                                    if (dispatch.controlAction === "fork") {
                                        await onFork()
                                        return
                                    }
                                }
                                // Steering: while the server-side turn is running,
                                // submit queues at the iteration boundary instead of
                                // opening a second (double-delivering) AI-SDK stream.
                                // This bypasses `onBeforeSubmit` on purpose — that
                                // gate throws while the session is busy, which it
                                // always is here (the turn is running).
                                if (steerable) {
                                    await onSteerSubmit(value, {
                                        effort,
                                        permissionMode,
                                        agentType,
                                        approvalModel,
                                        approvalPrompt,
                                    })
                                    return
                                }
                                await onBeforeSubmit(value)
                                sendMessage({
                                    text: value,
                                    files,
                                    metadata: {
                                        effort,
                                        permissionMode,
                                        agentType,
                                        approvalModel,
                                        approvalPrompt,
                                    },
                                })
                            }}
                            onStop={() => {
                                // Local abort for immediate feedback + the
                                // authoritative server-side interrupt (threadStatus
                                // flips via `thread/statusChanged`).
                                stop()
                                onInterrupt()
                            }}
                            loading={disabled || isBusy || isCompacting || isForking}
                            steerable={steerable && !disabled && !isCompacting && !isForking}
                            approvals={approvals}
                            onResolveApproval={resolveApproval}
                            commands={commands}
                            planMode={planMode}
                            onPlanModeChange={onPlanModeChange}
                            workspaceSlot={workspaceSlot}
                        />
                        <p
                            className="w-full truncate text-xs text-muted-foreground"
                            data-testid="assistant-model-status"
                        >
                            {modelStatusLabel}
                        </p>
                    </CardFooter>
                </Card>
            </div>
        </MessageScrollerProvider>
    )
}
