import type {
  AgentStatus,
  AgentThreadMessageResponse,
  AssistantMessageRecord,
  AssistantThought,
} from './assistant-types'
import { isRecord } from './assistant-types'
import { stripTrailingAssistantTurnArtifacts } from './assistant-message-utils'

const MAX_TOOL_DETAIL_LINES = 20

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

export function formatKnownToolResult(toolName: string | undefined, rawDetail: string): string {
  const trimmed = rawDetail.trim()
  if (!trimmed || !toolName) {
    return rawDetail
  }

  let parsed: unknown
  try {
    parsed = JSON.parse(trimmed)
  } catch {
    return rawDetail
  }

  if (!isRecord(parsed)) {
    return rawDetail
  }

  if (toolName === 'plan_update') {
    const lines: string[] = []
    if (typeof parsed.summary === 'string' && parsed.summary.trim()) {
      lines.push(parsed.summary.trim())
    }
    if (Array.isArray(parsed.items)) {
      for (const item of parsed.items) {
        if (!isRecord(item) || typeof item.step !== 'string' || typeof item.status !== 'string') {
          continue
        }
        lines.push(`${item.status.trim()}: ${item.step.trim()}`)
      }
    }
    return lines.length > 0 ? lines.join('\n') : rawDetail
  }

  if (toolName === 'code_lsp_status') {
    const lines: string[] = []
    if (typeof parsed.language_id === 'string') {
      lines.push(`language: ${parsed.language_id}`)
    }
    if (isRecord(parsed.provider)) {
      const id = typeof parsed.provider.id === 'string' ? parsed.provider.id : 'unknown'
      const transport =
        typeof parsed.provider.transport === 'string' ? parsed.provider.transport : 'unknown'
      lines.push(`provider: ${id} (${transport})`)
    } else {
      lines.push('provider: unavailable')
    }
    if (typeof parsed.workspace_root === 'string') {
      lines.push(`workspace: ${parsed.workspace_root}`)
    }
    return lines.length > 0 ? lines.join('\n') : rawDetail
  }

  if (toolName !== 'file_glob' && toolName !== 'grep') {
    return rawDetail
  }

  const matches = Array.isArray(parsed.matches) ? parsed.matches : []
  const total = typeof parsed.total === 'number' ? parsed.total : matches.length
  const lines = [`${total} matches${parsed.truncated === true ? ' (truncated)' : ''}`]
  for (const match of matches.slice(0, MAX_TOOL_DETAIL_LINES)) {
    if (!isRecord(match)) {
      continue
    }
    if (toolName === 'grep') {
      const file = typeof match.file === 'string' ? match.file : ''
      const line = typeof match.line === 'number' ? match.line : ''
      const text = typeof match.text === 'string' ? match.text : ''
      lines.push(`${file}:${line}: ${text}`.trim())
      continue
    }
    const path = typeof match.path === 'string' ? match.path : ''
    const kind = typeof match.kind === 'string' ? match.kind : 'match'
    lines.push(`${kind}: ${path}`.trim())
  }
  if (matches.length > MAX_TOOL_DETAIL_LINES) {
    lines.push(`... ${matches.length - MAX_TOOL_DETAIL_LINES} more`)
  }
  return lines.join('\n')
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
          detail: formatKnownToolResult(thought.toolName, message.content.trim()) || thought.detail,
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
