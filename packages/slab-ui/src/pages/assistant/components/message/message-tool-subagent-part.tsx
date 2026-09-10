"use client"

/**
 * Subagent tool part — registered under `messagePartComponents.tools` for
 * `delegate_subagent` (delegation card) and the companion
 * `subagent_status` / `subagent_message` / `subagent_stop` calls (registry
 * queries and steering, rendered through the same card with envelope-shape
 * tolerance — see the normalization below).
 *
 * A dedicated card for subagent delegations. In background mode (the default)
 * the tool call returns IMMEDIATELY, so the part itself reaches
 * `output-available` while the delegation is still running — the live status
 * comes from the out-of-band `subagentTasksByTaskId` state (fed by
 * `backgroundTask/updated` with `kind: "subagent"`). The expanded body shows
 * the delegation meta plus the result summary once terminal; the full result
 * arrives as a follow-up conversation message (server-side auto-resume).
 *
 * Inline (`background=false`) delegations keep the classic shape: the part
 * output carries the completion text directly.
 */

import { ToolRow, ToolRowContent, ToolRowTrigger, toolRowIcon } from "./message-tool-row"
import { deriveState, ToolErrorText, type ToolPartLike, type ToolState } from "./message-tool-part"
import { useMessageInteraction } from "../message-interaction-context"
import { summarizeToolCall } from "../../lib/tool-summaries"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { DetailFootnote, DetailMeta, num, parseToolEnvelope, str } from "./tool-detail-shared"

/** Map a live subagent task status onto the row status-symbol vocabulary. */
function liveState(status: string | undefined): ToolState | null {
  switch (status) {
    case "running":
      return "input-available"
    case "completed":
      return "output-available"
    case "failed":
      return "output-error"
    case "stopped":
      return "output-denied"
    default:
      return null
  }
}

function MessageToolSubagentPart(props: MessagePartRenderProps<TMessagePart, TMessage>) {
  const { part, kind, name, toolCallId } = props
  const { approvalStatusByItemId, subagentTasksByTaskId, subagentChildItemsByChildId } =
    useMessageInteraction()
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const partState = deriveState(p, approval)

  const partType = (p.type ?? "") as string
  const fromType = partType.startsWith("tool-") ? partType.slice("tool-".length) : partType
  const derivedName = (name ?? p.toolName ?? fromType) || "delegate_subagent"
  const summary = summarizeToolCall(derivedName, p.input)

  // Envelope normalization across the subagent tool family:
  // - delegate_subagent: {task_id, background, child_thread_id, …} or inline
  //   {status, completion_text, …}
  // - subagent_status:   {task: {task_id, child_thread_id, status, result, …}}
  //   (or {tasks: […]} for the list form)
  // - subagent_message:  {queued: bool, position?} (task_id only in input)
  // - subagent_stop:     {stopped: {task_id, status, result, …}}
  // Anything that does not parse degrades safely: the card still renders the
  // summarized call with no status/result extras.
  const envelope = parseToolEnvelope(p.output)
  const input = parseToolEnvelope(p.input) ?? {}
  const statusSnapshot = (envelope?.task ?? envelope?.stopped ?? null) as Record<
    string,
    unknown
  > | null
  const taskId =
    str(envelope?.task_id) ?? str(statusSnapshot?.task_id) ?? str(input.task_id)
  const isBackground = envelope?.background === true || taskId !== undefined
  const task = taskId ? subagentTasksByTaskId.get(taskId) : undefined
  // Relayed child activity (`subagent/childEvent`) correlates by the real
  // child thread id from the delegation envelope (or a status/stop snapshot).
  const childThreadId = str(envelope?.child_thread_id) ?? str(statusSnapshot?.child_thread_id)
  const childItems =
    isBackground && childThreadId ? subagentChildItemsByChildId.get(childThreadId) : undefined

  // Live state wins over the part state for background delegations: the part
  // is finalized but the delegation may still be running (or have failed).
  const state =
    isBackground && task ? (liveState(task.status) ?? partState) : partState

  const agentType = str(input.agent_type)
  const maxTurns = num(input.max_turns)

  const envelopeStatus = str(statusSnapshot?.status) ?? str(envelope?.status)
  const queued = envelope?.queued === true
  const resultText =
    str(envelope?.completion_text) ?? str(statusSnapshot?.result)
  // No fabricated status: an unknown envelope shape renders without a status
  // span instead of claiming success, and the delegation-vocabulary
  // "delegated" fallback applies only to delegate_subagent calls.
  const isDelegation = derivedName === "delegate_subagent"
  const statusLabel =
    task?.status ??
    (queued ? "queued" : envelopeStatus) ??
    (isDelegation && isBackground ? "delegated" : undefined)

  return (
    <ToolRow defaultOpen={state === "approval-requested"}>
      <ToolRowTrigger
        icon={toolRowIcon(derivedName)}
        label={summary.label}
        detail={summary.detail}
        state={state}
        title={p.errorText}
      />
      <ToolRowContent>
        <div className="space-y-2" data-testid="tool-detail-subagent">
          <DetailMeta>
            {statusLabel ? <span>status: {statusLabel}</span> : null}
            {agentType ? <span>agent: {agentType}</span> : null}
            {maxTurns !== undefined ? <span>max_turns: {maxTurns}</span> : null}
            {taskId ? <span>task: {taskId}</span> : null}
          </DetailMeta>
          {childItems && childItems.length > 0 ? (
            <div className="space-y-1" data-testid="tool-detail-subagent-activity">
              {childItems.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center gap-2 text-xs text-muted-foreground"
                >
                  <span
                    aria-hidden
                    className={
                      entry.phase === "completed"
                        ? "text-emerald-500"
                        : "animate-pulse text-muted-foreground/70"
                    }
                  >
                    {entry.phase === "completed" ? "✓" : "•"}
                  </span>
                  <span className="truncate">{entry.label}</span>
                </div>
              ))}
            </div>
          ) : null}
          {task?.resultSummary ? (
            <div className="rounded-md bg-muted/40 p-2 text-xs whitespace-pre-wrap">
              {task.resultSummary}
            </div>
          ) : null}
          {resultText ? (
            <div className="rounded-md bg-muted/40 p-2 text-xs whitespace-pre-wrap">
              {resultText}
            </div>
          ) : null}
          {derivedName === "delegate_subagent" && isBackground && !task?.resultSummary ? (
            <DetailFootnote>
              background delegation — the result arrives as a follow-up message when the
              subagent finishes (track with subagent_status / subagent_message / subagent_stop)
            </DetailFootnote>
          ) : null}
          {p.errorText ? <ToolErrorText errorText={p.errorText} /> : null}
        </div>
      </ToolRowContent>
    </ToolRow>
  )
}

export default MessageToolSubagentPart
