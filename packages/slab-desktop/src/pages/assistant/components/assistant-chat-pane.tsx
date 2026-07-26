"use client"

import { useChat } from "@ai-sdk/react"
import type { UIMessage } from "ai"
import { MessageCircleDashedIcon } from "lucide-react"
import { useEffect, useMemo } from "react"

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

import MessageList from "@/pages/assistant/components/message/index.tsx"
import { ModelLoadIndicator } from "@/pages/assistant/components/message/model-load-indicator"
import { TokenUsageIndicator } from "@/pages/assistant/components/message/token-usage-indicator"
import Sender from "@/pages/assistant/components/sender.tsx"
import { MessageInteractionContext } from "@/pages/assistant/components/message/message-interaction-context.ts"
import { useGreeting } from "../hooks/use-greeting"
import type {
    ApprovalRequest,
    ApprovalStatus,
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
    /** Transient model-load indicator state (null when idle). */
    modelLoad: ModelLoadState
    /** Token usage for the most recent completed turn (null until first turn). */
    turnUsage: TurnUsage | null
    /** Context window size for the consumption bar (null when unknown). */
    contextWindow: number | null
    resolveApproval: (itemId: string, approved: boolean, scope: ApprovalScope) => Promise<void>
    /** Manually compact the current thread (triggered by the `/compact` command). */
    onCompact: () => Promise<void>
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
    modelLoad,
    turnUsage,
    contextWindow,
    resolveApproval,
    onCompact,
}: AssistantChatPaneProps) {
    const { t } = useTranslation()
    const { messages, sendMessage, status, stop } = useChat({
        messages: initialMessages,
        transport,
    })
    const isBusy = status === "submitted" || status === "streaming"
    const greeting = useGreeting()

    const interactionValue = useMemo(
        () => ({ approvalStatusByItemId, liveOutputByItemId }),
        [approvalStatusByItemId, liveOutputByItemId],
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
                                    <MessageInteractionContext.Provider value={interactionValue}>
                                        <MessageList
                                            messages={messages}
                                            isBusy={isBusy}
                                            showHistoryMarker={initialMessages.length > 0}
                                        />
                                    </MessageInteractionContext.Provider>
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
                                if (value.trim() === "/compact") {
                                    await onCompact()
                                    return
                                }
                                await onBeforeSubmit(value)
                                sendMessage({ text: value, files, metadata: { effort, permissionMode } })
                            }}
                            onStop={stop}
                            loading={disabled || isBusy}
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
