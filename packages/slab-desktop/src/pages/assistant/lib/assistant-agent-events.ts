import type { AgentResponsesServerMessage, AgentStatus } from './assistant-types'

export type AssistantAgentStreamEvent =
  | { type: 'agent_status'; status: AgentStatus }
  | { type: 'approval_required'; call_id: string; tool_name: string; command: string }
  | { type: 'assistant_delta'; text: string }
  | { type: 'lagged' }
  | { type: 'tool_call_output'; call_id: string; output: string }
  | { type: 'tool_call_started'; tool_name: string; call_id: string; arguments: string }
  | { type: 'turn_cancelled'; reason: string }
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
    case 'agent.status':
      return typeof value.status === 'string'
        ? { type: 'agent_status', status: value.status as AgentStatus }
        : null
    case 'response.tool_call.approval_required':
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
    case 'response.output_text.delta':
      return typeof value.delta === 'string' ? { type: 'assistant_delta', text: value.delta } : null
    case 'agent.stream.lagged':
      return { type: 'lagged' }
    case 'response.tool_call.output':
      return typeof value.call_id === 'string' && typeof value.output === 'string'
        ? { type: 'tool_call_output', call_id: value.call_id, output: value.output }
        : null
    case 'response.function_call_arguments.done':
      return typeof value.name === 'string' &&
        typeof value.call_id === 'string' &&
        typeof value.arguments === 'string'
        ? {
            type: 'tool_call_started',
            tool_name: value.name,
            call_id: value.call_id,
            arguments: value.arguments,
          }
        : null
    case 'response.cancelled':
      return typeof value.reason === 'string'
        ? { type: 'turn_cancelled', reason: value.reason }
        : null
    case 'response.output_text.done':
    case 'response.completed':
      return typeof value.text === 'string' ? { type: 'turn_completed', text: value.text } : null
    case 'response.failed':
      return typeof value.error === 'string' ? { type: 'turn_failed', error: value.error } : null
    case 'response.context.compact_started':
    case 'response.context.compact_completed':
    case 'response.context.compact_skipped':
    case 'response.metrics':
    case 'response.background':
      return null
    default:
      return null
  }
}

export function parseAssistantAgentServerMessage(
  data: string
): AgentResponsesServerMessage | null {
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
    case 'agent.ack':
    case 'agent.session.restored':
    case 'agent.error':
      return value as AgentResponsesServerMessage
    default:
      return null
  }
}
