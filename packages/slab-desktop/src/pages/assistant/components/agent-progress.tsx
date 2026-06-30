import { cn } from "@/lib/utils"
import type { PlanProgress } from "../lib/plan-progress"

type AgentProgressLabels = {
  progress: string
}

type AgentProgressProps = {
  progress: PlanProgress | null
  className?: string
  labels: AgentProgressLabels
}

/**
 * Simple X/N plan progress bar (TC-FE-05). DAG-free by design (red-team
 * must_cut): only completed/total from the latest `plan_update`. Uses the
 * native `<progress>` element for accessibility.
 */
export function AgentProgress({ progress, className, labels }: AgentProgressProps) {
  if (!progress || progress.total <= 0) {
    return null
  }

  const completed = Math.min(Math.max(progress.completed, 0), progress.total)

  return (
    <div
      className={cn(
        "flex max-w-[min(100%,42rem)] items-center gap-3 rounded-2xl border border-border/60 bg-[var(--surface-soft)] px-3 py-2",
        className
      )}
      data-testid="agent-progress"
    >
      <progress
        value={completed}
        max={progress.total}
        aria-label={labels.progress}
        className="h-1.5 min-w-[4rem] flex-1 accent-foreground"
        data-testid="agent-progress-bar"
      />
      <span className="text-caption tabular-nums text-muted-foreground">
        {completed}/{progress.total}
      </span>
    </div>
  )
}
