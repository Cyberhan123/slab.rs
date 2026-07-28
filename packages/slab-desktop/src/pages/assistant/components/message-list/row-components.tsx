import type { ComponentType, ReactElement } from "react"
import { useTranslation } from "@slab/i18n"
import { Marker, MarkerContent } from "@slab/components/marker"
import { MessageItem } from "@/pages/assistant/components/message/message-item"
import { Shimmer } from "@/pages/assistant/components/message/shimmer"
import {
    formatMarkerDate,
    type ScrollerRow,
    type ScrollerRowOf,
} from "@/pages/assistant/lib/build-scroller-rows"

/**
 * Extras the list parent passes to every row. Only `historyCreatedAt` is
 * consumed (by the history marker); carrying it in a shared bag keeps the
 * registry dispatch uniform — `const C = rowComponents[row.kind]; <C ... />`.
 */
export type ScrollerRowExtraProps = {
    /** `thread.createdAt` (Unix ms) for the marker label; null = today. */
    historyCreatedAt: number | null
}

export type ScrollerRowComponentProps<R extends ScrollerRow> = {
    row: R
} & ScrollerRowExtraProps

export function HistoryMarkerRow({
    historyCreatedAt,
}: ScrollerRowComponentProps<ScrollerRowOf<"historyMarker">>): ReactElement {
    const { t } = useTranslation()
    const label = formatMarkerDate(
        historyCreatedAt != null ? new Date(historyCreatedAt) : new Date(),
    )
    return (
        <Marker variant="separator" data-testid="assistant-history-marker">
            <MarkerContent>
                {label} {t("pages.assistant.history.restored")}
            </MarkerContent>
        </Marker>
    )
}

export function CompactMarkerRow({
    row,
}: ScrollerRowComponentProps<ScrollerRowOf<"compactMarker">>): ReactElement {
    const { t } = useTranslation()
    const { marker } = row
    return (
        <Marker variant="separator" data-testid={`assistant-compact-marker-${marker.id}`}>
            <MarkerContent>
                {marker.phase === "compacting" ? (
                    <Shimmer>
                        {marker.mode === "auto"
                            ? t("pages.assistant.compaction.autoCompacting")
                            : t("pages.assistant.compaction.manuallyCompacting")}
                    </Shimmer>
                ) : marker.mode === "auto" ? (
                    t("pages.assistant.compaction.autoCompacted")
                ) : (
                    t("pages.assistant.compaction.manuallyCompacted")
                )}
            </MarkerContent>
        </Marker>
    )
}

export function MessageRow({
    row,
}: ScrollerRowComponentProps<ScrollerRowOf<"message">>): ReactElement {
    return (
        <MessageItem message={row.message} scrollAnchor={row.message.role === "user"} />
    )
}

/**
 * Row-level component registry, mirroring the MessageParts dispatch pattern.
 * Replaces the former `row.kind === ...` ternary chain in the list. Meta-rows
 * (history/compact markers) live in the SAME positioned container as messages
 * and bypass `<MessageScrollerItem>` — its content-visibility rules fight the
 * virtualizer — so they render straight into a bare `<Marker>`.
 *
 * Indexed access `rowComponents[row.kind]` cannot propagate `row.kind` narrowing
 * (a known TS limitation for heterogeneous registries), so the list casts the
 * resolved component at the dispatch site — the same pattern `message-parts.tsx`
 * uses with `React.createElement`. Each component re-narrows on its own param
 * type, so safety moves from "narrowed in the list" to "narrowed by signature".
 */
export const rowComponents: {
    historyMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"historyMarker">>>
    compactMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"compactMarker">>>
    message: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"message">>>
} = {
    historyMarker: HistoryMarkerRow,
    compactMarker: CompactMarkerRow,
    message: MessageRow,
}
