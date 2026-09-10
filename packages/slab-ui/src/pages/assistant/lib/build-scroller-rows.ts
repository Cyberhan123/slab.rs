import type {
    BackgroundTaskInfo,
    CompactionMarker,
    LiveTextEntry,
    ModelLoadState,
    SettingsMarker,
} from "@slab/core/harness"
import type { TMessage } from "@slab/ui/pages/assistant/components/message/message-item"

export const HISTORY_MARKER_ID = "__history_marker__" as const
export const SESSION_LOAD_MARKER_ID = "__session_load_marker__" as const
export const MODEL_LOAD_MARKER_ID = "__model_load_marker__" as const
export const QUEUED_INPUT_ID_PREFIX = "__queued_input_" as const
export const BACKGROUND_TASK_ID_PREFIX = "__background_task_" as const
export const LIVE_TEXT_ID_PREFIX = "__live_text_" as const

/**
 * A virtualized scroller row. Either a real message, or a synthetic non-message
 * status row (session/history/model-load/compaction/settings markers) rendered
 * through the `rowComponents` registry. Meta-rows live in the SAME positioned
 * container as messages (no nested `MessageScrollerItem`, whose
 * content-visibility rules fight the virtualizer) and all share the
 * `<Marker variant="separator">` form so system status reads as one ordered
 * timeline instead of scattered regions.
 */
export type ScrollerRow =
    | { kind: "sessionLoadMarker"; id: typeof SESSION_LOAD_MARKER_ID }
    | { kind: "historyMarker"; id: typeof HISTORY_MARKER_ID }
    | { kind: "compactMarker"; id: string; marker: CompactionMarker }
    | { kind: "settingsMarker"; id: string; marker: SettingsMarker }
    | { kind: "modelLoadMarker"; id: typeof MODEL_LOAD_MARKER_ID; modelLoad: NonNullable<ModelLoadState> }
    | { kind: "queuedInput"; id: string; text: string }
    | { kind: "backgroundTask"; id: string; task: BackgroundTaskInfo }
    | { kind: "liveText"; id: string; entry: LiveTextEntry }
    | { kind: "message"; id: string; message: TMessage }

/** Narrow a `ScrollerRow` to a single variant by its discriminant `kind`. */
export type ScrollerRowOf<K extends ScrollerRow["kind"]> = Extract<ScrollerRow, { kind: K }>

export type BuildScrollerRowsOptions = {
    /** When true, render a "history restored" separator at the bottom of history. */
    showHistoryMarker: boolean
    /** Number of restored (pre-live) messages; the marker renders after these. Defaults to all. */
    historyCount?: number
    /** Transient model-load state; rendered as a Marker at the live edge. */
    modelLoad?: ModelLoadState | null
    /** Session-scoped settings-change markers (model switch / permission mode). */
    settingsMarkers?: readonly SettingsMarker[]
    /** True while restoring; renders a session-load Marker when there are no messages yet. */
    sessionLoading?: boolean
    /** Steering inputs queued on the running turn; rendered as ghost user bubbles at the tail. */
    queuedTexts?: readonly string[]
    /** Resident background tasks (shell background=true); RUNNING tasks render a status Marker at the tail. */
    backgroundTasks?: readonly BackgroundTaskInfo[]
    /**
     * Controller-mirrored in-flight assistant text (remount-orphaned runs /
     * mid-turn reloads); rendered as assistant-aligned tail bubbles.
     */
    liveTailTexts?: readonly LiveTextEntry[]
}

/**
 * Build the virtualizer row list from messages + the synthetic status markers.
 * Ordering: the session-load marker leads (only while restoring with no
 * messages); messages follow with the history-restored marker between the
 * restored slice and live messages; completed compaction markers sit at the
 * live edge in arrival order; anchored settings-change markers splice in right
 * after the message they were recorded at (unanchored ones follow the
 * compaction markers); the transient model-load marker (the current activity)
 * trails them.
 */
export function buildScrollerRows(
    messages: ReadonlyArray<TMessage>,
    compactionMarkers: ReadonlyArray<CompactionMarker>,
    options: BuildScrollerRowsOptions,
): ScrollerRow[] {
    const out: ScrollerRow[] = []

    if (options.sessionLoading && messages.length === 0) {
        out.push({ kind: "sessionLoadMarker", id: SESSION_LOAD_MARKER_ID })
    }

    const messageRows: ScrollerRow[] = messages.map((message) => ({
        kind: "message",
        id: message.id,
        message,
    }))
    const restoredCount = options.historyCount ?? messages.length
    out.push(...messageRows.slice(0, restoredCount))
    if (options.showHistoryMarker && restoredCount > 0) {
        out.push({ kind: "historyMarker", id: HISTORY_MARKER_ID })
    }
    out.push(...messageRows.slice(restoredCount))

    for (const marker of compactionMarkers) {
        out.push({ kind: "compactMarker", id: marker.id, marker })
    }
    // Settings markers split into anchored (inserted right after the message
    // they were recorded at, keeping the timeline order of mid-conversation
    // switches) and unanchored/legacy (tail position, after compaction).
    const tailSettingsMarkers: SettingsMarker[] = []
    for (const marker of options.settingsMarkers ?? []) {
        const anchor = marker.afterMessageId
        let insertAfter = -1
        if (anchor) {
            // LAST match: repeated message ids anchor to the final occurrence.
            for (let index = out.length - 1; index >= 0; index -= 1) {
                const row = out[index]
                if (row.kind === "message" && row.message.id === anchor) {
                    insertAfter = index
                    break
                }
            }
        }
        if (insertAfter === -1) {
            tailSettingsMarkers.push(marker)
            continue
        }
        // Markers already anchored to this message pile up contiguously after
        // it; insert at the END of that run so same-anchor markers keep
        // arrival order.
        while (
            insertAfter + 1 < out.length &&
            out[insertAfter + 1].kind === "settingsMarker"
        ) {
            insertAfter += 1
        }
        out.splice(insertAfter + 1, 0, { kind: "settingsMarker", id: marker.id, marker })
    }
    for (const marker of tailSettingsMarkers) {
        out.push({ kind: "settingsMarker", id: marker.id, marker })
    }
    if (options.modelLoad) {
        out.push({ kind: "modelLoadMarker", id: MODEL_LOAD_MARKER_ID, modelLoad: options.modelLoad })
    }
    for (const task of options.backgroundTasks ?? []) {
        if (task.status !== "running") continue
        out.push({
            kind: "backgroundTask",
            id: `${BACKGROUND_TASK_ID_PREFIX}${task.taskId}`,
            task,
        })
    }
    for (const [index, text] of (options.queuedTexts ?? []).entries()) {
        out.push({ kind: "queuedInput", id: `${QUEUED_INPUT_ID_PREFIX}${index}`, text })
    }
    for (const entry of options.liveTailTexts ?? []) {
        out.push({ kind: "liveText", id: `${LIVE_TEXT_ID_PREFIX}${entry.itemId}`, entry })
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
