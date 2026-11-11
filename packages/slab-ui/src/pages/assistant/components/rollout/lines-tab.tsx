/**
 * Raw lines tab: the rollout JSONL passed through verbatim — the LLM-grade
 * payloads the normal UI strips (`<think>` bodies, name-tagged injected
 * fragments, per-turn TurnState, Compacted baselines, Extended-mode events).
 */

import api from "@slab/api"
import { Button } from "@slab/components/button"
import { Badge } from "@slab/components/badge"
import { Input } from "@slab/components/input"
import { ScrollArea } from "@slab/components/scroll-area"
import { useMemo, useState } from "react"
import { useTranslation } from "@slab/i18n"

import { JsonBlock } from "./json-block"

const ROLLOUT_TYPES = [
  "sessionMeta",
  "turnItem",
  "eventMsg",
  "compacted",
  "turnContext",
] as const

const PAGE = 200

export function LinesTab({ threadId }: { threadId: string }) {
  const { t } = useTranslation()
  const [typeFilter, setTypeFilter] = useState<string>("all")
  // Substring search over the serialized line payloads (case-insensitive) —
  // the fastest way to pin down one tool call / prompt fragment in a long
  // rollout without leaving the raw view.
  const [query, setQuery] = useState("")
  const [visible, setVisible] = useState(PAGE)

  const { data, isLoading, error } = api.useQuery(
    "get",
    "/v1/agents/rollouts/{thread_id}/lines",
    { params: { path: { thread_id: threadId }, query: { limit: 5000 } } },
  )

  const lines = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return (data?.lines ?? []).filter(
      (line) =>
        (typeFilter === "all" || line.rollout_type === typeFilter) &&
        (needle === "" ||
          JSON.stringify(line.item ?? "").toLowerCase().includes(needle)),
    )
  }, [data, typeFilter, query])

  if (isLoading) return <p className="p-4 text-sm text-muted-foreground">…</p>
  if (error)
    return (
      <p className="p-4 text-sm text-destructive">{String(error as unknown)}</p>
    )

  const shown = lines.slice(0, visible)
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-micro text-muted-foreground">
          {t("pages.agentRollouts.lines.filter")}
        </span>
        {(["all", ...ROLLOUT_TYPES] as const).map((type) => (
          <Button
            key={type}
            type="button"
            variant={typeFilter === type ? "default" : "outline"}
            size="sm"
            className="h-6 px-2 text-micro"
            onClick={() => {
              setTypeFilter(type)
              setVisible(PAGE)
            }}
          >
            {type === "all" ? t("pages.agentRollouts.lines.all") : type}
          </Button>
        ))}
        <Input
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setVisible(PAGE)
          }}
          placeholder={t("pages.agentRollouts.lines.search")}
          className="ml-2 h-7 w-44 text-xs"
        />
        <span className="ml-auto text-micro text-muted-foreground">
          {t("pages.agentRollouts.lines.total", {
            shown: shown.length,
            total: lines.length,
          })}
        </span>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-2 pb-8">
          {shown.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">
              {t("pages.agentRollouts.lines.empty")}
            </p>
          ) : (
            shown.map((line) => (
              <div key={line.index} className="space-y-1">
                <div className="flex items-center gap-2">
                  <span className="font-mono text-micro text-muted-foreground">
                    {t("pages.agentRollouts.lines.index", { index: line.index })}
                  </span>
                  <span className="font-mono text-micro text-muted-foreground">
                    {line.timestamp}
                  </span>
                  <Badge variant="secondary" className="text-micro">
                    {line.rollout_type}
                  </Badge>
                </div>
                <JsonBlock value={line.item} />
              </div>
            ))
          )}
          {shown.length < lines.length ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setVisible((count) => count + PAGE)}
            >
              {t("pages.agentRollouts.lines.loadMore")}
            </Button>
          ) : null}
        </div>
      </ScrollArea>
    </div>
  )
}
