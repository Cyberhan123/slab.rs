"use client"

import { useChat } from "@ai-sdk/react"
import type { UIMessage } from "ai"
import { MessageCircleDashedIcon } from "lucide-react"
import { useEffect } from "react"

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
import Sender from "@/pages/assistant/components/sender.tsx"
import { useGreeting } from "../hooks/use-greeting"
import type { HarnessChatTransport } from "../lib/harness"

export type AssistantChatPaneProps = {
    disabled: boolean
    initialMessages: UIMessage[]
    isHistoryLoading: boolean
    modelStatusLabel: string
    onBeforeSubmit: (value: string) => Promise<void>
    onBusyChange: (busy: boolean) => void
    onMessageCountChange: (count: number) => void
    transport: HarnessChatTransport<UIMessage>
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
}: AssistantChatPaneProps) {
    const { t } = useTranslation()
    const { messages, sendMessage, status } = useChat({
        messages: initialMessages,
        transport,
    })
    const isBusy = status === "submitted" || status === "streaming"
    const greeting = useGreeting()

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
                            <MessageList messages={messages} isBusy={isBusy} />
                        )}
                    </CardContent>
                    <CardFooter className="flex-col gap-2">
                        <Sender
                            onSubmit={async (value) => {
                                await onBeforeSubmit(value)
                                sendMessage({ text: value })
                            }}
                            loading={disabled || isBusy}
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
