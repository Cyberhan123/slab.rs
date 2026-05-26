import type { AgentStatus } from './assistant-types'

export type AssistantAgentStreamEvent =
  | { type: 'agent_status'; status: AgentStatus }
  | { type: 'approval_required'; call_id: string; tool_name: string; command: string }
  | { type: 'assistant_delta'; text: string }
  | { type: 'lagged' }
  | { type: 'tool_call_output'; call_id: string; output: string }
  | { type: 'tool_call_started'; tool_name: string; call_id: string; arguments: string }
  | { type: 'turn_completed'; text: string }
  | { type: 'turn_failed'; error: string }

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

export function parseAssistantAgentStreamEvent(data: string): AssistantAgentStreamEvent | null {
  let value: unknown
  try {
    value = JSON.parse(data)
  } catch {
    return null
  }

  if (!isRecord(value) || typeof value.type !== 'string') {
    return null
  }

  switch (value.type) {
    case 'agent_status':
      return typeof value.status === 'string'
        ? { type: 'agent_status', status: value.status as AgentStatus }
        : null
    case 'approval_required':
      return typeof value.call_id === 'string' &&
        typeof value.tool_name === 'string' &&
        typeof value.command === 'string'
        ? {
            type: 'approval_required',
            call_id: value.call_id,
            tool_name: value.tool_name,
            command: value.command,
          }
        : null
    case 'assistant_delta':
      return typeof value.text === 'string' ? { type: 'assistant_delta', text: value.text } : null
    case 'lagged':
      return { type: 'lagged' }
    case 'tool_call_output':
      return typeof value.call_id === 'string' && typeof value.output === 'string'
        ? { type: 'tool_call_output', call_id: value.call_id, output: value.output }
        : null
    case 'tool_call_started':
      return typeof value.tool_name === 'string' &&
        typeof value.call_id === 'string' &&
        typeof value.arguments === 'string'
        ? {
            type: 'tool_call_started',
            tool_name: value.tool_name,
            call_id: value.call_id,
            arguments: value.arguments,
          }
        : null
    case 'turn_completed':
      return typeof value.text === 'string' ? { type: 'turn_completed', text: value.text } : null
    case 'turn_failed':
      return typeof value.error === 'string' ? { type: 'turn_failed', error: value.error } : null
    default:
      return null
  }
}
