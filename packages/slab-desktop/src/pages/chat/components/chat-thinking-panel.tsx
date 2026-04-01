import { Brain, ChevronDown } from "lucide-react"
import { useState } from "react"
import type { ReactNode } from "react"

import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@slab/components/collapsible"
import { cn } from "@/lib/utils"

type ChatThinkingPanelProps = {
  thinking: string
  loading?: boolean
  children: ReactNode
}

export function ChatThinkingPanel({
  thinking,
  loading = false,
  children,
}: ChatThinkingPanelProps) {
  const [open, setOpen] = useState(false)

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="space-y-2">
      <CollapsibleTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center justify-between rounded-2xl border border-border/60 bg-[var(--surface-soft)] px-3 py-2 text-left text-sm text-muted-foreground transition hover:border-border hover:text-foreground"
        >
          <span className="inline-flex items-center gap-2">
            <Brain className={cn("size-4", loading && "animate-pulse")} />
            {loading ? "Thinking..." : "Reasoning trace"}
          </span>
          <ChevronDown
            className={cn("size-4 transition-transform", open && "rotate-180")}
          />
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent className="overflow-hidden rounded-2xl border border-border/60 bg-[var(--surface-soft)]">
        <div className="px-4 py-3 text-sm text-muted-foreground">
          {thinking ? children : "Waiting for reasoning content..."}
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}
