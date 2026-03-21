import { History, Layers3, Sparkles } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"

type ChatSessionSummaryCardProps = {
  title: string
  messageCount: number
  modelLabel: string
  deepThink: boolean
  onManageSessions: () => void
  onNewSession: () => void
}

export function ChatSessionSummaryCard({
  title,
  messageCount,
  modelLabel,
  deepThink,
  onManageSessions,
  onNewSession,
}: ChatSessionSummaryCardProps) {
  return (
    <Card variant="soft" className="w-full max-w-sm gap-4">
      <CardHeader className="gap-3">
        <div className="flex items-center justify-between gap-3">
          <Badge variant="chip">Latest session</Badge>
          <Sparkles className="size-4 text-[var(--brand-gold)]" />
        </div>
        <CardTitle className="line-clamp-2 text-lg">{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4 pt-0">
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="rounded-2xl bg-[var(--surface-soft)] px-4 py-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
              Messages
            </p>
            <p className="mt-1 inline-flex items-center gap-2 text-sm font-medium">
              <History className="size-4 text-muted-foreground" />
              {messageCount}
            </p>
          </div>
          <div className="rounded-2xl bg-[var(--surface-soft)] px-4 py-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
              Mode
            </p>
            <p className="mt-1 inline-flex items-center gap-2 text-sm font-medium">
              <Layers3 className="size-4 text-muted-foreground" />
              {deepThink ? "Deep think" : "Standard"}
            </p>
          </div>
        </div>

        <div className="rounded-2xl bg-[var(--surface-soft)] px-4 py-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
            Model
          </p>
          <p className="mt-1 text-sm font-medium">{modelLabel}</p>
        </div>

        <div className="flex gap-2">
          <Button variant="pill" size="pill" className="flex-1" onClick={onManageSessions}>
            Manage sessions
          </Button>
          <Button variant="quiet" size="pill" className="flex-1" onClick={onNewSession}>
            New session
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
