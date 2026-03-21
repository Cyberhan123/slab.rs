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
    <aside className="w-full max-w-[288px] rounded-[24px] border border-border/50 bg-white p-6 shadow-[0_1px_2px_rgba(0,0,0,0.05)]">
      <div className="flex items-center justify-between gap-3">
        <p className="text-[12px] font-bold uppercase tracking-[0.18em] text-[#6d7a77]">
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
                    ? "bg-[#ffddb8] text-[#855300]"
                    : "bg-[#89f5e7] text-[#00685f]"
                )}
              >
                <Icon className="size-4" />
              </span>

              <span className="min-w-0">
                <span className="block truncate text-sm font-semibold text-[#191c1e]">
                  {item.label}
                </span>
                <span className="block text-[11px] text-[#6d7a77]">{item.hint}</span>
              </span>
            </button>
          )
        })}
      </div>

      <Button
        type="button"
        variant="outline"
        className="mt-5 h-[38px] w-full rounded-[12px] border-border/60 bg-white text-[#3d4947] hover:bg-[var(--surface-soft)]"
        onClick={onManageSessions}
      >
        Manage sessions
      </Button>
    </aside>
  )
}
