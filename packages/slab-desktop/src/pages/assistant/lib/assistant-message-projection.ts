import type {
  AgentThreadMessageResponse,
  AssistantMessageRecord,
  SessionMessageResponse,
} from './assistant-types'
import { toStoredSessionAssistantMessage } from './assistant-history'

export function projectSessionMessages(
  messages: SessionMessageResponse[] | undefined
): AssistantMessageRecord[] {
  return (messages ?? []).map((message) => ({
    id: message.id,
    message: toStoredSessionAssistantMessage(message.role, message.content),
    status: 'success',
  }))
}

export function projectAgentThreadMessages(
  messages: AgentThreadMessageResponse[] | undefined
): AssistantMessageRecord[] {
  return (messages ?? [])
    .filter((message) => message.role === 'assistant' || message.role === 'user')
    .filter((message) => {
      if (message.role !== 'assistant') {
        return true
      }

      const content = message.content.trim()
      if (!content) {
        return false
      }

      return !content
        .split('\n')
        .every((line) => line.startsWith('tool_call') || line.startsWith('tool_call_id:'))
    })
    .map((message) => ({
      id: message.id,
      message: {
        role: message.role,
        content: message.content,
      },
      status: 'success',
    }))
}
