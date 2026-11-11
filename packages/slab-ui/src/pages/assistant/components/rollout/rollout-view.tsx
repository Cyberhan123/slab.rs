/**
 * In-assistant rollout trace view — the former standalone "Agent 调试追踪"
 * page bound to the CURRENT conversation's thread (no session rail).
 *
 * Read-only inspection of the rollout true source: the timeline projection
 * reusing the assistant message renderer, the raw rollout JSONL lines (with
 * the LLM-grade payloads the normal UI strips), and the trace bundle's
 * "conversation the model actually saw" reconstruction.
 *
 * Still gated on the `agent.debug` setting: the server 404s every endpoint
 * when the flag is off, so the view renders an enable-hint instead of empty
 * errors. Rendered as the assistant page's 轨迹 tab; `threadId` is null until
 * the conversation has a bound harness thread.
 */

import api from "@slab/api"
import { Badge } from "@slab/components/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@slab/components/tabs"
import { GUARDRAIL_PMIDS, useGuardrailFlag } from "@slab/ui/lib/guardrail-flags"
import { useTranslation } from "@slab/i18n"

import { LinesTab } from "./lines-tab"
import { TimelineTab } from "./timeline-tab"
import { TraceTab } from "./trace-tab"

export function RolloutView({ threadId }: { threadId: string | null }) {
  const { t } = useTranslation()
  const debugEnabled = useGuardrailFlag(GUARDRAIL_PMIDS.agentDebug)
  // The session list is the trace-bundle registry: it tells whether THIS
  // thread carries a bundle (replaces the standalone page's left rail, which
  // was the only consumer of the full list).
  const { data: sessions } = api.useQuery("get", "/v1/agents/rollouts", {}, {
    enabled: debugEnabled,
  })
  // Tri-state: `undefined` (list still loading) lets the trace tab fetch
  // optimistically instead of flashing the missing-bundle hint.
  const hasTrace =
    sessions === undefined
      ? undefined
      : sessions.some((entry) => entry.thread_id === threadId && entry.has_trace)

  if (!debugEnabled) {
    return (
      <div className="flex h-full items-center justify-center p-8" data-testid="assistant-rollout-disabled">
        <div className="max-w-md space-y-2 text-center">
          <h2 className="text-lg font-semibold">{t("pages.agentRollouts.disabled.title")}</h2>
          <p className="text-sm text-muted-foreground">
            {t("pages.agentRollouts.disabled.description")}
          </p>
        </div>
      </div>
    )
  }

  if (!threadId) {
    return (
      <div className="flex h-full items-center justify-center p-8" data-testid="assistant-rollout-empty">
        <p className="max-w-md text-center text-sm text-muted-foreground">
          {t("pages.agentRollouts.noThread")}
        </p>
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 px-4 pt-3">
        <span className="text-micro text-muted-foreground">
          {t("pages.agentRollouts.threadLabel")}
        </span>
        <Badge variant="outline" className="shrink-0 font-mono text-micro">
          <span className="max-w-72 truncate">{threadId}</span>
        </Badge>
        {hasTrace ? (
          <Badge variant="outline" className="shrink-0 text-micro">
            {t("pages.agentRollouts.traceBadge")}
          </Badge>
        ) : null}
      </div>
      <Tabs defaultValue="timeline" className="min-h-0 flex-1 px-4 pb-4">
        <TabsList>
          <TabsTrigger value="timeline">
            {t("pages.agentRollouts.tabs.timeline")}
          </TabsTrigger>
          <TabsTrigger value="lines">{t("pages.agentRollouts.tabs.lines")}</TabsTrigger>
          <TabsTrigger value="trace">{t("pages.agentRollouts.tabs.trace")}</TabsTrigger>
        </TabsList>
        <TabsContent value="timeline" className="min-h-0">
          <TimelineTab threadId={threadId} />
        </TabsContent>
        <TabsContent value="lines" className="min-h-0">
          <LinesTab threadId={threadId} />
        </TabsContent>
        <TabsContent value="trace" className="min-h-0">
          <TraceTab threadId={threadId} hasTrace={hasTrace} />
        </TabsContent>
      </Tabs>
    </div>
  )
}
