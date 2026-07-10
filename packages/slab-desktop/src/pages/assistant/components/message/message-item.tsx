"use client"

import * as React from "react"
import type { UIDataTypes, UIMessagePart, UITools } from "ai"
import { motion, useReducedMotion } from "motion/react"
import { Button } from "@slab/components/button"
import type { MessageAnimationPreset } from "@/pages/assistant/lib/message-animations"
import { MESSAGE_ANIMATIONS } from "@/pages/assistant/lib/message-animations"
import AgentAvatar from "@/pages/assistant/components/message/agent-avatar"
import UserAvatar from "@/pages/assistant/components/message/user-avatar"
import { Message, MessageContent, MessageHeader, MessageAvatar, MessageFooter } from "@slab/components/message"
import { MessageScrollerItem } from "@slab/components/message-scroller"
import { useTranslation } from "@slab/i18n"
import { useClipboard } from "@mantine/hooks"
import { CheckIcon, CopyIcon } from "lucide-react"
import { cn } from "@/lib/utils"
import { createMessageParts, MessageParts } from "./message-parts"
import type { MessagePartComponents, MessagePartItem, MessagePartsResult } from "./message-parts"
import { Bubble, BubbleContent } from "@slab/components/bubble"

import MessageTextPart from "./message-text-part"
import MessageReasoningPart from "./message-reasoning-part"
import MessageFallbackPart from "./message-fallback-part"
import MessageToolPart from "./message-tool-part"

type TMessagePart = {
    state?: string;
    type: string
    text?: string
    [key: string]: unknown
}

type TMessage = {
    id: string
    role: string
    text?: string
    parts?: ReadonlyArray<TMessagePart | UIMessagePart<UIDataTypes, UITools>>
}

type TRenderableMessagePart = TMessagePart | UIMessagePart<UIDataTypes, UITools>

const MotionMessageScrollerItem = motion.create(MessageScrollerItem)

function MessageItem({
    message,
    animationPreset = MESSAGE_ANIMATIONS["slide-up"],
    scrollAnchor,
    ...props
}: Omit<
    React.ComponentProps<typeof MotionMessageScrollerItem>,
    "animate" | "children" | "exit" | "initial" | "messageId" | "variants"
> & {
    animationPreset?: MessageAnimationPreset
    message: TMessage
}) {
    const shouldReduceMotion = useReducedMotion()
    const isUserMessage = message.role === "user"

    if (isUserMessage) {
        return (
            <MotionMessageScrollerItem
                messageId={message.id}
                scrollAnchor={scrollAnchor ?? true}
                variants={animationPreset.variants}
                initial={shouldReduceMotion ? false : "initial"}
                animate="animate"
                exit={shouldReduceMotion ? undefined : "exit"}
                {...props}
            >
                <MessageRow
                    message={message}
                />
            </MotionMessageScrollerItem>
        )
    }

    return (
        <MotionMessageScrollerItem
            messageId={message.id}
            scrollAnchor={scrollAnchor}
            initial={false}
            {...props}
        >
            <MessageRow
                message={message}
            />
        </MotionMessageScrollerItem>
    )
}

const messagePartComponents: MessagePartComponents<TMessagePart, TMessage> = {
    text: MessageTextPart,
    reasoning: MessageReasoningPart,
    tool: MessageToolPart,     
    fallback: MessageFallbackPart,
    tools: {},
}


function MessageRow({
    message,
}: {
    message: TMessage
}) {
    const isUserMessage = message.role === "user"
    // `createMessageParts` defaults to protocol (temporal) order — a tool call
    // renders where it actually occurred relative to the text/reasoning.
    const parsedParts = createMessageParts<TMessage>(message) as MessagePartsResult<
        TRenderableMessagePart,
        TMessage
    > & { all: Array<MessagePartItem<TRenderableMessagePart, TMessage>> }
    const { t } = useTranslation()
    const clipboard = useClipboard({ timeout: 2000 })
    const plainText = (message.parts ?? [])
        .filter((part) => (part as TMessagePart).type === "text")
        .map((part) => (part as TMessagePart).text ?? "")
        .join("")
        .trim()
    return (
        <Message align={isUserMessage ? "end" : "start"}>
            <MessageAvatar className={cn("items-start self-start group-has-data-[slot=message-footer]/message:-translate-y-0")}>
                {isUserMessage ? <UserAvatar name={t("pages.assistant.message.user")} /> : <AgentAvatar name={t("pages.assistant.message.assistant")} />}
            </MessageAvatar>
            <MessageContent>
                <MessageHeader>{isUserMessage ? t("pages.assistant.message.user") : t("pages.assistant.message.assistant")}</MessageHeader>
                <Bubble
                    align={message.role === "user" ? "end" : "start"}
                    variant={message.role === "user" ? "tinted" : "outline"}
                >
                    <BubbleContent className="space-y-2">
                        <MessageParts<TRenderableMessagePart, TMessage>
                            parts={parsedParts}
                            components={messagePartComponents as MessagePartComponents<TRenderableMessagePart, TMessage>}
                        />
                    </BubbleContent>
                </Bubble>
                {plainText ? (
                    <MessageFooter>
                        <Button
                            variant="ghost"
                            size="icon"
                            aria-label="Copy"
                            title="Copy"
                            onClick={() => {
                                clipboard.copy(plainText)
                            }}
                        >
                            {clipboard.copied ? <CheckIcon /> : <CopyIcon />}
                        </Button>
                    </MessageFooter>
                ) : null}
            </MessageContent>
        </Message>
    )
}


export { MessageItem, type TMessage, type TMessagePart }
