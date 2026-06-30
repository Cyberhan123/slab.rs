/**
 * Parse a `plan_update` tool output into a lightweight progress snapshot.
 *
 * The backend `plan_update` tool (crates/slab-agent-tools/src/plan.rs) returns:
 * `{ summary?, items: [{ step, status, result_ref? }], counts: { pending, in_progress, completed, blocked }, current_step? }`.
 *
 * Used by the agent progress bar (TC-FE-05) to render X/N completed without
 * drawing a DAG (red-team must_cut).
 */

export interface PlanProgress {
  total: number
  completed: number
  currentStep?: string
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

/**
 * Parse a plan_update tool output string into a PlanProgress snapshot.
 * Returns `null` when the output is not a recognizable non-empty plan.
 */
export function parsePlanProgress(toolOutput: string): PlanProgress | null {
  let value: unknown
  try {
    value = JSON.parse(toolOutput)
  } catch {
    return null
  }

  if (!isRecord(value) || !Array.isArray(value.items) || value.items.length === 0) {
    return null
  }

  const items = value.items as unknown[]
  const completedFromItems = items.filter(
    (item) => isRecord(item) && item.status === 'completed'
  ).length
  const counts = isRecord(value.counts) ? value.counts : {}
  const completed =
    typeof counts.completed === 'number' ? counts.completed : completedFromItems

  const currentStep =
    typeof value.current_step === 'string' && value.current_step.trim()
      ? value.current_step.trim()
      : undefined

  return { total: items.length, completed, currentStep }
}
