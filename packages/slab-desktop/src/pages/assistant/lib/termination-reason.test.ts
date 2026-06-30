import { describe, expect, it } from 'vitest'

import { isResumableReason, terminationReasonLabel } from './termination-reason'

describe('terminationReasonLabel', () => {
  it('maps resumable reasons to user-facing copy', () => {
    expect(terminationReasonLabel('max_turns_reached')?.title).toBe('Reached the turn limit')
    expect(terminationReasonLabel('repetition_detected')?.title).toBe('Paused a repetitive loop')
    expect(terminationReasonLabel('budget_exhausted')?.title).toBe('Token budget exhausted')
    expect(terminationReasonLabel('interrupted')?.title).toBe('Interrupted')
  })

  it('returns null for completed / unknown / empty reasons', () => {
    expect(terminationReasonLabel('completed')).toBeNull()
    expect(terminationReasonLabel('some_future_reason')).toBeNull()
    expect(terminationReasonLabel(undefined)).toBeNull()
    expect(terminationReasonLabel(null)).toBeNull()
  })
})

describe('isResumableReason', () => {
  it('is true only for the resumable reason set', () => {
    expect(isResumableReason('max_turns_reached')).toBe(true)
    expect(isResumableReason('interrupted')).toBe(true)
    expect(isResumableReason('completed')).toBe(false)
    expect(isResumableReason(undefined)).toBe(false)
  })
})
