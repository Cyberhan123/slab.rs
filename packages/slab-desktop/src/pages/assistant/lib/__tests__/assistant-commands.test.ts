import { describe, expect, it } from 'vitest'

import type { CommandInfo } from '../harness/types'

import {
  isCompactCommand,
  isForkCommand,
  parseAssistantCommand,
  resolveCommandDispatch,
} from '../assistant-commands'

/** Mirror of the server-side `command/list` snapshot: the built-ins + one skill. */
const COMMANDS: CommandInfo[] = [
  {
    name: 'compact',
    aliases: [],
    description: 'Summarize the conversation history to reclaim context.',
    kind: 'control',
    source: 'builtin',
    controlAction: 'compact',
  },
  {
    name: 'fork',
    aliases: [],
    description: 'Branch the current thread into a new child thread.',
    kind: 'control',
    source: 'builtin',
    controlAction: 'fork',
  },
  {
    name: 'plan',
    aliases: [],
    description: 'Seed a planning prompt for the model.',
    kind: 'prompt',
    source: 'builtin',
  },
  {
    name: 'rust',
    aliases: [],
    description: 'Rust tips',
    kind: 'prompt',
    source: 'skill',
  },
]

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

describe('isForkCommand', () => {
  it('matches /fork with surrounding whitespace', () => {
    expect(isForkCommand('/fork')).toBe(true)
    expect(isForkCommand('  /fork  ')).toBe(true)
  })

  it('rejects non-exact matches', () => {
    expect(isForkCommand('/forkx')).toBe(false)
    expect(isForkCommand('/f')).toBe(false)
    expect(isForkCommand('fork')).toBe(false)
    expect(isForkCommand('/compact')).toBe(false)
    expect(isForkCommand('')).toBe(false)
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

describe('resolveCommandDispatch', () => {
  it('routes /compact to the compact control action', () => {
    expect(resolveCommandDispatch('/compact', COMMANDS)).toEqual({
      action: 'control',
      controlAction: 'compact',
    })
  })

  it('routes /fork to the fork control action', () => {
    expect(resolveCommandDispatch('/fork', COMMANDS)).toEqual({
      action: 'control',
      controlAction: 'fork',
    })
  })

  it('routes a control command with surrounding whitespace', () => {
    expect(resolveCommandDispatch('  /compact  ', COMMANDS)).toEqual({
      action: 'control',
      controlAction: 'compact',
    })
  })

  it('falls through to send for a prompt command (/plan)', () => {
    expect(resolveCommandDispatch('/plan foo', COMMANDS)).toEqual({ action: 'send' })
  })

  it('falls through to send for a prompt skill command', () => {
    expect(resolveCommandDispatch('/rust', COMMANDS)).toEqual({ action: 'send' })
  })

  it('falls through to send for an unknown command', () => {
    expect(resolveCommandDispatch('/unknown', COMMANDS)).toEqual({ action: 'send' })
  })

  it('falls through to send for non-command text', () => {
    expect(resolveCommandDispatch('hello world', COMMANDS)).toEqual({ action: 'send' })
  })

  it('resolves a command by alias', () => {
    const withAlias: CommandInfo[] = [
      { ...COMMANDS[0], aliases: ['c'] },
    ]
    expect(resolveCommandDispatch('/c', withAlias)).toEqual({
      action: 'control',
      controlAction: 'compact',
    })
  })
})
