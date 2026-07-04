import { useRef } from "react"
import {
    MessageScroller,
    MessageScrollerButton,
    MessageScrollerContent,
    MessageScrollerViewport,
} from "@slab/components/message-scroller"
import { useVirtualizer } from "@tanstack/react-virtual"
import type { TMessage } from "./message-item"
import { MessageItem } from "./message-item"

type MessageListProps = {
    messages: TMessage[]
    isBusy: boolean
}


function MessageList({ messages, isBusy }: MessageListProps) {
    const viewportRef = useRef<HTMLDivElement>(null)
    const virtualizer = useVirtualizer({
        count: messages.length,
        getScrollElement: () => viewportRef.current,
        estimateSize: () => 86,
        getItemKey: (index) => messages[index]?.id ?? index,
        overscan: 8,
    })

    return <MessageScroller>
        <MessageScrollerViewport ref={viewportRef}>
            <MessageScrollerContent
                aria-busy={isBusy}
                className="p-(--card-spacing)"
            >
                <div
                    className="relative w-full"
                    style={{ height: virtualizer.getTotalSize() }}
                >
                    {virtualizer.getVirtualItems().map((virtualItem) => {
                        const message = messages[virtualItem.index]

                        if (!message) {
                            return null
                        }

                        return (
                            <div
                                key={virtualItem.key}
                                ref={virtualizer.measureElement}
                                data-index={virtualItem.index}
                                className="absolute start-0 top-0 w-full pb-4"
                                style={{
                                    transform: `translateY(${virtualItem.start + 14}px)`,
                                }}
                            >
                                <MessageItem
                                    key={message.id}
                                    message={message}
                                    scrollAnchor={message.role === "user"}
                                />
                            </div>
                        )
                    })}
                </div>
            </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
    </MessageScroller>
}

export default MessageList
export type { MessageListProps }