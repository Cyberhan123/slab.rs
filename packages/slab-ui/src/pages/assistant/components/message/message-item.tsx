"use client"

import * as React from "react"
import type { UIDataTypes, UIMessagePart, UITools } from "ai"
import { motion, useReducedMotion } from "motion/react"
import { Button } from "@slab/components/button"
import type { MessageAnimationPreset } from "@slab/ui/pages/assistant/lib/message-animations"
import { MESSAGE_ANIMATIONS } from "@slab/ui/pages/assistant/lib/message-animations"
import AgentAvatar from "@slab/ui/pages/assistant/components/agent-avatar"
import UserAvatar from "@slab/ui/pages/assistant/components/user-avatar"
import { Message, MessageContent, MessageHeader, MessageAvatar, MessageFooter } from "@slab/components/message"
import { MessageScrollerItem } from "@slab/components/message-scroller"
import { useTranslation } from "@slab/i18n"
import { useClipboard } from "@mantine/hooks"
import { CheckIcon, CopyIcon, Undo2Icon } from "lucide-react"
import { cn } from "@slab/ui/lib/utils"
import { useMessageInteraction } from "@slab/ui/pages/assistant/components/message-interaction-context"
import { createMessageParts, MessageParts } from "./message-parts"
import type { MessagePartComponents, MessagePartItem, MessagePartsResult } from "./message-parts"
import { Bubble, BubbleContent } from "@slab/components/bubble"

import MessageTextPart from "./message-text-part"
import MessageReasoningPart from "./message-reasoning-part"
import MessageFallbackPart from "./message-fallback-part"
import MessageToolPart from "./message-tool-part"
import MessageToolCommandPart from "./message-tool-command-part"
import MessageToolFileChangePart from "./message-tool-file-change-part"
import MessageToolFileGlobPart from "./message-tool-file-glob-part"
import MessageToolGrepPart from "./message-tool-grep-part"
import MessageToolListDirPart from "./message-tool-list-dir-part"
import MessageToolPlanPart from "./message-tool-plan-part"
import MessageToolReadFilePart from "./message-tool-read-file-part"
import MessageToolSubagentPart from "./message-tool-subagent-part"

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

/**
 * Memoized on shallow props (message identity above all): streaming replaces
 * the streaming message's object, so only that row re-renders per chunk; the
 * rest of a long conversation skips re-rendering entirely.
 */
const MessageItem = React.memo(function MessageItem({
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
})

const messagePartComponents: MessagePartComponents<TMessagePart, TMessage> = {
    text: MessageTextPart,
    reasoning: MessageReasoningPart,
    // Default for every tool call without a dedicated entry in `tools` below
    // (unknown tools, MCP calls, not-yet-covered built-ins): compact row +
    // generic Parameters/Result JSON cards.
    tool: MessageToolPart,
    fallback: MessageFallbackPart,
    // Keyed by the part's toolName as it rides the wire (`turn-items.ts`
    // `toolItemFields`): camelCase synthetic names for the TurnItem-specific
    // variants, raw snake_case tool names for generic built-in tool calls.
    tools: {
        commandExecution: MessageToolCommandPart,
        fileChange: MessageToolFileChangePart,
        plan: MessageToolPlanPart,
        read_file: MessageToolReadFilePart,
        list_dir: MessageToolListDirPart,
        file_glob: MessageToolFileGlobPart,
        grep: MessageToolGrepPart,
        delegate_subagent: MessageToolSubagentPart,
    },
}

/**
 * `createMessageParts` cache keyed by message identity: parts are rebuilt on
 * every MessageRow render otherwise, which re-classifies / re-groups every
 * part of every visible row on unrelated re-renders. Streaming replaces the
 * streaming message's object, so its cache entry rebuilds per chunk while
 * stable messages hit the cache. Entries die with their message objects.
 */
const messagePartsCache = new WeakMap<object, unknown>()

function memoizedCreateMessageParts<TMessage extends object>(message: TMessage): unknown {
    let cached = messagePartsCache.get(message)
    if (cached === undefined) {
        cached = createMessageParts<TMessage>(message)
        messagePartsCache.set(message, cached)
    }
    return cached
}


function MessageRow({
    message,
}: {
    message: TMessage
}) {
    const isUserMessage = message.role === "user"
    // `createMessageParts` defaults to protocol (temporal) order — a tool call
    // renders where it actually occurred relative to the text/reasoning.
    // Memoized by message identity (see `messagePartsCache`).
    const parsedParts = memoizedCreateMessageParts<TMessage>(message) as MessagePartsResult<
        TRenderableMessagePart,
        TMessage
    > & { all: Array<MessagePartItem<TRenderableMessagePart, TMessage>> }
    const { t } = useTranslation()
    const clipboard = useClipboard({ timeout: 2000 })
    const { userMessageTurnIndex, rollbackToMessage } = useMessageInteraction()
    const plainText = (message.parts ?? [])
        .filter((part) => (part as TMessagePart).type === "text")
        .map((part) => (part as TMessagePart).text ?? "")
        .join("")
        .trim()
    // Rollback is offered on user messages (except the first turn) when the host
    // wired the action. Retracting removes that message and everything after it.
    const canRollback =
        isUserMessage &&
        rollbackToMessage !== undefined &&
        (userMessageTurnIndex.get(message.id) ?? 0) > 0
    const rollbackLabel = t("pages.assistant.message.rollback")
    return (
        <Message align={isUserMessage ? "end" : "start"} data-testid={`assistant-message-${message.role}`}>
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
                {plainText || canRollback ? (
                    <MessageFooter>
                        {plainText ? (
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
                        ) : null}
                        {canRollback ? (
                            <Button
                                variant="ghost"
                                size="icon"
                                aria-label={rollbackLabel}
                                title={rollbackLabel}
                                data-testid="assistant-message-rollback"
                                onClick={() => rollbackToMessage?.(message.id)}
                            >
                                <Undo2Icon />
                            </Button>
                        ) : null}
                    </MessageFooter>
                ) : null}
            </MessageContent>
        </Message>
    )
}


export { MessageItem, type TMessage, type TMessagePart }
