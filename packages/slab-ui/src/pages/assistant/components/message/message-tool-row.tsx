"use client"

import {
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceDescription,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@slab/components/trace"
import {
  BotIcon,
  FileDiffIcon,
  FilePenIcon,
  FileSearchIcon,
  FileTextIcon,
  FolderIcon,
  GlobeIcon,
  ListChecksIcon,
  MessageCircleQuestionIcon,
  SquareTerminalIcon,
  WrenchIcon,
} from "lucide-react"
import { memo, useState, type ComponentProps, type ReactNode } from "react"

import type { ToolState } from "./message-tool-part"

/**
 * Compact thinking-style tool row on the shadcn Trace primitives: the folded
 * form is a single muted `TraceLine` (`icon Label: detail` + status word)
 * whose `status` drives the running shimmer / error coloring; expanding
 * reveals the full detail body (terminal output, diffs, JSON). The row is a
 * bare line while nothing can be shown, and a collapsible once a body exists.
 *
 * The tool lifecycle stays exposed as `data-tool-state` on the line (the
 * `span[data-tool-state]` e2e hook) alongside the Trace status vocabulary:
 * running / error / none — never dimmed with opacity.
 */

type ToolRowProps = Omit<
  ComponentProps<typeof TraceCollapsible>,
  "open" | "onOpenChange"
> & {
  /** Rows awaiting an interactive decision start (and stay) expanded. */
  defaultOpen?: boolean
}

export function ToolRow({
  className,
  defaultOpen = false,
  ...props
}: ToolRowProps) {
  const [open, setOpen] = useState(defaultOpen)
  // Force the row open while it awaits a decision — including when the approval
  // lands AFTER the row mounted (an uncontrolled `defaultOpen` alone would have
  // consumed its one-shot at mount and left a late approval folded away).
  // Adjust-state-during-render pattern (guarded, lint-clean).
  if (defaultOpen && !open) {
    setOpen(true)
  }
  return (
    <TraceCollapsible
      className={className}
      open={open}
      onOpenChange={setOpen}
      {...props}
    />
  )
}

/** Trace status for a tool state: running / error / none. Never "pending" —
 * the vocabulary stays Running, Ran, Failed to run, Denied. */
function traceStatus(state: ToolState): "running" | "error" | undefined {
  switch (state) {
    case "approval-requested":
    case "approval-responded":
    case "input-available":
    case "input-streaming":
      return "running"
    case "output-error":
      return "error"
    case "output-available":
    case "output-denied":
      return undefined
  }
}

/** Short status word for the line's trailing description. */
const STATE_WORDS: Record<ToolState, string> = {
  "approval-requested": "Awaiting approval",
  "approval-responded": "Responded",
  "input-available": "Running",
  "input-streaming": "Running",
  "output-available": "Ran",
  "output-denied": "Denied",
  "output-error": "Failed to run",
}

type ToolRowTriggerProps = Omit<
  ComponentProps<typeof TraceCollapsibleTrigger>,
  "children"
> & {
  /** Leading tool icon; defaults to the per-tool map below. */
  icon?: ReactNode
  /** Summary label — a tool identity (`Read`/`Write`/`Bash`), not localized. */
  label: string
  /** The summarized argument (path / command / pattern). */
  detail?: string
  state: ToolState
  /** Tooltip text for the whole line (e.g. the cwd for a Bash call). */
  title?: string
}

export const ToolRowTrigger = memo(
  ({ className, icon, label, detail, state, title, ...props }: ToolRowTriggerProps) => (
    <TraceCollapsibleTrigger className={className} {...props}>
      <TraceLine data-tool-state={state} status={traceStatus(state)} title={title}>
        <TraceIcon>{icon ?? <WrenchIcon />}</TraceIcon>
        <TraceTitle>{detail ? `${label}: ${detail}` : label}</TraceTitle>
        <TraceDescription>{STATE_WORDS[state]}</TraceDescription>
      </TraceLine>
    </TraceCollapsibleTrigger>
  ),
)
ToolRowTrigger.displayName = "ToolRowTrigger"

export const ToolRowContent = ({
  className,
  ...props
}: ComponentProps<typeof TraceCollapsibleContent>) => (
  <TraceCollapsibleContent className={className} {...props} />
)

/** Per-tool leading icon for the collapsed row. */
export function toolRowIcon(toolName: string): ReactNode {
  switch (toolName) {
    case "commandExecution":
      return <SquareTerminalIcon />
    case "read_file":
      return <FileTextIcon />
    case "write_file":
    case "fileChange":
      return <FilePenIcon />
    case "grep":
      return <FileSearchIcon />
    case "file_glob":
      return <FileDiffIcon />
    case "list_dir":
    case "fs_watch":
      return <FolderIcon />
    case "webSearch":
    case "web_search":
    case "tool_search":
      return <GlobeIcon />
    case "plan":
    case "update_plan":
    case "present_plan":
      return <ListChecksIcon />
    case "questionnaire":
      return <MessageCircleQuestionIcon />
    case "delegate_subagent":
    case "task.complete":
      return <BotIcon />
    default:
      if (toolName.startsWith("git_")) {
        return <FileDiffIcon />
      }
      return <WrenchIcon />
  }
}
