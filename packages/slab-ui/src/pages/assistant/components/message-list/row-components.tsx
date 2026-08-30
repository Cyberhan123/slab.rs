import { memo } from "react"
import type { ComponentType, ReactElement } from "react"
import { useTranslation } from "@slab/i18n"
import { Marker, MarkerContent } from "@slab/components/marker"
import { Message, MessageAvatar, MessageContent, MessageHeader } from "@slab/components/message"
import { Bubble, BubbleContent } from "@slab/components/bubble"
import UserAvatar from "@slab/ui/pages/assistant/components/user-avatar"
import { MessageItem } from "@slab/ui/pages/assistant/components/message/message-item"
import { Shimmer } from "@slab/ui/pages/assistant/components/message/shimmer"
import {
    formatMarkerDate,
    type ScrollerRow,
    type ScrollerRowOf,
} from "@slab/ui/pages/assistant/lib/build-scroller-rows"

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

/**
 * Memoized on the row's MESSAGE identity: the scroller rebuilds its row
 * objects on every list change (queued chip, marker, model-load state), but
 * an unchanged message must not re-render — long conversations otherwise
 * re-render every visible row per unrelated state change. Streaming updates
 * replace the streaming message's object (AI-SDK immutable state), so only
 * the actively-streaming row re-renders per chunk.
 */
export const MessageRow = memo(
    function MessageRow({
        row,
    }: ScrollerRowComponentProps<ScrollerRowOf<"message">>): ReactElement {
        return <MessageItem message={row.message} scrollAnchor={row.message.role === "user"} />
    },
    (prev, next) => prev.row.message === next.row.message,
)

/** "Loading this session…" separator shown while restoring, before any messages exist. */
export function SessionLoadMarkerRow(
    _props: ScrollerRowComponentProps<ScrollerRowOf<"sessionLoadMarker">>,
): ReactElement {
    const { t } = useTranslation()
    return (
        <Marker variant="separator" data-testid="assistant-session-load-marker">
            <MarkerContent>
                <Shimmer>{`${t("pages.assistant.loading.title")} — ${t("pages.assistant.loading.description")}`}</Shimmer>
            </MarkerContent>
        </Marker>
    )
}

/** "Downloading/loading model…" separator that tracks the live model-load phase. */
export function ModelLoadMarkerRow({
    row,
}: ScrollerRowComponentProps<ScrollerRowOf<"modelLoadMarker">>): ReactElement {
    const { t } = useTranslation()
    const { modelLoad } = row
    const label =
        modelLoad.phase === "downloading"
            ? t("pages.assistant.modelLoad.downloading")
            : t("pages.assistant.modelLoad.loading")
    const percent =
        modelLoad.downloadedBytes != null &&
        modelLoad.totalBytes != null &&
        modelLoad.totalBytes > 0
            ? Math.min(100, Math.round((modelLoad.downloadedBytes / modelLoad.totalBytes) * 100))
            : null
    return (
        <Marker variant="separator" data-testid="assistant-model-load-marker">
            <MarkerContent>
                <Shimmer>{label}</Shimmer>
                {percent != null ? <span className="tabular-nums">{percent}%</span> : null}
            </MarkerContent>
        </Marker>
    )
}

/**
 * Ghost user bubble for a queued steering input: rendered at the tail while a
 * turn runs, replaced by the real (rollout-backed) message row after the run
 * ends and the controller resyncs. Mirrors the user-message row structure
 * (avatar + header + tinted bubble), dimmed and labelled "Queued".
 */
export function QueuedInputRow({
    row,
}: ScrollerRowComponentProps<ScrollerRowOf<"queuedInput">>): ReactElement {
    const { t } = useTranslation()
    return (
        <Message align="end" data-testid="assistant-queued-input">
            <MessageAvatar>
                <UserAvatar name={t("pages.assistant.message.user")} />
            </MessageAvatar>
            <MessageContent>
                <MessageHeader>{t("pages.assistant.message.queuedLabel")}</MessageHeader>
                <Bubble align="end" variant="tinted" className="opacity-70">
                    <BubbleContent>
                        <p className="whitespace-pre-wrap break-words">{row.text}</p>
                    </BubbleContent>
                </Bubble>
            </MessageContent>
        </Message>
    )
}

/**
 * Status Marker for a RUNNING resident background task (shell
 * background=true). Non-localized by convention (the compact tool-row labels
 * `Read:`/`Bash:` are too) — no i18n key surface for v1.
 */
export function BackgroundTaskRow({
    row,
}: ScrollerRowComponentProps<ScrollerRowOf<"backgroundTask">>): ReactElement {
    const { task } = row
    const label = task.command?.trim() || task.taskId
    return (
        <Marker variant="separator" data-testid={`assistant-background-task-${task.taskId}`}>
            <MarkerContent>
                <span className="inline-flex items-center gap-1.5">
                    <span aria-hidden className="text-primary">▶</span>
                    <span className="font-mono text-xs">{label}</span>
                    <span className="text-muted-foreground text-xs">
                        (background · {task.taskId}
                        {task.pid != null ? ` · pid ${task.pid}` : ""})
                    </span>
                </span>
            </MarkerContent>
        </Marker>
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
    sessionLoadMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"sessionLoadMarker">>>
    historyMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"historyMarker">>>
    compactMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"compactMarker">>>
    modelLoadMarker: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"modelLoadMarker">>>
    queuedInput: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"queuedInput">>>
    backgroundTask: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"backgroundTask">>>
    message: ComponentType<ScrollerRowComponentProps<ScrollerRowOf<"message">>>
} = {
    sessionLoadMarker: SessionLoadMarkerRow,
    historyMarker: HistoryMarkerRow,
    compactMarker: CompactMarkerRow,
    modelLoadMarker: ModelLoadMarkerRow,
    queuedInput: QueuedInputRow,
    backgroundTask: BackgroundTaskRow,
    message: MessageRow,
}
