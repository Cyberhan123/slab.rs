import { FolderOpen, MessageSquareDot, Plus } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

export type ChatSessionSummaryItem = {
  key: string
  label: string
  hint: string
  tone: "warm" | "mint"
}

type ChatSessionSummaryCardProps = {
  items: ChatSessionSummaryItem[]
  onManageSessions: () => void
  onNewSession: () => void
}

export function ChatSessionSummaryCard({
  items,
  onManageSessions,
  onNewSession,
}: ChatSessionSummaryCardProps) {
  return (
    <aside className="w-full max-w-[288px] rounded-[24px] border border-border/50 bg-[var(--shell-card)] p-6 shadow-[var(--shell-elevation)]">
      <div className="flex items-center justify-between gap-3">
        <p className="text-[12px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          Latest session
        </p>
        <Button
          type="button"
          variant="quiet"
          size="icon-xs"
          className="rounded-full"
          onClick={onNewSession}
        >
          <Plus className="size-3.5" />
          <span className="sr-only">New session</span>
        </Button>
      </div>

      <div className="mt-4 space-y-4">
        {items.map((item, index) => {
          const Icon = index === 0 ? FolderOpen : MessageSquareDot

          return (
            <button
              key={item.key}
              type="button"
              onClick={onManageSessions}
              className="flex w-full items-center gap-3 text-left"
            >
              <span
                className={cn(
                  "flex size-8 shrink-0 items-center justify-center rounded-[8px]",
                  item.tone === "warm"
                    ? "bg-[var(--brand-gold)]/20 text-[var(--brand-gold)]"
                    : "bg-[var(--brand-teal)]/20 text-[var(--brand-teal)]"
                )}
              >
                <Icon className="size-4" />
              </span>

              <span className="min-w-0">
                <span className="block truncate text-sm font-semibold text-foreground">
                  {item.label}
                </span>
                <span className="block text-[11px] text-muted-foreground">{item.hint}</span>
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
      >
        Manage sessions
      </Button>
    </aside>
  )
}
