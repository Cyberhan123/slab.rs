import { Play } from "lucide-react"

import { Button } from "@slab/components/button"
import { cn } from "@/lib/utils"

import { terminationReasonLabel } from "../lib/termination-reason"

type AgentResumeControlLabels = {
  resume: string
}

type AgentResumeControlProps = {
  reason: string | null
  onResume: () => void
  className?: string
  disabled?: boolean
  labels: AgentResumeControlLabels
}

/**
 * Termination-reason banner with a resume affordance (TC-FE-05). Shown only for
 * resumable reasons (max_turns_reached / repetition_detected / budget_exhausted
 * / interrupted) where the thread is kept alive and can be continued without
 * re-sending the original prompt.
 */
export function AgentResumeControl({
  reason,
  onResume,
  className,
  disabled,
  labels,
}: AgentResumeControlProps) {
  const copy = terminationReasonLabel(reason)
  if (!copy) {
    return null
  }

  return (
    <div
      className={cn(
        "flex max-w-[min(100%,42rem)] items-center gap-2 rounded-2xl border border-border/60 bg-[var(--surface-soft)] px-3 py-2",
        className
      )}
      data-testid="agent-resume-control"
    >
      <div className="min-w-0 flex-1">
        <div className="truncate text-xs font-medium text-foreground">{copy.title}</div>
        {copy.hint ? (
          <div className="truncate text-caption text-muted-foreground">{copy.hint}</div>
        ) : null}
      </div>
      <Button
        type="button"
        variant="pill"
        size="sm"
        className="h-7 rounded-full px-3 text-caption"
        disabled={disabled}
        onClick={onResume}
        data-testid="agent-resume-button"
      >
        <Play className="size-3.5" />
        {labels.resume}
      </Button>
    </div>
  )
}
