"use client"

import { cn } from "@slab/ui/lib/utils"
import {
  CheckCircleIcon,
  CircleIcon,
  ClockIcon,
  ListChecksIcon,
  XCircleIcon,
} from "lucide-react"
import type { ReactNode } from "react"

import type { Plan, PlanStatus } from "@slab/core/harness/types"
import { useMessageInteraction } from "../message-interaction-context"
import { ToolRow, ToolRowContent, ToolRowTrigger, toolRowIcon } from "./message-tool-row"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import {
  isApprovalPending,
  deriveState,
  type ToolPartLike,
} from "./message-tool-part"

/** Icon + tone for a single step's lifecycle status. */
function statusIcon(status: PlanStatus): ReactNode {
  switch (status) {
    case "completed":
      return <CheckCircleIcon className="size-4 shrink-0 text-green-600" />
    case "in_progress":
      return <ClockIcon className="size-4 shrink-0 animate-pulse text-blue-600" />
    case "blocked":
      return <XCircleIcon className="size-4 shrink-0 text-orange-600" />
    default:
      return <CircleIcon className="size-4 shrink-0 text-muted-foreground" />
  }
}

/**
 * Read-only rendering of a structured {@link Plan} body: summary, a compact
 * counts line, and the ordered step list with per-step status icons (the
 * current in-progress step is emphasized). Shared by the in-stream plan card
 * and the plan approval card so the two views cannot drift.
 */
export function PlanCardBody({ plan }: { plan: Plan }) {
  const total = plan.items.length
  const c = plan.counts
  return (
    <div className="space-y-3" data-testid="assistant-plan-body">
      {plan.summary ? <p className="text-sm text-foreground">{plan.summary}</p> : null}
      <p className="text-muted-foreground text-xs">
        {total} steps · {c.completed} done · {c.in_progress} in progress · {c.pending} pending
        {c.blocked > 0 ? ` · ${c.blocked} blocked` : ""}
      </p>
      <ol className="space-y-1.5">
        {plan.items.map((item, index) => {
          const current = plan.current_step === index
          return (
            <li
              key={`${index}:${item.step}`}
              className={cn(
                "flex items-start gap-2 rounded-md p-2 text-xs",
                current ? "bg-blue-500/10 ring-1 ring-blue-500/30" : "bg-muted/50",
              )}
            >
              {statusIcon(item.status)}
              <div className="min-w-0 flex-1">
                <span className={cn("text-foreground", current && "font-medium")}>{item.step}</span>
                {item.depends_on && item.depends_on.length > 0 ? (
                  <span className="ml-2 text-muted-foreground">
                    (needs: {item.depends_on.join(", ")})
                  </span>
                ) : null}
                {item.result_ref ? (
                  <span className="ml-2 font-mono text-muted-foreground">{item.result_ref}</span>
                ) : null}
              </div>
            </li>
          )
        })}
      </ol>
    </div>
  )
}

/**
 * Renders a `plan` / `update_plan` / `present_plan` tool call as a compact
 * `Plan: <summary>` row (thinking-style, collapsed by default) whose expanded
 * body keeps the structured plan view (summary / counts / step list).
 *
 * Registered under `messagePartComponents.tools["plan"]` so the parts engine
 * routes plan tools here ahead of the generic `tool` renderer (see
 * `getMessagePartComponent`: `tools[name]` is checked before `tool`).
 */
function MessageToolPlanPart({
  part,
  kind,
  toolCallId,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const { approvalStatusByItemId } = useMessageInteraction()
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)

  const plan = (p.input ?? {}) as Plan
  const title = plan.summary ?? "plan"

  return (
    <ToolRow defaultOpen={isApprovalPending(state)}>
      <ToolRowTrigger
        icon={toolRowIcon("plan")}
        label="Plan"
        detail={title}
        state={state}
      />
      <ToolRowContent>
        <div data-testid="assistant-tool-plan" className="space-y-3">
          <div className="flex items-center gap-2 text-muted-foreground text-xs">
            <ListChecksIcon className="size-4" />
            <span>Plan</span>
          </div>
          <PlanCardBody plan={plan} />
        </div>
      </ToolRowContent>
    </ToolRow>
  )
}

export default MessageToolPlanPart
