import { Tooltip, TooltipContent, TooltipTrigger } from "@slab/components/tooltip"
import { useTranslation } from "@slab/i18n"

import type { TurnUsage } from "../lib/harness"

/**
 * Token-usage indicator driven by the `turn/completed` harness notification's
 * `usage` payload. Shows a compact "used X%" label (or a total-token count when
 * the context window is unknown); hovering reveals the prompt / completion /
 * cached breakdown as plain text. Rendered in the composer footer. Hidden when
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
        contextWindow && contextWindow > 0 ? Math.min(1, prompt / contextWindow) : null

    const label =
        ratio != null
            ? t("pages.assistant.usage.used", { percent: Math.round(ratio * 100) })
            : t("pages.assistant.usage.total", {
                  formatted: formatter.format(usage.totalTokens ?? 0),
              })

    return (
        <Tooltip>
            <TooltipTrigger asChild>
                <span
                    className="w-fit cursor-default px-1 text-xs tabular-nums text-muted-foreground"
                    data-testid="assistant-token-usage"
                >
                    {label}
                </span>
            </TooltipTrigger>
            <TooltipContent>
                <div className="flex flex-col gap-0.5 tabular-nums">
                    <span>
                        {t("pages.assistant.usage.prompt", { formatted: formatter.format(prompt) })}
                    </span>
                    <span>
                        {t("pages.assistant.usage.completion", {
                            formatted: formatter.format(completion),
                        })}
                    </span>
                    {cached > 0 && (
                        <span>
                            {t("pages.assistant.usage.cached", {
                                formatted: formatter.format(cached),
                            })}
                        </span>
                    )}
                </div>
            </TooltipContent>
        </Tooltip>
    )
}
