"use client"

import { useChat } from "@ai-sdk/react"
import type { UIMessage } from "ai"
import { MessageCircleDashedIcon } from "lucide-react"
import { useCallback, useEffect, useMemo } from "react"

import { useTranslation } from "@slab/i18n"
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

import MessageList from "@/pages/assistant/components/message-list"
import { ModelLoadIndicator } from "@/pages/assistant/components/model-load-indicator"
import { TokenUsageIndicator } from "@/pages/assistant/components/token-usage-indicator"
import Sender from "@/pages/assistant/components/sender.tsx"
import { MessageInteractionContext } from "@/pages/assistant/components/message-interaction-context"
import { isCompactCommand, isForkCommand } from "@/pages/assistant/lib/assistant-commands"
import { useWorkspaceConfirmDialog } from "@/pages/workspace/hooks/use-workspace-confirm"
import { useGreeting } from "../hooks/use-greeting"
import type {
    ApprovalRequest,
    ApprovalStatus,
    CompactionMarker,
    ModelLoadState,
} from "../hooks/use-harness-conversation"
import type { ApprovalScope, HarnessChatTransport, TurnUsage } from "../lib/harness"

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
    compactionMarkers,
    isCompacting,
    isForking,
    userMessageTurnIndex,
    onRollbackFromTurn,
}: AssistantChatPaneProps) {
    const { t } = useTranslation()
    const { messages, sendMessage, status, stop } = useChat({
        messages: initialMessages,
        transport,
    })
    const isBusy = status === "submitted" || status === "streaming"
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

    const interactionValue = useMemo(
        () => ({
            approvalStatusByItemId,
            liveOutputByItemId,
            livePatchByItemId,
            userMessageTurnIndex,
            rollbackToMessage: handleRollbackMessage,
        }),
        [
            approvalStatusByItemId,
            liveOutputByItemId,
            livePatchByItemId,
            userMessageTurnIndex,
            handleRollbackMessage,
        ],
    )

    useEffect(() => {
        onBusyChange(isBusy)
    }, [isBusy, onBusyChange])

    useEffect(() => {
        onMessageCountChange(messages.length)
    }, [messages.length, onMessageCountChange])

    return (
        <MessageScrollerProvider defaultScrollPosition="last-anchor">
            <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
                <Card className="h-full w-full gap-0 border-none shadow-none">
                    <CardContent className="flex-1 overflow-hidden p-0">
                        <div className="flex h-full flex-col">
                            <div className="min-h-0 flex-1">
                                {isHistoryLoading && messages.length === 0 ? (
                                    <Empty className="h-full" data-testid="assistant-loading-state">
                                        <EmptyHeader>
                                            <EmptyMedia variant="icon">
                                                <MessageCircleDashedIcon />
                                            </EmptyMedia>
                                            <EmptyTitle>{t("pages.assistant.loading.title")}</EmptyTitle>
                                            <EmptyDescription>
                                                {t("pages.assistant.loading.description")}
                                            </EmptyDescription>
                                        </EmptyHeader>
                                    </Empty>
                                ) : messages.length === 0 ? (
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
                                    </Empty>
                                ) : (
                                    <>
                                        <MessageInteractionContext.Provider value={interactionValue}>
                                            <MessageList
                                                messages={messages}
                                                isBusy={isBusy}
                                                showHistoryMarker={initialMessages.length > 0}
                                                historyCount={initialMessages.length}
                                                historyCreatedAt={historyCreatedAt}
                                                compactionMarkers={compactionMarkers}
                                            />
                                        </MessageInteractionContext.Provider>
                                        {rollbackConfirmDialog}
                                    </>
                                )}
                            </div>
                        </div>
                    </CardContent>
                    <CardFooter className="flex-col gap-2">
                        <ModelLoadIndicator modelLoad={modelLoad} />
                        <TokenUsageIndicator usage={turnUsage} contextWindow={contextWindow} />
                        <Sender
                            onSubmit={async (value, { files, effort, permissionMode }) => {
                                // `/compact` is a control command — never reaches the model.
                                if (isCompactCommand(value)) {
                                    await onCompact()
                                    return
                                }
                                // `/fork` is a control command — never reaches the model.
                                if (isForkCommand(value)) {
                                    await onFork()
                                    return
                                }
                                await onBeforeSubmit(value)
                                sendMessage({ text: value, files, metadata: { effort, permissionMode } })
                            }}
                            onStop={stop}
                            loading={disabled || isBusy || isCompacting || isForking}
                            approvals={approvals}
                            onResolveApproval={resolveApproval}
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
