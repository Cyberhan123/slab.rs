import { FolderOpen, MessageSquareDot } from "lucide-react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import { cn } from "@/lib/utils"

export type AssistantSessionSummaryItem = {
  key: string
  label: string
  hint: string
  tone: "warm" | "mint"
}

type AssistantSessionSummaryCardProps = {
  items: AssistantSessionSummaryItem[]
  onManageSessions: () => void
  onNewSession: () => void
  onSelectSession: (key: string) => void
  disableNewSession?: boolean
  testIdPrefix: string
}

export function AssistantSessionSummaryCard({
  items,
  onManageSessions,
  onNewSession,
  onSelectSession,
  disableNewSession = false,
  testIdPrefix,
}: AssistantSessionSummaryCardProps) {
  const { t } = useTranslation()

  return (
    <aside className="w-full max-w-[288px] rounded-2xl border border-border/50 bg-[var(--shell-card)] p-6">
      <div className="flex items-center justify-between gap-3">
        <p className="text-label font-bold uppercase tracking-eyebrow text-muted-foreground">
          {t("pages.assistant.summaryCard.latestSession")}
        </p>
        <Button
          type="button"
          variant="quiet"
          size="icon-xs"
          className="rounded-full"
          onClick={onNewSession}
          disabled={disableNewSession}
          data-testid={`${testIdPrefix}-new-session-button`}
        >
          <MessageSquareDot className="size-3.5" />
          <span className="sr-only">{t("pages.assistant.summaryCard.createSession")}</span>
        </Button>
      </div>

      <div className="mt-4 space-y-4">
        {items.map((item, index) => {
          const Icon = index === 0 ? FolderOpen : MessageSquareDot

          return (
            <button
              key={item.key}
              type="button"
              data-testid={`${testIdPrefix}-session-${item.key}`}
              onClick={() => onSelectSession(item.key)}
              className="flex w-full items-center gap-3 text-left"
            >
              <span
                className={cn(
                  "flex size-8 shrink-0 items-center justify-center rounded-[8px]",
                  item.tone === "warm"
                    ? "bg-[color:color-mix(in_oklab,var(--brand-gold)_20%,transparent)] text-[color:var(--brand-gold)]"
                    : "bg-[color:color-mix(in_oklab,var(--brand-teal)_20%,transparent)] text-[color:var(--brand-teal)]"
                )}
              >
                <Icon className="size-4" />
              </span>

              <span className="min-w-0">
                <span className="block truncate text-sm font-semibold text-foreground">
                  {item.label}
                </span>
                <span className="block text-caption text-muted-foreground">{item.hint}</span>
              </span>
            </button>
          )
        })}
      </div>

      <Button
        type="button"
        variant="outline"
        className="mt-5 h-[38px] w-full rounded-[12px] border-border/60 bg-[var(--shell-card)] text-foreground/70 hover:bg-[var(--surface-soft)]"
        onClick={onManageSessions}
        data-testid={`${testIdPrefix}-manage-sessions-button`}
      >
        {t("pages.assistant.summaryCard.manageSessions")}
      </Button>
    </aside>
  )
}
