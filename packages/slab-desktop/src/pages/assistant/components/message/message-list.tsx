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
import type { CompactionMarker } from "@/pages/assistant/hooks/use-harness-conversation"
import type { TMessage } from "./message-item"
import { MessageItem } from "./message-item"
import { Shimmer } from "./shimmer"

type MessageListProps = {
    messages: TMessage[]
    isBusy: boolean
    /** When true, render a "history restored" separator at the bottom of history. */
    showHistoryMarker?: boolean
    /** Number of restored (pre-live) messages; the marker renders after these. Defaults to all. */
    historyCount?: number
    /** `thread.createdAt` (Unix ms) for the marker label; falls back to today. */
    historyCreatedAt?: number | null
    /** Session-scoped compaction markers rendered at the end of the stream. */
    compactionMarkers?: CompactionMarker[]
}

/**
 * A virtualized scroller row. Either a real message, or a synthetic non-message
 * row (the "history restored" separator or a compaction divider). The list
 * renders each kind by branch; meta-rows live in the SAME positioned container
 * as messages (no nested `MessageScrollerItem`, whose content-visibility rules
 * fight the virtualizer).
 */
type ScrollerRow =
    | { kind: "historyMarker"; id: "__history_marker__" }
    | { kind: "compactMarker"; id: string; marker: CompactionMarker }
    | { kind: "message"; id: string; message: TMessage }

const HISTORY_MARKER_ID = "__history_marker__"

/** Stable `YYYY-MM-DD` label for the restore separator (today's date). */
function formatMarkerDate(date: Date): string {
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
}

function MessageList({
    messages,
    isBusy,
    showHistoryMarker = false,
    historyCount,
    historyCreatedAt,
    compactionMarkers,
}: MessageListProps) {
    const { t } = useTranslation()
    const viewportRef = useRef<HTMLDivElement>(null)

    const rows = useMemo<ScrollerRow[]>(() => {
        const messageRows: ScrollerRow[] = messages.map((message) => ({
            kind: "message",
            id: message.id,
            message,
        }))
        // The history marker sits at the BOTTOM of the restored history: after
        // the restored slice, before any new live messages from this turn.
        const restoredCount = historyCount ?? messages.length
        const out: ScrollerRow[] = messageRows.slice(0, restoredCount)
        if (showHistoryMarker && restoredCount > 0) {
            out.push({ kind: "historyMarker", id: HISTORY_MARKER_ID })
        }
        out.push(...messageRows.slice(restoredCount))
        // Compaction markers render at the END of the stream.
        for (const marker of compactionMarkers ?? []) {
            out.push({ kind: "compactMarker", id: marker.id, marker })
        }
        return out
    }, [messages, showHistoryMarker, historyCount, compactionMarkers])

    const markerLabel = useMemo(
        () => formatMarkerDate(historyCreatedAt != null ? new Date(historyCreatedAt) : new Date()),
        [historyCreatedAt],
    )

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
                                ) : row.kind === "compactMarker" ? (
                                    <Marker
                                        variant="separator"
                                        data-testid={`assistant-compact-marker-${row.marker.id}`}
                                    >
                                        <MarkerContent>
                                            {row.marker.phase === "compacting" ? (
                                                <Shimmer>
                                                    {row.marker.mode === "auto"
                                                        ? t(
                                                              "pages.assistant.compaction.autoCompacting",
                                                          )
                                                        : t(
                                                              "pages.assistant.compaction.manuallyCompacting",
                                                          )}
                                                </Shimmer>
                                            ) : (
                                                (row.marker.mode === "auto"
                                                    ? t("pages.assistant.compaction.autoCompacted")
                                                    : t(
                                                          "pages.assistant.compaction.manuallyCompacted",
                                                      ))
                                            )}
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
