"use client"

import { Badge } from "@slab/components/badge"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@slab/components/collapsible"
import { cn } from "@/lib/utils"
import {
  CheckCircleIcon,
  ChevronDownIcon,
  CircleIcon,
  ClockIcon,
  WrenchIcon,
  XCircleIcon,
} from "lucide-react"
import { isValidElement, type ComponentProps, type ReactNode } from "react"

import { CodeBlock } from "./code-block"
import { Terminal } from "./terminal"
import { useMessageInteraction, type ApprovalStatus } from "./message-interaction-context"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

/** Display states for a tool card (mirrors the AI-SDK + approval vocabulary). */
type ToolState =
  | "approval-requested"
  | "approval-responded"
  | "input-available"
  | "input-streaming"
  | "output-available"
  | "output-denied"
  | "output-error"

const statusLabels: Record<ToolState, string> = {
  "approval-requested": "Awaiting Approval",
  "approval-responded": "Responded",
  "input-available": "Running",
  "input-streaming": "Pending",
  "output-available": "Completed",
  "output-denied": "Denied",
  "output-error": "Error",
}

const statusIcons: Record<ToolState, ReactNode> = {
  "approval-requested": <ClockIcon className="size-4 text-yellow-600" />,
  "approval-responded": <CheckCircleIcon className="size-4 text-blue-600" />,
  "input-available": <ClockIcon className="size-4 animate-pulse" />,
  "input-streaming": <CircleIcon className="size-4" />,
  "output-available": <CheckCircleIcon className="size-4 text-green-600" />,
  "output-denied": <XCircleIcon className="size-4 text-orange-600" />,
  "output-error": <XCircleIcon className="size-4 text-red-600" />,
}

const getStatusBadge = (state: ToolState) => (
  <Badge className="gap-1.5 rounded-full text-xs" variant="secondary">
    {statusIcons[state]}
    {statusLabels[state]}
  </Badge>
)

type ToolPartLike = TMessagePart & {
  toolName?: string
  toolCallId?: string
  input?: unknown
  output?: unknown
  errorText?: string
}

/** Resolve the display state from the part data + the out-of-band approval status. */
function deriveState(part: ToolPartLike, approval: ApprovalStatus | undefined): ToolState {
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

const Tool = ({ className, ...props }: ComponentProps<typeof Collapsible>) => (
  <Collapsible className={cn("group not-prose mb-2 w-full rounded-md border", className)} {...props} />
)

const ToolHeader = ({
  className,
  title,
  state,
  ...props
}: ComponentProps<typeof CollapsibleTrigger> & { title?: string; state: ToolState }) => (
  <CollapsibleTrigger
    className={cn("flex w-full items-center justify-between gap-4 p-3", className)}
    {...props}
  >
    <div className="flex min-w-0 items-center gap-2">
      <WrenchIcon className="size-4 shrink-0 text-muted-foreground" />
      <span className="truncate font-medium text-sm">{title}</span>
      {getStatusBadge(state)}
    </div>
    <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
  </CollapsibleTrigger>
)

const ToolContent = ({ className, ...props }: ComponentProps<typeof CollapsibleContent>) => (
  <CollapsibleContent
    className={cn(
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 space-y-4 p-4 text-popover-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className,
    )}
    {...props}
  />
)

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
        <CodeBlock code={typeof input === "string" ? input : JSON.stringify(input, null, 2)} language="json" />
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
  if (typeof output === "object" && output !== null && !isValidElement(output)) {
    OutputNode = <CodeBlock code={JSON.stringify(output, null, 2)} language="json" />
  } else if (typeof output === "string") {
    OutputNode = <CodeBlock code={output} language="json" />
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
  const isActive = state === "input-available" || state === "input-streaming" || state === "approval-requested"

  // Command-execution tools render as an interactive terminal (ANSI output,
  // streaming cursor) rather than JSON parameter/result cards.
  if (derivedName === "commandExecution") {
    const input = (p.input ?? {}) as { command?: string; cwd?: string }
    const output = typeof p.output === "string" ? p.output : ""
    const terminalOutput = [
      input.cwd ? `# cd ${input.cwd}` : null,
      input.command ? `$ ${input.command}` : null,
      output || (p.errorText ?? ""),
    ]
      .filter((line) => line !== null && line !== "")
      .join("\n")
    return (
      <Tool defaultOpen={isActive}>
        <ToolHeader title={derivedName} state={state} />
        <ToolContent>
          <Terminal output={terminalOutput} isStreaming={isActive} />
        </ToolContent>
      </Tool>
    )
  }

  return (
    <Tool defaultOpen={isActive}>
      <ToolHeader title={derivedName} state={state} />
      <ToolContent>
        <ToolInput input={p.input} />
        <ToolOutput output={p.output} errorText={p.errorText} />
      </ToolContent>
    </Tool>
  )
}

export default MessageToolPart
