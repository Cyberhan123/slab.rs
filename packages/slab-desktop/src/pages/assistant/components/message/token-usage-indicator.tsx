import { useTranslation } from "@slab/i18n"

import type { TurnUsage } from "../../lib/harness"

/**
 * Token-usage indicator driven by the `turn/completed` harness notification's
 * `usage` payload. Shows prompt / completion (and cached) token counts plus a
 * context-window consumption bar when a context window is known. Rendered as a
 * sibling of {@link ModelLoadIndicator} in the composer footer. Hidden when
 * `usage` is null (no turn has completed yet).
 */
export function TokenUsageIndicator({
    usage,
    contextWindow,
}: {
    usage: TurnUsage | null
    contextWindow: number | null
}) {
    const { t } = useTranslation()
    if (!usage) return null

    const prompt = usage.promptTokens ?? 0
    const completion = usage.completionTokens ?? 0
    const cached = usage.cachedTokens ?? 0
    const formatter = new Intl.NumberFormat()
    const ratio =
        contextWindow && contextWindow > 0
            ? Math.min(1, prompt / contextWindow)
            : null

    return (
        <div
            className="flex w-full flex-col gap-1 px-1 text-xs text-muted-foreground"
            data-testid="assistant-token-usage"
        >
            <div className="flex items-center gap-3 tabular-nums">
                <span>{t("pages.assistant.usage.prompt", { formatted: formatter.format(prompt) })}</span>
                <span>
                    {t("pages.assistant.usage.completion", {
                        formatted: formatter.format(completion),
                    })}
                </span>
                {cached > 0 && (
                    <span>
                        {t("pages.assistant.usage.cached", { formatted: formatter.format(cached) })}
                    </span>
                )}
            </div>
            {ratio != null && (
                <div
                    className="h-1 w-full overflow-hidden rounded bg-muted"
                    data-testid="assistant-token-usage-bar"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={Math.round(ratio * 100)}
                >
                    <div
                        className="h-full rounded bg-primary/60 transition-all"
                        style={{ width: `${Math.round(ratio * 100)}%` }}
                    />
                </div>
            )}
        </div>
    )
}
