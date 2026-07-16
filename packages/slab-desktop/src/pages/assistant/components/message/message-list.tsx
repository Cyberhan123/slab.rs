import { useMemo, useRef } from "react"
import { useTranslation } from "@slab/i18n"
import { Marker, MarkerContent } from "@slab/components/marker"
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
    /** When true, prepend a "history restored" separator above the messages. */
    showHistoryMarker?: boolean
}

/**
 * A virtualized scroller row. Either a real message, or a synthetic non-message
 * row (today: the "history restored" separator). The list renders each kind by
 * branch; meta-rows live in the SAME positioned container as messages (no nested
 * `MessageScrollerItem`, whose content-visibility rules fight the virtualizer).
 */
type ScrollerRow =
    | { kind: "historyMarker"; id: "__history_marker__" }
    | { kind: "message"; id: string; message: TMessage }

const HISTORY_MARKER_ID = "__history_marker__"

/** Stable `YYYY-MM-DD` label for the restore separator (today's date). */
function formatMarkerDate(date: Date): string {
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
}

function MessageList({ messages, isBusy, showHistoryMarker = false }: MessageListProps) {
    const { t } = useTranslation()
    const viewportRef = useRef<HTMLDivElement>(null)

    const rows = useMemo<ScrollerRow[]>(() => {
        const messageRows: ScrollerRow[] = messages.map((message) => ({
            kind: "message",
            id: message.id,
            message,
        }))
        // Only mark restored history — never a fresh/empty session.
        if (showHistoryMarker && messageRows.length > 0) {
            return [{ kind: "historyMarker", id: HISTORY_MARKER_ID }, ...messageRows]
        }
        return messageRows
    }, [messages, showHistoryMarker])

    const markerLabel = useMemo(() => formatMarkerDate(new Date()), [])

    const virtualizer = useVirtualizer({
        count: rows.length,
        getScrollElement: () => viewportRef.current,
        estimateSize: () => 86,
        getItemKey: (index) => rows[index]?.id ?? index,
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
                        const row = rows[virtualItem.index]

                        if (!row) {
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
                                {row.kind === "historyMarker" ? (
                                    <Marker
                                        variant="separator"
                                        data-testid="assistant-history-marker"
                                    >
                                        <MarkerContent>
                                            {markerLabel} {t("pages.assistant.history.restored")}
                                        </MarkerContent>
                                    </Marker>
                                ) : (
                                    <MessageItem
                                        message={row.message}
                                        scrollAnchor={row.message.role === "user"}
                                    />
                                )}
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
