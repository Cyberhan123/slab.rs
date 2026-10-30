import type { MessagePartItem } from "../components/message/message-parts"

/**
 * Group message parts into Trace runs (shadcn chat rule: "rows rendered from
 * message parts go in one `Trace` per run of parts"). Consecutive
 * `tool`/`reasoning` items join one run; any other kind — text above all —
 * ends the run and renders outside.
 *
 * Deliberately kind-only: run boundaries must stay stable for a given parts
 * sequence, because a row that re-parents (bare fragment ↔ Trace) remounts and
 * loses its expanded state — an approval-pending row that resolves mid-turn
 * would collapse and hide its diff card.
 */

export type TraceRun<TPart = unknown, TMessage = unknown> = {
  /** Whether the run renders inside a `<Trace>` container. */
  trace: boolean
  key: string
  items: Array<MessagePartItem<TPart, TMessage>>
}

const TRACE_RUN_KINDS = new Set(["tool", "reasoning"])

export function groupTraceRuns<TPart, TMessage>(
  items: ReadonlyArray<MessagePartItem<TPart, TMessage>>
): Array<TraceRun<TPart, TMessage>> {
  const runs: Array<TraceRun<TPart, TMessage>> = []

  for (const item of items) {
    const joinsTrace = TRACE_RUN_KINDS.has(item.kind)
    const run = runs[runs.length - 1]

    if (run && run.trace === joinsTrace) {
      run.items.push(item)
    } else {
      runs.push({ trace: joinsTrace, key: item.key, items: [item] })
    }
  }

  return runs
}
