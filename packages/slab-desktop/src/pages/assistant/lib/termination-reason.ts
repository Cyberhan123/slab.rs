/**
 * Map a structured agent termination reason (the `reason` field carried by
 * `turn_cancelled` / thread status — see crates/slab-agent TerminationReason)
 * to user-facing copy and a resumability flag (TC-FE-05).
 *
 * Resumable reasons (max_turns_reached / repetition_detected / budget_exhausted
 * / interrupted) keep the thread interruptible-and-resumable; the UI offers a
 * "resume" affordance for them.
 */

export interface TerminationReasonCopy {
  title: string
  hint?: string
}

const RESUMABLE_REASONS: Record<string, TerminationReasonCopy> = {
  max_turns_reached: {
    title: 'Reached the turn limit',
    hint: 'The agent paused at the turn cap. Resume to keep going.',
  },
  repetition_detected: {
    title: 'Paused a repetitive loop',
    hint: 'The agent was repeating itself. Adjust the request and resume.',
  },
  budget_exhausted: {
    title: 'Token budget exhausted',
    hint: 'The agent paused to protect your quota. Raise the budget and resume.',
  },
  interrupted: {
    title: 'Interrupted',
    hint: 'Resume from where the agent stopped.',
  },
}

/** Return user-facing copy for a termination reason, or null if not resumable/unknown. */
export function terminationReasonLabel(
  reason: string | undefined | null
): TerminationReasonCopy | null {
  if (!reason) {
    return null
  }
  return RESUMABLE_REASONS[reason] ?? null
}

/** A reason is resumable when the underlying thread is kept alive (Interrupted). */
export function isResumableReason(reason: string | undefined | null): boolean {
  return terminationReasonLabel(reason) !== null
}
