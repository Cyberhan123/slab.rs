/**
 * Timeline tab: the rollout projected into the harness `Thread` wire type —
 * the exact same projection `thread/resume` restores history with — rendered
 * with the assistant message renderer, plus every turn's final prompt
 * (`TurnState.input_messages`, the complete input the model was sent).
 */

import api from "@slab/api"
import type { UIMessage } from "ai"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@slab/components/collapsible"
import { ScrollArea } from "@slab/components/scroll-area"
import { ChevronRightIcon } from "lucide-react"
import { useMemo } from "react"
import { useTranslation } from "@slab/i18n"
import { turnItemsToMessages } from "@slab/core/harness"
import type { Thread } from "@slab/api/harness"
import { MessageItem } from "@slab/ui/pages/assistant/components/message/message-item"

import { JsonBlock } from "./json-block"

export function TimelineTab({ threadId }: { threadId: string }) {
  const { t } = useTranslation()
  const { data, isLoading, error } = api.useQuery(
    "get",
    "/v1/agents/rollouts/{thread_id}/timeline",
    { params: { path: { thread_id: threadId } } },
  )

  const messages = useMemo<UIMessage[]>(() => {
    const thread = data?.thread as Thread | undefined
    if (!thread) return []
    return turnItemsToMessages(thread.turns.flatMap((turn) => turn.items))
  }, [data])

  if (isLoading) return <p className="p-4 text-sm text-muted-foreground">…</p>
  if (error)
    return (
      <p className="p-4 text-sm text-destructive">{String(error as unknown)}</p>
    )

  return (
    <ScrollArea className="h-full">
      <div className="space-y-4 pb-8">
        {messages.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">
            {t("pages.agentRollouts.timeline.empty")}
          </p>
        ) : (
          <div className="space-y-2">
            {messages.map((message) => (
              <MessageItem key={message.id} message={message} />
            ))}
          </div>
        )}

        {(data?.turn_prompts ?? []).map((prompt, index) => (
          <Collapsible key={index}>
            <CollapsibleTrigger className="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm hover:bg-accent/50">
              <ChevronRightIcon className="size-4 transition-transform group-data-[state=open]:rotate-90" />
              <span className="font-medium">
                {t("pages.agentRollouts.timeline.turnPrompt", { index })}
              </span>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <div className="space-y-1 px-2 pb-2">
                <p className="text-micro text-muted-foreground">
                  {t("pages.agentRollouts.timeline.turnPromptHint")}
                </p>
                {prompt === null || prompt === undefined ? (
                  <p className="text-xs text-muted-foreground">
                    {t("pages.agentRollouts.timeline.turnPromptMissing")}
                  </p>
                ) : (
                  <JsonBlock value={prompt} className="max-h-96" />
                )}
              </div>
            </CollapsibleContent>
          </Collapsible>
        ))}
      </div>
    </ScrollArea>
  )
}
