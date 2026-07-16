import { useTranslation } from "@slab/i18n"
import { Spinner } from "@slab/components/spinner"
import type { ModelLoadState } from "../../hooks/use-harness-conversation"

/**
 * Transient model-load indicator driven by `model/load/delta` +
 * `model/load/completed` harness notifications. Rendered OUTSIDE the message
 * virtualizer (as a sibling near the composer) so per-delta progress updates
 * don't perturb virtual item heights or scroll. Hidden when `modelLoad` is null.
 */
export function ModelLoadIndicator({ modelLoad }: { modelLoad: ModelLoadState }) {
    const { t } = useTranslation()
    if (!modelLoad) return null

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
        <div
            className="flex items-center gap-2 px-1 text-xs text-muted-foreground"
            data-testid="assistant-model-load-indicator"
            data-phase={modelLoad.phase}
        >
            <Spinner className="size-3.5" />
            <span className="truncate">{label}</span>
            {percent != null && <span className="tabular-nums">{percent}%</span>}
        </div>
    )
}
