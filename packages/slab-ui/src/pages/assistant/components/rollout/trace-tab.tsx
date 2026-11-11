/**
 * Trace tab: the thread's trace bundle — manifest, the raw trace events, and
 * the reducer's L3 reconstruction of the conversation the model actually saw
 * (the cross-check for the timeline's per-turn prompts).
 *
 * Events are fetched NETWORK-PAGED (offset/limit) instead of one full
 * `limit=5000` pull per tab switch. The endpoint pages only the `events`
 * array — manifest + conversation ride along on every page fetch (a debug
 * viewer tradeoff; the first page alone is far lighter than the old full
 * pull).
 */

import api from "@slab/api"
import { Button } from "@slab/components/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@slab/components/collapsible"
import { ScrollArea } from "@slab/components/scroll-area"
import { ChevronRightIcon } from "lucide-react"
import { useTranslation } from "@slab/i18n"

import { JsonBlock } from "./json-block"

const PAGE = 500

/**
 * `hasTrace` comes from the rollout session list (`SessionMeta.trace_path`).
 * `false` renders the missing-bundle hint without a request; `undefined`
 * (list not loaded yet, or a caller without the session snapshot) optimistically
 * fetches — the endpoint's own empty/error state shows instead.
 */
export function TraceTab({ threadId, hasTrace }: { threadId: string; hasTrace?: boolean }) {
  const { t } = useTranslation()
  const {
    data,
    isLoading,
    error,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
  } = api.useInfiniteQuery(
    "get",
    "/v1/agents/rollouts/{thread_id}/trace",
    { params: { path: { thread_id: threadId }, query: { limit: PAGE } } },
    {
      pageParamName: "offset",
      initialPageParam: 0,
      getNextPageParam: (lastPage, _allPages, lastPageParam) => {
        const fetched = (lastPageParam as number) + lastPage.events.length
        return fetched < lastPage.total_events && lastPage.events.length > 0
          ? fetched
          : undefined
      },
    },
  )

  if (hasTrace === false) {
    return (
      <p className="p-4 text-sm text-muted-foreground">{t("pages.agentRollouts.trace.missing")}</p>
    )
  }
  if (isLoading) return <p className="p-4 text-sm text-muted-foreground">…</p>
  if (error)
    return (
      <p className="p-4 text-sm text-destructive">{String(error as unknown)}</p>
    )

  // Manifest + conversation come from the FIRST page; events accumulate.
  const firstPage = data?.pages[0]
  const events = (data?.pages ?? []).flatMap((page) => page.events)
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
              <JsonBlock value={firstPage?.manifest} />
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
              {(firstPage?.conversation ?? []).length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t("pages.agentRollouts.trace.conversationEmpty")}
                </p>
              ) : (
                (firstPage?.conversation ?? []).map((message, index) => (
                  <div
                    // Entries are raw JSON without stable ids; the list is
                    // append-only, so position is the identity.
                    // oxlint-disable-next-line react/no-array-index-key -- positional entries
                    key={index}
                    className="space-y-1"
                  >
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
                shown: events.length,
                total: firstPage?.total_events ?? 0,
              })}
            </span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="space-y-2 px-2 pb-2">
              {events.map((event, index) => {
                // The event log is append-only raw JSON without stable ids.
                // oxlint-disable-next-line react/no-array-index-key -- positional log rows
                return <JsonBlock key={index} value={event} className="max-h-48" />
              })}
              {hasNextPage ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={isFetchingNextPage}
                  onClick={() => fetchNextPage()}
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
