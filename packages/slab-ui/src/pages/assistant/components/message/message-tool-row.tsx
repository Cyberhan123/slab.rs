"use client"

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@slab/components/collapsible"
import { cn } from "@slab/ui/lib/utils"
import {
  BotIcon,
  CheckCircleIcon,
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  FileDiffIcon,
  FilePenIcon,
  FileSearchIcon,
  FileTextIcon,
  FolderIcon,
  GlobeIcon,
  ListChecksIcon,
  Loader2Icon,
  MinusCircleIcon,
  SquareTerminalIcon,
  TriangleAlertIcon,
  WrenchIcon,
} from "lucide-react"
import { memo, useState, type ComponentProps, type ReactNode } from "react"

import type { ToolState } from "./message-tool-part"

/**
 * Compact "thinking-style" tool row — the collapsed form is a single muted
 * line (`icon Label: detail` + status symbol + chevron) instead of the old
 * bordered card, so multi-tool turns stay scannable and the basic information
 * (which file was read, which command ran) is visible even while folded.
 * Expanding reveals the full detail body (terminal output, diffs, JSON).
 *
 * Template: `message-reasoning-part.tsx` (`Reasoning`/`ReasoningTrigger`).
 */

type ToolRowProps = ComponentProps<typeof Collapsible> & {
  /** Rows awaiting an interactive decision start (and stay) expanded. */
  defaultOpen?: boolean
}

export function ToolRow({ className, defaultOpen = false, ...props }: ToolRowProps) {
  const [open, setOpen] = useState(defaultOpen)
  // Force the row open while it awaits a decision — including when the approval
  // lands AFTER the row mounted (an uncontrolled `defaultOpen` alone would have
  // consumed its one-shot at mount and left a late approval folded away).
  // Adjust-state-during-render pattern (guarded, lint-clean).
  if (defaultOpen && !open) {
    setOpen(true)
  }
  return (
    <Collapsible className={cn("group not-prose mb-0.5", className)} open={open} onOpenChange={setOpen} {...props} />
  )
}

/** Small status symbol replacing the old status Badge (size win). */
function ToolStatusSymbol({ state }: { state: ToolState }) {
  switch (state) {
    case "approval-requested":
      return (
        <span data-tool-state={state} title="Awaiting approval">
          <ClockIcon className="size-3.5 shrink-0 text-yellow-600" />
        </span>
      )
    case "approval-responded":
      return (
        <span data-tool-state={state} title="Responded">
          <CheckCircleIcon className="size-3.5 shrink-0 text-blue-600" />
        </span>
      )
    case "input-available":
    case "input-streaming":
      return (
        <span data-tool-state={state} title="Running">
          <Loader2Icon className="size-3.5 shrink-0 animate-spin" />
        </span>
      )
    case "output-available":
      return (
        <span data-tool-state={state} title="Completed">
          <CheckIcon className="size-3.5 shrink-0 text-green-600" />
        </span>
      )
    case "output-denied":
      return (
        <span data-tool-state={state} title="Denied">
          <MinusCircleIcon className="size-3.5 shrink-0 text-orange-600" />
        </span>
      )
    case "output-error":
      return (
        <span data-tool-state={state} title="Error">
          <TriangleAlertIcon className="size-3.5 shrink-0 text-red-600" />
        </span>
      )
  }
}

type ToolRowTriggerProps = ComponentProps<typeof CollapsibleTrigger> & {
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
  ({ className, icon, label, detail, state, title, ...props }: ToolRowTriggerProps) => {
    return (
      <CollapsibleTrigger
        className={cn(
          "flex w-full items-center gap-2 text-muted-foreground text-sm transition-colors hover:text-foreground",
          className,
        )}
        title={title}
        {...props}
      >
        {icon ?? <WrenchIcon className="size-4 shrink-0" />}
        <span className="flex min-w-0 flex-1 items-center gap-1.5">
          <span className="shrink-0 font-medium">
            {label}
            {detail ? ":" : ""}
          </span>
          {detail ? (
            <span className="truncate font-mono text-xs leading-5">{detail}</span>
          ) : null}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-1.5">
          <ToolStatusSymbol state={state} />
          <ChevronRightIcon className="size-4 shrink-0 group-data-[state=closed]:block group-data-[state=open]:hidden" />
          <ChevronDownIcon className="size-4 shrink-0 group-data-[state=closed]:hidden group-data-[state=open]:block" />
        </span>
      </CollapsibleTrigger>
    )
  },
)
ToolRowTrigger.displayName = "ToolRowTrigger"

export const ToolRowContent = ({
  className,
  ...props
}: ComponentProps<typeof CollapsibleContent>) => (
  <CollapsibleContent
    className={cn(
      "mt-1 text-sm",
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-muted-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className,
    )}
    {...props}
  />
)

/** Per-tool leading icon for the collapsed row. */
export function toolRowIcon(toolName: string): ReactNode {
  switch (toolName) {
    case "commandExecution":
      return <SquareTerminalIcon className="size-4 shrink-0" />
    case "read_file":
      return <FileTextIcon className="size-4 shrink-0" />
    case "write_file":
    case "fileChange":
      return <FilePenIcon className="size-4 shrink-0" />
    case "grep":
      return <FileSearchIcon className="size-4 shrink-0" />
    case "file_glob":
      return <FileDiffIcon className="size-4 shrink-0" />
    case "list_dir":
    case "fs_watch":
      return <FolderIcon className="size-4 shrink-0" />
    case "webSearch":
    case "web_search":
    case "tool_search":
      return <GlobeIcon className="size-4 shrink-0" />
    case "plan":
    case "update_plan":
    case "present_plan":
      return <ListChecksIcon className="size-4 shrink-0" />
    case "delegate_subagent":
    case "task.complete":
      return <BotIcon className="size-4 shrink-0" />
    default:
      if (toolName.startsWith("git_")) {
        return <FileDiffIcon className="size-4 shrink-0" />
      }
      return <WrenchIcon className="size-4 shrink-0" />
  }
}
