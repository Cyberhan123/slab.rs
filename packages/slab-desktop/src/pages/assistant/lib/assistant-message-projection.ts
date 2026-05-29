import type {
  AgentStatus,
  AgentThreadMessageResponse,
  AssistantMessageRecord,
  AssistantThought,
  SessionMessageResponse,
} from './assistant-types'
import { toStoredSessionAssistantMessage } from './assistant-history'
import { stripTrailingAssistantTurnArtifacts } from './assistant-message-utils'

export function projectSessionMessages(
  messages: SessionMessageResponse[] | undefined
): AssistantMessageRecord[] {
  return (messages ?? []).map((message) => ({
    id: message.id,
    message: toStoredSessionAssistantMessage(message.role, message.content),
    status: 'success',
  }))
}

function finalizePendingThoughts(
  thoughts: AssistantThought[],
  threadStatus: AgentStatus | undefined
): AssistantThought[] {
  return thoughts.map((thought) => {
    if (thought.status !== 'loading') {
      return thought
    }

    switch (threadStatus) {
      case 'pending':
      case 'running':
      case 'interrupting':
        return thought
      case 'interrupted':
      case 'shutdown':
        return { ...thought, status: 'abort' }
      case 'errored':
        return { ...thought, status: 'error' }
      case 'completed':
      case undefined:
        return { ...thought, status: 'success' }
    }
  })
}

function isToolOnlyContent(content: string): boolean {
  return content
    .split('\n')
    .every((line) => line.startsWith('tool_call') || line.startsWith('tool_call_id:'))
}

export function projectAgentThreadMessages(
  messages: AgentThreadMessageResponse[] | undefined,
  threadStatus?: AgentStatus
): AssistantMessageRecord[] {
  const projectedMessages: AssistantMessageRecord[] = []
  let assistantGroup:
    | {
        id: string
        content: string[]
        thoughts: AssistantThought[]
      }
    | null = null

  const getAssistantGroup = (id: string) => {
    if (!assistantGroup) {
      assistantGroup = { id, content: [], thoughts: [] }
    }

    return assistantGroup
  }

  const flushAssistantGroup = () => {
    if (!assistantGroup) {
      return
    }

    const content = assistantGroup.content.join('\n\n').trim()
    const thoughts = finalizePendingThoughts(assistantGroup.thoughts, threadStatus)
    if (!content && thoughts.length === 0) {
      assistantGroup = null
      return
    }

    projectedMessages.push({
      id: assistantGroup.id,
      message: {
        role: 'assistant',
        content,
        ...(thoughts.length > 0 ? { thoughts } : {}),
      },
      status: 'success',
    })
    assistantGroup = null
  }

  for (const message of messages ?? []) {
    if (message.role === 'tool') {
      const toolCallId = message.tool_call_id?.trim()
      if (!toolCallId) {
        continue
      }

      const group = getAssistantGroup(`thoughts-${toolCallId}`)
      let matched = false
      group.thoughts = group.thoughts.map((thought) => {
        if (thought.callId !== toolCallId && thought.id !== toolCallId) {
          return thought
        }

        matched = true
        return {
          ...thought,
          detail: message.content.trim() || thought.detail,
          status: 'success',
        }
      })

      if (!matched) {
        group.thoughts.push({
          callId: toolCallId,
          detail: message.content.trim(),
          id: toolCallId,
          status: 'success',
          summary: `tool_call_id: ${toolCallId}`,
          title: 'tool_call',
        })
      }
      continue
    }

    if (message.role === 'user') {
      flushAssistantGroup()
      projectedMessages.push({
        id: message.id,
        message: {
          role: 'user',
          content: message.content,
        },
        status: 'success',
      })
      continue
    }

    if (message.role !== 'assistant') {
      continue
    }

    const group = getAssistantGroup(message.id)
    const content = stripTrailingAssistantTurnArtifacts(message.content)
    if (content.trim() && !isToolOnlyContent(content)) {
      group.content.push(content)
    }

    const toolCalls = message.tool_calls ?? []
    if (toolCalls.length > 0) {
      group.thoughts = [
        ...group.thoughts,
        ...toolCalls.map<AssistantThought>((toolCall, index) => {
          const callId = toolCall.id?.trim() || `${message.id}-tool-${index}`
          const toolName = toolCall.function.name.trim() || 'tool_call'
          const detail = toolCall.function.arguments ?? ''

          return {
            callId,
            detail,
            id: callId,
            status: 'loading',
            summary: `tool_call id=${callId}: ${toolName}(${detail})`,
            title: 'tool_call',
            toolName,
          }
        }),
      ]
    }
  }

  flushAssistantGroup()
  return projectedMessages
}
