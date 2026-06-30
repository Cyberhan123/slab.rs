import { describe, expect, it } from 'vitest'

import { parsePlanProgress } from './plan-progress'

describe('parsePlanProgress', () => {
  it('reads completed/total from counts and surfaces the current step', () => {
    const plan = JSON.stringify({
      summary: 'ship it',
      items: [
        { step: 'inspect', status: 'completed' },
        { step: 'implement', status: 'in_progress' },
        { step: 'verify', status: 'pending' },
      ],
      counts: { pending: 1, in_progress: 1, completed: 1, blocked: 0 },
      current_step: 'implement',
    })

    expect(parsePlanProgress(plan)).toEqual({ total: 3, completed: 1, currentStep: 'implement' })
  })

  it('falls back to counting completed items when counts are missing', () => {
    const plan = JSON.stringify({
      items: [
        { step: 'a', status: 'completed' },
        { step: 'b', status: 'completed' },
        { step: 'c', status: 'pending' },
      ],
    })

    expect(parsePlanProgress(plan)).toEqual({ total: 3, completed: 2 })
  })

  it('returns null for empty plans so the progress bar stays hidden', () => {
    expect(parsePlanProgress(JSON.stringify({ items: [] }))).toBeNull()
  })

  it('returns null for non-plan tool output', () => {
    expect(parsePlanProgress('not json')).toBeNull()
    expect(parsePlanProgress(JSON.stringify({ items: 'nope' }))).toBeNull()
    expect(parsePlanProgress(JSON.stringify({ ok: true }))).toBeNull()
  })
})
