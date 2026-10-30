import type { MessagePartItem } from "../components/message/message-parts"

/**
 * Group message parts into Trace runs (shadcn chat rule: "rows rendered from
 * message parts go in one `Trace` per run of parts"). Consecutive
 * `tool`/`reasoning` items join one run; any other kind — text above all —
 * ends the run and renders outside. A pending approval also ends the run
 * after its row (the prompt itself lives in the out-of-band ApprovalCard).
 */

export type TraceRun<TPart = unknown, TMessage = unknown> = {
  /** Whether the run renders inside a `<Trace>` container. */
  trace: boolean
  key: string
  items: Array<MessagePartItem<TPart, TMessage>>
}

const TRACE_RUN_KINDS = new Set(["tool", "reasoning"])

export function groupTraceRuns<TPart, TMessage>(
  items: ReadonlyArray<MessagePartItem<TPart, TMessage>>,
  isApprovalPending?: (item: MessagePartItem<TPart, TMessage>) => boolean
): Array<TraceRun<TPart, TMessage>> {
  const runs: Array<TraceRun<TPart, TMessage>> = []
  // Set after an approval-pending row so the next trace item starts a fresh
  // run instead of rejoining the row's run.
  let forceNew = false

  for (const item of items) {
    const joinsTrace = TRACE_RUN_KINDS.has(item.kind) && !isApprovalPending?.(item)
    const run = runs[runs.length - 1]

    if (run && !forceNew && run.trace === joinsTrace) {
      run.items.push(item)
    } else {
      runs.push({ trace: joinsTrace, key: item.key, items: [item] })
    }

    forceNew = TRACE_RUN_KINDS.has(item.kind) && !!isApprovalPending?.(item)
  }

  return runs
}
