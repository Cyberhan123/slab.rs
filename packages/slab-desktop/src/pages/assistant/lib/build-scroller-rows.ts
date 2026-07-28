import type { CompactionMarker } from "@/pages/assistant/hooks/use-harness-conversation"
import type { TMessage } from "@/pages/assistant/components/message/message-item"

export const HISTORY_MARKER_ID = "__history_marker__" as const

/**
 * A virtualized scroller row. Either a real message, or a synthetic non-message
 * row (the "history restored" separator or a compaction divider). The list
 * dispatches each kind through the `rowComponents` registry; meta-rows live in
 * the SAME positioned container as messages (no nested `MessageScrollerItem`,
 * whose content-visibility rules fight the virtualizer).
 */
export type ScrollerRow =
    | { kind: "historyMarker"; id: typeof HISTORY_MARKER_ID }
    | { kind: "compactMarker"; id: string; marker: CompactionMarker }
    | { kind: "message"; id: string; message: TMessage }

/** Narrow a `ScrollerRow` to a single variant by its discriminant `kind`. */
export type ScrollerRowOf<K extends ScrollerRow["kind"]> = Extract<ScrollerRow, { kind: K }>

export type BuildScrollerRowsOptions = {
    /** When true, render a "history restored" separator at the bottom of history. */
    showHistoryMarker: boolean
    /** Number of restored (pre-live) messages; the marker renders after these. Defaults to all. */
    historyCount?: number
}

/**
 * Build the virtualizer row list from messages + the synthetic non-message
 * markers. The history marker sits at the BOTTOM of the restored slice (after
 * the restored messages, before any new live messages from the current turn);
 * compaction markers render at the END of the stream.
 */
export function buildScrollerRows(
    messages: ReadonlyArray<TMessage>,
    compactionMarkers: ReadonlyArray<CompactionMarker>,
    options: BuildScrollerRowsOptions,
): ScrollerRow[] {
    const messageRows: ScrollerRow[] = messages.map((message) => ({
        kind: "message",
        id: message.id,
        message,
    }))
    const restoredCount = options.historyCount ?? messages.length
    const out: ScrollerRow[] = messageRows.slice(0, restoredCount)
    if (options.showHistoryMarker && restoredCount > 0) {
        out.push({ kind: "historyMarker", id: HISTORY_MARKER_ID })
    }
    out.push(...messageRows.slice(restoredCount))
    for (const marker of compactionMarkers) {
        out.push({ kind: "compactMarker", id: marker.id, marker })
    }
    return out
}

/** Stable `YYYY-MM-DD` label (zero-padded) for the restore separator. */
export function formatMarkerDate(date: Date): string {
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    return `${year}-${month}-${day}`
}
