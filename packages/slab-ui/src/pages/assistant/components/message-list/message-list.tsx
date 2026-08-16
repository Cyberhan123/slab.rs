import { useMemo, useRef } from "react"
import type { ComponentType } from "react"
import {
    MessageScroller,
    MessageScrollerButton,
    MessageScrollerContent,
    MessageScrollerViewport,
} from "@slab/components/message-scroller"
import { useVirtualizer } from "@tanstack/react-virtual"
import type { CompactionMarker, ModelLoadState } from "@slab/core/harness"
import { buildScrollerRows, type ScrollerRow } from "@slab/ui/pages/assistant/lib/build-scroller-rows"
import type { TMessage } from "@slab/ui/pages/assistant/components/message/message-item"
import { rowComponents, type ScrollerRowExtraProps } from "./row-components"

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
    /** Transient model-load state; rendered as a Marker at the live edge. */
    modelLoad?: ModelLoadState | null
    /** True while restoring; renders a session-load Marker when there are no messages yet. */
    sessionLoading?: boolean
}

function MessageList({
    messages,
    isBusy,
    showHistoryMarker = false,
    historyCount,
    historyCreatedAt,
    compactionMarkers,
    modelLoad,
    sessionLoading = false,
}: MessageListProps) {
    const viewportRef = useRef<HTMLDivElement>(null)

    const rows = useMemo<ScrollerRow[]>(
        () =>
            buildScrollerRows(messages, compactionMarkers ?? [], {
                showHistoryMarker,
                historyCount,
                modelLoad,
                sessionLoading,
            }),
        [messages, showHistoryMarker, historyCount, compactionMarkers, modelLoad, sessionLoading],
    )

    const virtualizer = useVirtualizer({
        count: rows.length,
        getScrollElement: () => viewportRef.current,
        estimateSize: () => 86,
        getItemKey: (index) => rows[index]?.id ?? index,
        overscan: 8,
    })

    return (
        <MessageScroller>
            <MessageScrollerViewport ref={viewportRef}>
                <MessageScrollerContent aria-busy={isBusy} className="p-(--card-spacing)">
                    <div
                        className="relative w-full"
                        style={{ height: virtualizer.getTotalSize() }}
                    >
                        {virtualizer.getVirtualItems().map((virtualItem) => {
                            const row = rows[virtualItem.index]

                            if (!row) {
                                return null
                            }

                            // Meta-rows share the SAME positioned bare `<div>` as
                            // messages; only the inner body differs, dispatched via
                            // the `rowComponents` registry (see row-components.tsx).
                            const RowCmp = rowComponents[row.kind] as ComponentType<
                                { row: ScrollerRow } & ScrollerRowExtraProps
                            >

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
                                    <RowCmp
                                        row={row}
                                        historyCreatedAt={historyCreatedAt ?? null}
                                    />
                                </div>
                            )
                        })}
                    </div>
                </MessageScrollerContent>
            </MessageScrollerViewport>
            <MessageScrollerButton />
        </MessageScroller>
    )
}

export default MessageList
export type { MessageListProps }
