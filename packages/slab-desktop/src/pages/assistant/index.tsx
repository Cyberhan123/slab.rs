"use client"

import { useChat } from "@ai-sdk/react"
import {
    MessageCircleDashedIcon,
} from "lucide-react"
import {
    useGreeting,
} from "./hooks/use-greeting"
import { createChat } from "@/pages/assistant/lib/message-provider"
import { useTranslation } from "@slab/i18n"
import {
    Card,
    CardContent,
    CardFooter,
} from "@slab/components/card"

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

const chat = createChat()
    .user(
        "I'm building a chat for our app and the scroll behavior is driving me nuts. Every time the AI streams a reply, the whole thread jumps around."
    )
    .sleep(1000)
    .assistant(
        "That's the classic streaming scroll problem. Wrap your message list in `MessageScroller` and turn on `autoScroll` — the viewport pins to the bottom as tokens arrive, so users always see the latest text land in place.\n\nThe important part: it only auto-scrolls while the reader is already at the bottom. The moment they scroll up to read something earlier, auto-scroll backs off and their position is preserved. You get smooth streaming without fighting the user's intent."
    )
    .user(
        "Okay, but when someone sends a new message the view still feels jarring — like the whole conversation reloads from the top."
    )
    .sleep(1000)
    .assistant(
        "MessageScrollerItem fixes that with turn anchoring. Set `scrollAnchor` on the turn that should settle near the top instead of blindly snapping to the document bottom.\n\nIt also leaves a small peek of the previous exchange visible above the anchor, so context isn't lost. The reply starts in view without that disorienting jump you get from a plain overflow container."
    )
    .user(
        "And if they've scrolled up to re-read an older answer? I don't want to yank them back down."
    )
    .sleep(1000)
    .assistant(
        "You won't. Auto-scroll only runs when the viewport is already pinned to the bottom, so scrolling up is a deliberate opt-out — their place in the thread stays put even as new tokens keep arriving below.\n\nWhen there is content they haven't seen yet, `MessageScrollerButton` appears at the bottom of the viewport. One tap jumps them back to the newest message and re-engages auto-scroll. Same pattern as Slack or iMessage: quiet when you're caught up, helpful when you're not."
    )
    .user("Last one — does this work with assistive tech?")
    .sleep(1000)
    .assistant(
        '`MessageScrollerContent` sets `role="log"` and `aria-relevant="additions"` by default, so screen readers announce new messages as they stream in.\n\nThe scroll button is a real `<button>` with an sr-only label, and it\'s removed from the tab order when you\'re already at the bottom — no ghost focus stops.'
    )
const initialMessages = chat.get({ count: 0 })
const transport = chat.transport({ chunkDelayMs: 20 })

function Assistant() {

    const { t } = useTranslation()
    const { messages, sendMessage, status } = useChat({
        messages: initialMessages,
        transport,
    })

    const isBusy = status === "submitted" || status === "streaming"

    const greeting = useGreeting()

    return (
        <MessageScrollerProvider>
            <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
                <Card className="h-full w-full gap-0 border-none shadow-none">
                    <CardContent className="flex-1 overflow-hidden p-0">
                        {messages.length === 0 ? (
                            <Empty className="h-full">
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
                            onSubmit={(value) => {
                                sendMessage({
                                    text: value,
                                })
                            }}
                            loading={isBusy}
                        />

                    </CardFooter>
                </Card>
            </div>
        </MessageScrollerProvider>
    )
}

export default Assistant
