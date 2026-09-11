/**
 * Trace tab: the thread's trace bundle — manifest, the raw trace events, and
 * the reducer's L3 reconstruction of the conversation the model actually saw
 * (the cross-check for the timeline's per-turn prompts).
 */

import api from "@slab/api"
import { Button } from "@slab/components/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@slab/components/collapsible"
import { ScrollArea } from "@slab/components/scroll-area"
import { ChevronRightIcon } from "lucide-react"
import { useState } from "react"
import { useTranslation } from "@slab/i18n"

import { JsonBlock } from "./json-block"

const PAGE = 100

export function TraceTab({ threadId, hasTrace }: { threadId: string; hasTrace: boolean }) {
  const { t } = useTranslation()
  const [visible, setVisible] = useState(PAGE)
  const { data, isLoading, error } = api.useQuery(
    "get",
    "/v1/agents/rollouts/{thread_id}/trace",
    { params: { path: { thread_id: threadId }, query: { limit: 5000 } } },
  )

  if (!hasTrace) {
    return (
      <p className="p-4 text-sm text-muted-foreground">{t("pages.agentRollouts.trace.missing")}</p>
    )
  }
  if (isLoading) return <p className="p-4 text-sm text-muted-foreground">…</p>
  if (error)
    return (
      <p className="p-4 text-sm text-destructive">{String(error as unknown)}</p>
    )

  const shown = (data?.events ?? []).slice(0, visible)
  return (
    <ScrollArea className="h-full">
      <div className="space-y-4 pb-8">
        <Collapsible defaultOpen>
          <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-accent/50">
            <ChevronRightIcon className="size-4 transition-transform group-data-[state=open]:rotate-90" />
            <span className="font-medium">{t("pages.agentRollouts.trace.manifest")}</span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="px-2 pb-2">
              <JsonBlock value={data?.manifest} />
            </div>
          </CollapsibleContent>
        </Collapsible>

        <Collapsible defaultOpen>
          <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-accent/50">
            <ChevronRightIcon className="size-4 transition-transform group-data-[state=open]:rotate-90" />
            <span className="font-medium">
              {t("pages.agentRollouts.trace.conversation")}
            </span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="space-y-2 px-2 pb-2">
              {(data?.conversation ?? []).length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t("pages.agentRollouts.trace.conversationEmpty")}
                </p>
              ) : (
                (data?.conversation ?? []).map((message, index) => (
                  <div key={index} className="space-y-1">
                    <span className="font-mono text-micro text-muted-foreground">
                      #{index}
                    </span>
                    <JsonBlock value={message} className="max-h-64" />
                  </div>
                ))
              )}
            </div>
          </CollapsibleContent>
        </Collapsible>

        <Collapsible>
          <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-accent/50">
            <ChevronRightIcon className="size-4 transition-transform group-data-[state=open]:rotate-90" />
            <span className="font-medium">{t("pages.agentRollouts.trace.events")}</span>
            <span className="ml-auto text-micro text-muted-foreground">
              {t("pages.agentRollouts.trace.eventsCount", {
                shown: shown.length,
                total: data?.total_events ?? 0,
              })}
            </span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="space-y-2 px-2 pb-2">
              {shown.map((event, index) => (
                <JsonBlock key={index} value={event} className="max-h-48" />
              ))}
              {shown.length < (data?.events ?? []).length ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => setVisible((count) => count + PAGE)}
                >
                  {t("pages.agentRollouts.trace.loadMore")}
                </Button>
              ) : null}
            </div>
          </CollapsibleContent>
        </Collapsible>
      </div>
    </ScrollArea>
  )
}
