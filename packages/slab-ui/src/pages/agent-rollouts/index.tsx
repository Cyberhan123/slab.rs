/**
 * Rollout debug viewer — the "Agent 调试追踪" surface.
 *
 * Read-only inspection of the rollout true source: sessions discovered on
 * disk (left rail) and, per thread, the timeline projection reusing the
 * assistant message renderer, the raw rollout JSONL lines (with the LLM-grade
 * payloads the normal UI strips), and the trace bundle's
 * "conversation the model actually saw" reconstruction.
 *
 * Gated on the `agent.debug` setting: the server 404s every endpoint when the
 * flag is off, and this page renders an enable-hint instead of empty errors.
 */

import { useMemo, useState } from "react"
import api from "@slab/api"
import { Badge } from "@slab/components/badge"
import { ScrollArea } from "@slab/components/scroll-area"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@slab/components/tabs"
import { cn } from "@slab/ui/lib/utils"
import { useGuardrailFlag, GUARDRAIL_PMIDS } from "@slab/ui/lib/guardrail-flags"
import { useTranslation } from "@slab/i18n"

import { LinesTab } from "./lines-tab"
import { TimelineTab } from "./timeline-tab"
import { TraceTab } from "./trace-tab"

export default function AgentRolloutsPage() {
  const { t } = useTranslation()
  const debugEnabled = useGuardrailFlag(GUARDRAIL_PMIDS.agentDebug)
  const { data: sessions, isLoading } = api.useQuery(
    "get",
    "/v1/agents/rollouts",
    {},
    { enabled: debugEnabled },
  )
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null)

  const groups = useMemo(() => {
    const byDay = new Map<string, NonNullable<typeof sessions>[number][]>()
    for (const entry of sessions ?? []) {
      const day = entry.started_at.slice(0, 10) || "unknown"
      const bucket = byDay.get(day) ?? []
      bucket.push(entry)
      byDay.set(day, bucket)
    }
    return Array.from(byDay.entries())
  }, [sessions])

  const selected = useMemo(
    () => (sessions ?? []).find((entry) => entry.thread_id === selectedThreadId) ?? null,
    [sessions, selectedThreadId],
  )

  if (!debugEnabled) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="max-w-md space-y-2 text-center">
          <h2 className="text-lg font-semibold">{t("pages.agentRollouts.disabled.title")}</h2>
          <p className="text-sm text-muted-foreground">
            {t("pages.agentRollouts.disabled.description")}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-72 shrink-0 flex-col border-r">
        <div className="px-4 py-3">
          <h2 className="text-sm font-semibold">{t("pages.agentRollouts.rail.title")}</h2>
        </div>
        <ScrollArea className="min-h-0 flex-1">
          <div className="space-y-4 px-2 pb-4">
            {isLoading ? null : groups.length === 0 ? (
              <p className="px-2 text-xs text-muted-foreground">
                {t("pages.agentRollouts.rail.empty")}
              </p>
            ) : (
              groups.map(([day, entries]) => (
                <div key={day} className="space-y-1">
                  <p className="px-2 text-micro font-medium text-muted-foreground">{day}</p>
                  {entries.map((entry) => (
                    <button
                      key={entry.thread_id}
                      type="button"
                      data-testid={`rollout-session-${entry.thread_id}`}
                      onClick={() => setSelectedThreadId(entry.thread_id)}
                      className={cn(
                        "w-full space-y-1 rounded-lg px-2 py-2 text-left transition-colors",
                        entry.thread_id === selectedThreadId
                          ? "bg-accent text-accent-foreground"
                          : "hover:bg-accent/50",
                      )}
                    >
                      <div className="flex items-center gap-2">
                        <span className="truncate font-mono text-xs">{entry.thread_id}</span>
                        {entry.has_trace ? (
                          <Badge variant="outline" className="shrink-0 text-micro">
                            {t("pages.agentRollouts.rail.traceBadge")}
                          </Badge>
                        ) : null}
                      </div>
                      <div className="flex items-center justify-between text-micro text-muted-foreground">
                        <span className="truncate">{entry.role_name ?? entry.session_id}</span>
                        <span className="shrink-0">{formatBytes(entry.size_bytes)}</span>
                      </div>
                    </button>
                  ))}
                </div>
              ))
            )}
          </div>
        </ScrollArea>
      </aside>

      <main className="min-h-0 min-w-0 flex-1 p-4">
        {selected ? (
          <Tabs defaultValue="timeline" className="flex h-full min-h-0 flex-col">
            <TabsList>
              <TabsTrigger value="timeline">
                {t("pages.agentRollouts.tabs.timeline")}
              </TabsTrigger>
              <TabsTrigger value="lines">{t("pages.agentRollouts.tabs.lines")}</TabsTrigger>
              <TabsTrigger value="trace">{t("pages.agentRollouts.tabs.trace")}</TabsTrigger>
            </TabsList>
            <TabsContent value="timeline" className="min-h-0 flex-1">
              <TimelineTab threadId={selected.thread_id} />
            </TabsContent>
            <TabsContent value="lines" className="min-h-0 flex-1">
              <LinesTab threadId={selected.thread_id} />
            </TabsContent>
            <TabsContent value="trace" className="min-h-0 flex-1">
              <TraceTab threadId={selected.thread_id} hasTrace={selected.has_trace} />
            </TabsContent>
          </Tabs>
        ) : (
          <div className="flex h-full items-center justify-center">
            <p className="text-sm text-muted-foreground">{t("pages.agentRollouts.rail.empty")}</p>
          </div>
        )}
      </main>
    </div>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
