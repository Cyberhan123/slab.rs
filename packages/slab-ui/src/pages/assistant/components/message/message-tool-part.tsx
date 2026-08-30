"use client"

import { cn } from "@slab/ui/lib/utils"
import { isValidElement, type ComponentProps, type ReactNode } from "react"

import { CodeBlock } from "./code-block"
import { useMessageInteraction, type ApprovalStatus } from "../message-interaction-context"
import { summarizeToolCall } from "../../lib/tool-summaries"
import { ToolRow, ToolRowContent, ToolRowTrigger, toolRowIcon } from "./message-tool-row"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

/** Display states for a tool row (mirrors the AI-SDK + approval vocabulary). */
export type ToolState =
  | "approval-requested"
  | "approval-responded"
  | "input-available"
  | "input-streaming"
  | "output-available"
  | "output-denied"
  | "output-error"

export type ToolPartLike = TMessagePart & {
  toolName?: string
  toolCallId?: string
  input?: unknown
  output?: unknown
  errorText?: string
}

/** Resolve the display state from the part data + the out-of-band approval status. */
export function deriveState(part: ToolPartLike, approval: ApprovalStatus | undefined): ToolState {
  if (approval === "pending") return "approval-requested"
  if (approval === "denied") return "output-denied"
  // approval === "approved" falls through to the part state (tool is now running).
  const state = part.state
  if (state === "output-error" || part.errorText) return "output-error"
  if (
    state === "output-available" ||
    (part.output !== undefined && part.output !== null && part.output !== "")
  ) {
    return "output-available"
  }
  if (state === "input-streaming") return "input-streaming"
  return "input-available"
}

/** Max characters shown for a tool parameter/result before truncating with an ellipsis. */
const TOOL_PREVIEW_LIMIT = 240

/**
 * Compact, length-capped preview of a tool parameter/result value. Strings are
 * returned as-is; objects/arrays are serialized to a single-line JSON (no
 * indentation) so the expanded row never shows a wall of pretty-printed JSON.
 * Values longer than {@link TOOL_PREVIEW_LIMIT} are truncated with "…".
 */
export function compactToolValue(value: unknown): string {
  if (value === undefined || value === null) return ""
  let compact: string
  if (typeof value === "string") {
    compact = value
  } else {
    try {
      compact = JSON.stringify(value)
    } catch {
      compact = String(value)
    }
  }
  return compact.length > TOOL_PREVIEW_LIMIT ? `${compact.slice(0, TOOL_PREVIEW_LIMIT)}…` : compact
}

const ToolInput = ({
  className,
  input,
  ...props
}: ComponentProps<"div"> & { input: unknown }) => {
  if (input === undefined || input === null) return null
  return (
    <div className={cn("space-y-2 overflow-hidden", className)} {...props}>
      <h4 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
        Parameters
      </h4>
      <div className="rounded-md bg-muted/50">
        <CodeBlock code={compactToolValue(input)} language="json" />
      </div>
    </div>
  )
}

const ToolOutput = ({
  className,
  output,
  errorText,
  ...props
}: ComponentProps<"div"> & { output: unknown; errorText?: string }) => {
  const hasOutput = output !== undefined && output !== null && output !== ""
  if (!errorText && !hasOutput) return null

  let OutputNode: ReactNode = <div>{output as ReactNode}</div>
  if (
    (typeof output === "object" && output !== null && !isValidElement(output)) ||
    typeof output === "string"
  ) {
    OutputNode = <CodeBlock code={compactToolValue(output)} language="json" />
  }

  return (
    <div className={cn("space-y-2", className)} {...props}>
      <h4 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
        {errorText ? "Error" : "Result"}
      </h4>
      <div
        className={cn(
          "overflow-x-auto rounded-md text-xs [&_table]:w-full",
          errorText ? "bg-destructive/10 text-destructive" : "bg-muted/50 text-foreground",
        )}
      >
        {errorText ? <div className="whitespace-pre-wrap p-3">{errorText}</div> : null}
        {OutputNode}
      </div>
    </div>
  )
}

/** Whether a tool row is still in flight (running / awaiting a decision). */
export function isToolActive(state: ToolState): boolean {
  return (
    state === "input-available" ||
    state === "input-streaming" ||
    state === "approval-requested"
  )
}

/**
 * Whether a tool row must start expanded because it awaits an interactive
 * decision — the only default-open case. Rows that merely run stay collapsed
 * (during generation and after); the user expands them via the trigger.
 */
export function isApprovalPending(state: ToolState): boolean {
  return state === "approval-requested"
}

function MessageToolPart({
  part,
  kind,
  name,
  toolCallId,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "tool") return null

  const p = part as ToolPartLike
  const { approvalStatusByItemId } = useMessageInteraction()
  const approval = toolCallId ? approvalStatusByItemId.get(toolCallId) : undefined
  const state = deriveState(p, approval)

  const partType = (p.type ?? "") as string
  const fromType = partType.startsWith("tool-")
    ? partType.split("-").slice(1).join("-")
    : partType
  const derivedName = (name ?? p.toolName ?? fromType) || "tool"

  const summary = summarizeToolCall(derivedName, p.input)

  return (
    <ToolRow defaultOpen={isApprovalPending(state)}>
      <ToolRowTrigger
        icon={toolRowIcon(derivedName)}
        label={summary.label}
        detail={summary.detail}
        state={state}
        title={p.errorText}
      />
      <ToolRowContent>
        <ToolInput input={p.input} />
        <ToolOutput output={p.output} errorText={p.errorText} />
      </ToolRowContent>
    </ToolRow>
  )
}

export default MessageToolPart
