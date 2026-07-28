import { describe, expect, it } from 'vitest'

import { isCompactCommand, parseAssistantCommand } from '../assistant-commands'

describe('isCompactCommand', () => {
  it('matches /compact with surrounding whitespace', () => {
    expect(isCompactCommand('/compact')).toBe(true)
    expect(isCompactCommand('  /compact  ')).toBe(true)
  })

  it('rejects non-exact matches', () => {
    expect(isCompactCommand('/compactx')).toBe(false)
    expect(isCompactCommand('/c')).toBe(false)
    expect(isCompactCommand('compact')).toBe(false)
    expect(isCompactCommand('')).toBe(false)
  })
})

describe('parseAssistantCommand', () => {
  it('parses a bare command', () => {
    expect(parseAssistantCommand('/compact')).toEqual({ name: 'compact', args: '' })
  })

  it('parses a name plus args', () => {
    expect(parseAssistantCommand('/foo bar baz')).toEqual({ name: 'foo', args: 'bar baz' })
  })

  it('returns null for non-command input', () => {
    expect(parseAssistantCommand('hello')).toBeNull()
  })

  it('returns null for a bare slash', () => {
    expect(parseAssistantCommand('/')).toBeNull()
  })
})
