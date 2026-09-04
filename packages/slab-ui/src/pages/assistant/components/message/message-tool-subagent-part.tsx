"use client"

/**
 * `delegate_subagent` tool part — registered under
 * `messagePartComponents.tools["delegate_subagent"]`.
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
  const { approvalStatusByItemId, subagentTasksByTaskId } = useMessageInteraction()
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const partState = deriveState(p, approval)

  const partType = (p.type ?? "") as string
  const fromType = partType.startsWith("tool-") ? partType.slice("tool-".length) : partType
  const derivedName = (name ?? p.toolName ?? fromType) || "delegate_subagent"
  const summary = summarizeToolCall(derivedName, p.input)

  // Background delegations carry {task_id, background:true, …} in the output;
  // inline ones carry the legacy {completion_text, …} shape.
  const envelope = parseToolEnvelope(p.output)
  const taskId = str(envelope?.task_id)
  const isBackground = envelope?.background === true || taskId !== undefined
  const task = taskId ? subagentTasksByTaskId.get(taskId) : undefined

  // Live state wins over the part state for background delegations: the part
  // is finalized but the delegation may still be running (or have failed).
  const state =
    isBackground && task ? (liveState(task.status) ?? partState) : partState

  const input = parseToolEnvelope(p.input) ?? {}
  const agentType = str(input.agent_type)
  const maxTurns = num(input.max_turns)

  const statusLabel = task
    ? task.status
    : isBackground
      ? "delegated"
      : str(envelope?.status) ?? "completed"

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
            <span>status: {statusLabel}</span>
            {agentType ? <span>agent: {agentType}</span> : null}
            {maxTurns !== undefined ? <span>max_turns: {maxTurns}</span> : null}
            {taskId ? <span>task: {taskId}</span> : null}
          </DetailMeta>
          {task?.resultSummary ? (
            <div className="rounded-md bg-muted/40 p-2 text-xs whitespace-pre-wrap">
              {task.resultSummary}
            </div>
          ) : null}
          {str(envelope?.completion_text) ? (
            <div className="rounded-md bg-muted/40 p-2 text-xs whitespace-pre-wrap">
              {str(envelope?.completion_text)}
            </div>
          ) : null}
          {isBackground && !task?.resultSummary ? (
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
